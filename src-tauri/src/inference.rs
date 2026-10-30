use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use llama_cpp_rs::LLama;
use llama_cpp_rs::options::{ModelOptions, PredictOptions};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

use crate::model_manager::ModelManager;

/// Monotonic counter backing stream ids, so each generation run gets its own
/// event channel and the frontend can unsubscribe a single listener safely.
static STREAM_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_stream_id() -> String {
    format!("s{}", STREAM_SEQ.fetch_add(1, Ordering::SeqCst))
}

/// A thread-safe handle to a loaded llama.cpp model.
///
/// llama.cpp contexts are only touched from one thread at a time (guarded by a
/// `Mutex`), so it is sound to declare the handle `Send + Sync` for Tauri
/// state even though the underlying raw pointer is not `Sync`.
pub(crate) struct LlamaHandle(LLama);

// SAFETY: LlamaHandle contains a llama_cpp_rs::LLama context whose raw pointer is not Sync.
// All access to the inner LLama is guarded by LlamaState.model Mutex, which provides
// exclusive mutable access for eval operations and makes sharing the handle via Arc safe.
// No interior mutability or data races exist outside the Mutex guard.
unsafe impl Send for LlamaHandle {}
unsafe impl Sync for LlamaHandle {}

/// Holds the currently loaded GGUF model instance.
pub(crate) struct LlamaState {
    model: Mutex<Option<LlamaHandle>>,
    model_path: Mutex<Option<std::path::PathBuf>>,
}

impl Default for LlamaState {
    fn default() -> Self {
        Self::new()
    }
}

impl LlamaState {
    pub(crate) fn new() -> Self {
        Self {
            model: Mutex::new(None),
            model_path: Mutex::new(None),
        }
    }

