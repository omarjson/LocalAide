//! Chat engine that runs llama.cpp's `llama-server` as a child process.
//!
//! The server listens on 127.0.0.1 only, on a free port, and requires a random
//! API key on every chat request. The app starts it when a model loads and
//! stops it when a different model loads or the app exits.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use super::capabilities::{LocalProps, parse_props};
use super::{EngineStatus, GenerationMetrics, InferenceEngine, InferenceToken, PromptMessage};
use crate::app::error::AppError;
use crate::app::lifecycle;

/// Environment variable that overrides the server binary path. Used in development.
const ENV_BIN: &str = "LOCALAIDE_LLAMA_SERVER";
const BIN_NAME: &str = "llama-server";
const HEALTH_TIMEOUT: Duration = Duration::from_secs(180);
/// Health checks start fast and back off exponentially up to a cap, so a
/// quickly-starting server is detected in ~50 ms instead of a fixed 250 ms.
const POLL_INITIAL: Duration = Duration::from_millis(50);
const POLL_MAX: Duration = Duration::from_millis(500);
/// Whole-request timeout for chat. Generation can run for minutes on a large model.
const CHAT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// How often the reply loop checks the cancel flag while the server is busy.
const STREAM_POLL: Duration = Duration::from_millis(200);
/// Longest silence from the server before a reply counts as stuck. Prompt
/// processing on a large context can take minutes, so the limit is generous.
const STREAM_STALL_LIMIT: Duration = Duration::from_secs(10 * 60);
/// The server log is rotated once it passes this size. One old copy is kept.
const MAX_SERVER_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Double a polling delay, clamped at `max`. Pure so it can be unit tested.
pub fn next_backoff(current: Duration, max: Duration) -> Duration {
    current.saturating_mul(2).min(max)
}

struct Running {
    child: Child,
    port: u16,
    api_key: String,
    model_path: PathBuf,
    context_size: i32,
    /// True when the model has a vision projector, so it can read images.
    has_mmproj: bool,
}

/// The running server's address and key, for the shared chat client.
#[derive(Clone)]
pub struct LocalEndpoint {
    pub port: u16,
    pub api_key: String,
    pub has_mmproj: bool,
    pub context_size: i32,
    pub model_path: PathBuf,
}

pub struct SidecarEngine {
    running: Mutex<Option<Running>>,
    /// `/props` of the loaded model, keyed by its path.
    props_cache: Mutex<Option<(PathBuf, LocalProps)>>,
    client: reqwest::blocking::Client,
    /// Held for a whole load, so two loads never run two servers at once.
    load_lock: Mutex<()>,
    /// Bumped by `unload`. A load that started before the bump discards its server.
    epoch: AtomicU64,
}

impl SidecarEngine {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(CHAT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            running: Mutex::new(None),
            props_cache: Mutex::new(None),
            client,
            load_lock: Mutex::new(()),
            epoch: AtomicU64::new(0),
        }
    }

    /// Kill the running server, if any, and wait for it to exit.
    fn stop(&self) {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut r) = guard.take() {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    /// Where the running server listens, for the shared chat client.
    pub fn endpoint(&self) -> Option<LocalEndpoint> {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let r = guard.as_mut()?;
        if matches!(r.child.try_wait(), Ok(Some(_)) | Err(_)) {
            *guard = None;
            return None;
        }
        Some(LocalEndpoint {
            port: r.port,
            api_key: r.api_key.clone(),
            has_mmproj: r.has_mmproj,
            context_size: r.context_size,
            model_path: r.model_path.clone(),
        })
    }

    /// What the server says about its model (`GET /props`): the chat
    /// template's thinking and tool support, and vision. Read once per load.
    pub fn props(&self) -> Option<LocalProps> {
        let ep = self.endpoint()?;
        {
            let cache = self.props_cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((path, props)) = cache.as_ref()
                && *path == ep.model_path
            {
                return Some(props.clone());
            }
        }
        let json: Value = self
            .client
            .get(format!("http://127.0.0.1:{}/props", ep.port))
            .bearer_auth(&ep.api_key)
            .timeout(Duration::from_secs(3))
            .send()
            .ok()
            .filter(|r| r.status().is_success())?
            .json()
            .ok()?;
        let props = parse_props(&json);
        *self.props_cache.lock().unwrap_or_else(|e| e.into_inner()) =
            Some((ep.model_path, props.clone()));
        Some(props)
    }

    fn health_ok(&self, port: u16) -> bool {
        self.client
            .get(format!("http://127.0.0.1:{port}/health"))
            .timeout(Duration::from_secs(2))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

impl Default for SidecarEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SidecarEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

impl InferenceEngine for SidecarEngine {
    fn load(&self, model_path: &Path, context_size: i32) -> Result<String, AppError> {
        // One load at a time: two servers at once would double the RAM use.
        let _serial = self.load_lock.lock().unwrap_or_else(|e| e.into_inner());
        let epoch = self.epoch.load(Ordering::SeqCst);
        self.stop();

        let bin = locate_binary()?;
        let port = free_port()?;
        let api_key = new_api_key();
        let model_id = model_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut cmd = Command::new(&bin);
        cmd.arg("-m")
            .arg(model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-c")
            .arg(context_size.to_string())
            .arg("--jinja")
            // The key goes in the environment, not the command line, which
            // other programs on this computer can read.
            .env("LLAMA_API_KEY", &api_key)
            .stdin(Stdio::null());

        let mmproj = mmproj_for(model_path);
        if let Some(path) = &mmproj {
            cmd.arg("--mmproj").arg(path);
        }

        match open_log() {
            Some((out, err)) => {
                cmd.stdout(Stdio::from(out)).stderr(Stdio::from(err));
            }
            None => {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }

        lifecycle::hide_console_window(&mut cmd);
        let child = cmd.spawn().map_err(|e| {
            AppError::model_load_failed(&model_id, &format!("failed to start llama-server: {e}"))
        })?;
        lifecycle::tie_child_to_app(&child);

        let mut running = Running {
            child,
            port,
            api_key,
            model_path: model_path.to_path_buf(),
            context_size,
            has_mmproj: mmproj.is_some(),
        };

        let deadline = Instant::now() + HEALTH_TIMEOUT;
        let mut backoff = POLL_INITIAL;
        loop {
            if let Ok(Some(status)) = running.child.try_wait() {
                return Err(AppError::model_load_failed(
                    &model_id,
                    &startup_exit_message(&status),
                ));
            }
            if self.health_ok(running.port) {
                break;
            }
            if Instant::now() >= deadline {
                let _ = running.child.kill();
                let _ = running.child.wait();
                return Err(AppError::model_load_failed(
                    &model_id,
                    "llama-server did not become ready in time",
                ));
            }
            std::thread::sleep(backoff);
            backoff = next_backoff(backoff, POLL_MAX);
        }

        // An unload during this load means the user wants no model: drop the new server.
        if self.epoch.load(Ordering::SeqCst) != epoch {
            let _ = running.child.kill();
            let _ = running.child.wait();
            return Err(AppError::cancelled());
        }
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        // Replacing a running server would orphan it, so stop the old one first.
        if let Some(mut previous) = guard.replace(running) {
            let _ = previous.child.kill();
            let _ = previous.child.wait();
        }
        Ok(format!("Successfully loaded {}", model_path.display()))
    }

    fn unload(&self) -> Result<(), AppError> {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.stop();
        Ok(())
    }

    fn generate(
        &self,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        mut on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError> {
        let (port, api_key, has_mmproj) = {
            let guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
            let r = guard.as_ref().ok_or_else(|| {
                AppError::model_not_installed("No model loaded in inference engine")
            })?;
            (r.port, r.api_key.clone(), r.has_mmproj)
        };

        let has_images = messages.iter().any(|m| !m.images.is_empty());
        if has_images && !has_mmproj {
            return Err(AppError::invalid_input(
                "The loaded model cannot read images. Choose a model marked (vision).",
            ));
        }

        // The server applies the model's own chat template to the message list.
        // Build the body from borrowed `&str` slices so base64 images are
        // serialized straight into the HTTP body, not cloned into a Value first.
        let body = wire_body(messages, max_tokens, temperature);

        let start = Instant::now();
        let resp = self
            .client
            .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .map_err(|e| AppError::internal(format!("chat request failed: {e}")))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(AppError::internal(format!(
                "llama-server returned {code}: {text}"
            )));
        }

        let mut total_tokens = 0usize;
        let mut ttft: Option<Duration> = None;
        let mut cancelled = false;

        // A helper thread reads the stream, so this loop can check the cancel flag
        // every STREAM_POLL. Without it, Stop does nothing while the server is still
        // processing the prompt and has sent no line. When this function returns, the
        // receiver is dropped. The helper then stops at its next line and closes the
        // connection, which tells the server to stop generating.
        let (lines_tx, lines_rx) = std::sync::mpsc::channel::<std::io::Result<String>>();
        std::thread::spawn(move || {
            for line in BufReader::new(resp).lines() {
                if lines_tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut last_activity = Instant::now();
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let line = match lines_rx.recv_timeout(STREAM_POLL) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    if last_activity.elapsed() > STREAM_STALL_LIMIT {
                        return Err(AppError::internal(
                            "The model stopped responding. Try again, or load the model again.",
                        ));
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            last_activity = Instant::now();
            let line = line.map_err(|e| AppError::internal(format!("stream read failed: {e}")))?;
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data.trim() == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            let Some(text) = v["choices"][0]["delta"]["content"].as_str() else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            if ttft.is_none() {
                ttft = Some(start.elapsed());
            }
            on_token(InferenceToken {
                token: text.to_string(),
                index: total_tokens,
            });
            total_tokens += 1;
        }

        let elapsed = start.elapsed().as_secs_f64();
        let tokens_per_sec = if elapsed > 0.0 {
            total_tokens as f64 / elapsed
        } else {
            0.0
        };

        Ok(GenerationMetrics {
            total_tokens,
            ttft,
            tokens_per_sec,
            cancelled,
        })
    }

    fn status(&self) -> EngineStatus {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let exited = match guard.as_mut() {
            Some(r) => matches!(r.child.try_wait(), Ok(Some(_)) | Err(_)),
            None => true,
        };
        if exited {
            *guard = None;
        }
        match guard.as_ref() {
            Some(r) => EngineStatus {
                loaded: true,
                active_model: Some(r.model_path.to_string_lossy().to_string()),
                context_size: r.context_size,
            },
            None => EngineStatus {
                loaded: false,
                active_model: None,
                context_size: 2048,
            },
        }
    }
}

/// Find the `llama-server` binary: the env override first, then next to the app executable.
pub(crate) fn locate_binary() -> Result<PathBuf, AppError> {
    if let Ok(p) = std::env::var(ENV_BIN) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Ok(path);
        }
    }
    // The build LocalAide downloaded for this computer's processor.
    if let Some(path) = crate::services::externals::llama_binary() {
        return Ok(path);
    }
    // A server placed next to the app by hand (development, older installs).
    let exe = std::env::current_exe().map_err(|e| AppError::internal(e.to_string()))?;
    let dir = exe
        .parent()
        .ok_or_else(|| AppError::internal("cannot locate the app folder"))?;
    for name in [BIN_NAME.to_string(), format!("{BIN_NAME}.exe")] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(AppError::new(
        "ENGINE_MISSING",
        "The model engine is not on this computer yet. LocalAide downloads it (about 25 MB) the first time a model loads.",
    ))
}

/// Why llama-server stopped while starting, in words. Exit code 0xC000001D
/// (Windows) or SIGILL (signal 4) means the processor lacks an instruction
/// the build needs.
pub(crate) fn startup_exit_message(status: &std::process::ExitStatus) -> String {
    let illegal = status.code().map(|c| c as u32) == Some(0xC000_001D) || {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            status.signal() == Some(4)
        }
        #[cfg(not(unix))]
        {
            false
        }
    };
    if illegal {
        format!(
            "llama-server stopped at once ({status}): this processor does not support an instruction the model engine uses. If you set {ENV_BIN} or put a llama-server next to the app, remove it so LocalAide uses the build it downloads for this processor. See logs/llama-server.log."
        )
    } else {
        format!("llama-server exited during startup ({status}). See logs/llama-server.log.")
    }
}

/// Pick a free loopback port. There is a small race before the server binds it.
pub(crate) fn free_port() -> Result<u16, AppError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|e| AppError::internal(format!("cannot pick a port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::internal(e.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

/// A 256-bit random key, hex encoded, drawn from the OS random source
/// (BCryptGenRandom on Windows, getrandom elsewhere).
pub(crate) fn new_api_key() -> String {
    let mut bytes = [0u8; 32];
    if getrandom::fill(&mut bytes).is_err() {
        // Practically unreachable on the desktop targets. Fall back to
        // process-unique hashing so startup never panics over a key.
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        for (i, chunk) in bytes.chunks_mut(8).enumerate() {
            let mut h = RandomState::new().build_hasher();
            h.write_u8(i as u8);
            chunk.copy_from_slice(&h.finish().to_le_bytes());
        }
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Vision projector that sits next to a model: `{id}.gguf` pairs with `{id}.mmproj.gguf`.
fn mmproj_for(model_path: &Path) -> Option<PathBuf> {
    let stem = model_path.file_stem()?.to_string_lossy().to_string();
    let candidate = model_path.with_file_name(format!("{stem}.mmproj.gguf"));
    candidate.is_file().then_some(candidate)
}

/// Open the server log in the app folder for stdout and stderr.
pub(crate) fn open_log() -> Option<(File, File)> {
    let dir = crate::app::paths::app_dir().join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("llama-server.log");
    // One old copy is kept, so a long session cannot grow the log without limit.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_SERVER_LOG_BYTES) {
        let _ = std::fs::rename(&path, dir.join("llama-server.log.old"));
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    let second = file.try_clone().ok()?;
    Some((file, second))
}

/// Convert one chat turn to the OpenAI-style wire format. Images become content parts.
/// Retained for the shared remote client, which still builds a `Value` body.
pub(crate) fn wire_message(m: &PromptMessage) -> Value {
    if m.images.is_empty() {
        return json!({ "role": m.role, "content": m.content });
    }
    let mut parts = vec![json!({ "type": "text", "text": m.content })];
    for img in &m.images {
        parts.push(json!({ "type": "image_url", "image_url": { "url": img } }));
    }
    json!({ "role": m.role, "content": parts })
}

/// Borrowed serializable view of the chat request. Every string here is a
/// `&str` into the caller's messages, so serializing the body copies each
/// base64 image once (into the HTTP buffer) instead of once into a `Value`
/// and again when the `Value` is serialized.
#[derive(Serialize)]
struct WireBody<'a> {
    messages: Vec<WireMessage<'a>>,
    max_tokens: i32,
    temperature: f32,
    top_p: f32,
    stream: bool,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'a str,
    content: WireContent<'a>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum WireContent<'a> {
    Text(&'a str),
    Parts(Vec<WirePart<'a>>),
}

#[derive(Serialize)]
struct WirePart<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_url: Option<WireImageUrl<'a>>,
}

#[derive(Serialize)]
struct WireImageUrl<'a> {
    url: &'a str,
}

fn wire_body(
    messages: &[PromptMessage],
    max_tokens: i32,
    temperature: f32,
) -> WireBody<'_> {
    let wire_messages = messages
        .iter()
        .map(|m| {
            let content = if m.images.is_empty() {
                WireContent::Text(m.content.as_str())
            } else {
                let mut parts = Vec::with_capacity(1 + m.images.len());
                parts.push(WirePart {
                    kind: "text",
                    text: Some(m.content.as_str()),
                    image_url: None,
                });
                for img in &m.images {
                    parts.push(WirePart {
                        kind: "image_url",
                        text: None,
                        image_url: Some(WireImageUrl { url: img.as_str() }),
                    });
                }
                WireContent::Parts(parts)
            };
            WireMessage {
                role: m.role.as_str(),
                content,
            }
        })
        .collect();
    WireBody {
        messages: wire_messages,
        max_tokens,
        temperature,
        top_p: 0.9,
        stream: true,
    }
}