    pub(crate) fn active_model_path(&self) -> Option<std::path::PathBuf> {
        let guard = self.model_path.lock().ok()?;
        guard.clone()
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeRequest {
    pub text: String,
    pub max_length: Option<i32>,
}

/// Load + activate a GGUF model into the llama.cpp runtime.
#[tauri::command]
pub async fn inference_load(
    app: AppHandle,
    state: State<'_, Arc<LlamaState>>,
    mgr: State<'_, ModelManager>,
    id: String,
) -> Result<String, String> {
    let entry = mgr
        .inner()
        .list_models()
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("model {id} not in catalog"))?;
    if !entry.installed {
        return Err(format!("model {id} is not installed"));
    }
    let path = mgr.primary_path(&id);

    let mut opts = ModelOptions::default();
    opts.context_size = 2048;
    opts.n_batch = 512;
    opts.embeddings = false;

    let llm = LLama::new(path.to_string_lossy().to_string(), &opts)
        .map_err(|e| format!("failed to load model: {e}"))?;

    {
        let mut m = state.model.lock().map_err(|e| format!("model mutex poisoned: {e}"))?;
        *m = Some(LlamaHandle(llm));
    }
    {
        let mut p = state.model_path.lock().map_err(|e| format!("model path mutex poisoned: {e}"))?;
        *p = Some(path.clone());
    }
    mgr.inner().activate(&id)?;

    let _ = app.emit("model_activated", serde_json::json!({ "id": id }));
    Ok(format!("loaded {id}"))
}

/// Stream chat completion. Tokens are emitted as `llm_stream` events carrying a
/// `stream_id`; `llm_done` / `llm_error` signal the terminal state.
#[tauri::command]
pub async fn chat_stream(
    app: AppHandle,
    state: State<'_, Arc<LlamaState>>,
    req: ChatRequest,
) -> Result<String, String> {
    state
        .active_model_path()
        .ok_or_else(|| "no model loaded".to_string())?;

    let system_prompt = req.system_prompt.unwrap_or_default();
    let temperature = req.temperature.unwrap_or(0.7);
    let max_tokens = req.max_tokens.unwrap_or(512);

    let prompt = if system_prompt.is_empty() {
        req.message.clone()
    } else {
        format!("{system_prompt}\n{}", req.message)
    };

    let state_arc = state.inner().clone();
    let stream_id = next_stream_id();
    let sid = stream_id.clone();

    let app_send = app.clone();
    std::thread::spawn(move || {
        let mut opts = PredictOptions::default();
        opts.threads = 4;
        opts.tokens = max_tokens;
        opts.temperature = temperature;
        opts.top_p = 0.9;
        opts.repeat = 64;
        opts.seed = -1;

        let app_tok = app_send.clone();
        let sid_tok = sid.clone();
        opts.set_token_callback(Some(Box::new(move |token: String| -> bool {
            let _ = app_tok.emit(
                "llm_stream",
                serde_json::json!({ "stream_id": sid_tok, "token": token }),
            );
            true
        })));

        let result = {
            let guard = match state_arc.model.lock() {
                Ok(g) => g,
                Err(e) => {
                    log::error!("model mutex poisoned in chat_stream: {e}");
                    let _ = app_send.emit(
                        "llm_error",
                        serde_json::json!({ "stream_id": sid, "error": "model mutex poisoned" }),
                    );
                    return;
                }
            };
            let Some(ref llm) = *guard else {
                let _ = app_send.emit(
                    "llm_error",
                    serde_json::json!({ "stream_id": sid, "error": "no model loaded" }),
                );
                return;
            };
            llm.0.eval(prompt, &mut opts)
        };
        match result {
            Ok(()) => {
                let _ = app_send.emit("llm_done", serde_json::json!({ "stream_id": sid }));
            }
            Err(e) => {
                let _ = app_send.emit(
                    "llm_error",
                    serde_json::json!({ "stream_id": sid, "error": e.to_string() }),
                );
            }
        }
    });

    // The id lets the caller match only its own events.
    Ok(stream_id)
}

/// Summarize text using the loaded model.
///
/// Mirrors `chat_stream`: tokens arrive as `summarize_stream` events carrying a
/// `stream_id`, and the run ends with `summarize_done` / `summarize_error`.
#[tauri::command]
pub async fn summarize(
    app: AppHandle,
    state: State<'_, Arc<LlamaState>>,
    req: SummarizeRequest,
) -> Result<String, String> {
    state
        .active_model_path()
        .ok_or_else(|| "no model loaded".to_string())?;

    let system_prompt =
        "You are a concise summarizer. Summarize the following text in a few clear sentences:";
    let prompt = format!("{}\n\n{}", system_prompt, req.text);
    let max_tokens = req.max_length.unwrap_or(256);

    let state_arc = state.inner().clone();
    let stream_id = next_stream_id();
    let sid = stream_id.clone();

    let app_send = app.clone();
    std::thread::spawn(move || {
        let mut opts = PredictOptions::default();
        opts.threads = 4;
        opts.tokens = max_tokens;
        opts.temperature = 0.3;
        opts.top_p = 0.9;
        opts.repeat = 64;
        opts.seed = -1;

        let app_tok = app_send.clone();
        let sid_tok = sid.clone();
        opts.set_token_callback(Some(Box::new(move |token: String| -> bool {
            let _ = app_tok.emit(
                "summarize_stream",
                serde_json::json!({ "stream_id": sid_tok, "token": token }),
            );
            true
        })));

        let result = {
            let guard = match state_arc.model.lock() {
                Ok(g) => g,
                Err(e) => {
                    log::error!("model mutex poisoned in summarize: {e}");
                    let _ = app_send.emit(
                        "summarize_error",
                        serde_json::json!({ "stream_id": sid, "error": "model mutex poisoned" }),
                    );
                    return;
                }
            };
            let Some(ref llm) = *guard else {
                let _ = app_send.emit(
                    "summarize_error",
                    serde_json::json!({ "stream_id": sid, "error": "no model loaded" }),
                );
                return;
            };
            llm.0.eval(prompt, &mut opts)
        };
        match result {
            Ok(()) => {
                let _ = app_send.emit("summarize_done", serde_json::json!({ "stream_id": sid }));
            }
            Err(e) => {
                let _ = app_send.emit(
                    "summarize_error",
                    serde_json::json!({ "stream_id": sid, "error": e.to_string() }),
                );
            }
        }
    });

    Ok(stream_id)
}
