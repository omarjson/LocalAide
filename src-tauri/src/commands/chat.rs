use crate::app::blocking::{run_blocking, with_app_state};
use crate::app::error::AppError;
use crate::app::events::ChatStreamEvent;
use crate::app::paths::conversations_dir;
use crate::app::state::AppState;
use crate::domain::chat::{
    ChatMessage, ChatStreamRequest, ChatThread, SummarizeRequest, SummarizeResponse,
    ThreadExportRequest, ThreadExportResponse,
};
use crate::services::inference::InferenceEngine;
use crate::services::inference::PromptMessage;
use crate::services::inference::context::ContextBudget;
use std::fs;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::State;

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredThread {
    pub id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<ChatMessage>,
}

const MAX_IMAGES: usize = 4;
const MAX_IMAGES_BYTES: usize = 20 * 1024 * 1024;
/// How long a chat waits for the query embedding before it falls back to keyword recall.
const RECALL_EMBED_LIMIT: Duration = Duration::from_secs(3);

/// Reject unsupported or oversized attachments before any model work starts.
fn validate_images(images: &[String]) -> Result<(), AppError> {
    if images.len() > MAX_IMAGES {
        return Err(AppError::invalid_input(format!(
            "Attach at most {MAX_IMAGES} images."
        )));
    }
    const PREFIXES: [&str; 4] = [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
        "data:image/gif;base64,",
    ];
    let mut total = 0usize;
    for img in images {
        if !PREFIXES.iter().any(|p| img.starts_with(p)) {
            return Err(AppError::invalid_input(
                "Only PNG, JPEG, WEBP and GIF images are supported.",
            ));
        }
        total += img.len();
    }
    if total > MAX_IMAGES_BYTES {
        return Err(AppError::invalid_input(
            "The attached images are too large. Keep them under 20 MB in total.",
        ));
    }
    Ok(())
}

/// Thread ids become file names in the conversations folder, so only a short
/// plain token is accepted. The app issues ids like `t_1731542400000`.
fn validate_thread_id(thread_id: &str) -> Result<(), AppError> {
    let plain = !thread_id.is_empty()
        && thread_id.len() <= 64
        && thread_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if plain {
        Ok(())
    } else {
        Err(AppError::invalid_input(
            "That conversation id is not valid.",
        ))
    }
}

/// Runs `work` on a helper thread and waits at most `limit` for its answer. A
/// slow answer returns None, and the helper finishes in the background.
fn answer_within<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> Option<T> + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx.recv_timeout(limit).ok().flatten()
}

/// What the chat screen needs to know about the local engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InferenceStatusDto {
    pub loaded: bool,
    pub loading: bool,
}

#[tauri::command]
pub fn inference_status(state: State<'_, AppState>) -> InferenceStatusDto {
    InferenceStatusDto {
        loaded: state.inference_engine.status().loaded,
        loading: state.engine_loading.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub async fn inference_load(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    load_and_activate(&state, id).await
}

/// Load a model into the local engine, then mark it active and remember it in
/// the selection. The "Use this" button, the chat picker and the preset install
/// all come through here.
pub(crate) async fn load_and_activate(state: &AppState, id: String) -> Result<String, AppError> {
    // Filesystem truth: the model must exist in the active catalog or the
    // community registry AND on disk. This keeps freshly downloaded models
    // loadable and honors split-GGUF dest names.
    state
        .model_service
        .get_model_entry(&id)
        .ok_or_else(|| AppError::model_not_installed(&id))?;
    let path = state.model_service.model_file_path(&id);
    if !path.is_file() {
        return Err(AppError::model_not_installed(&id));
    }
    // The engine uses a blocking HTTP client. Run it on a plain thread, not the async runtime.
    let engine = state.inference_engine.clone();
    let load_path = path.clone();
    // Context is the user's setting capped by the model's native window.
    let context = {
        let setting = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))
            .and_then(|s| s.load().map_err(AppError::internal))
            .map(|c| c.context_tokens)
            .unwrap_or(8192);
        state.model_service.effective_context(&id, setting)
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(engine.load(&load_path, context));
    });
    let res = rx.await.map_err(|e| AppError::internal(e.to_string()))??;
    if let Err(e) = state.model_manager.activate(&id) {
        log::warn!("Could not save the active model {id}: {e}");
    }

    // Persist active model in config for auto-reload
    if let Ok(store) = state.config_store.lock()
        && let Ok(mut cfg) = store.load()
    {
        if !cfg.selected_models.contains(&id) {
            cfg.selected_models.push(id.clone());
        }
        let _ = store.save(&cfg);
    }

    Ok(res)
}

/// Stop the local engine, free the model's RAM and forget the active model.
/// The model file stays installed. Loading it again restarts the engine.
#[tauri::command]
pub async fn inference_unload(state: State<'_, AppState>) -> Result<(), AppError> {
    with_app_state(&state, |state| {
        state.inference_engine.unload()?;
        if let Err(e) = state.model_manager.clear_active() {
            log::warn!("Could not clear the active model: {e}");
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn chat_stream(
    state: State<'_, AppState>,
    req: ChatStreamRequest,
    channel: tauri::ipc::Channel<ChatStreamEvent>,
) -> Result<String, AppError> {
    let status = state.inference_engine.status();
    if !status.loaded {
        return Err(AppError::model_not_installed(
            "No model loaded in inference engine. Please select and activate a model.",
        ));
    }

    let max_tokens = req.max_tokens.unwrap_or(512);
    let temperature = req.temperature.unwrap_or(0.7);

    validate_images(&req.images)?;

    // RAG Recall: Retrieve context from MemoryService / LibraryService. The
    // query embeds against the local engine when it is running, so hybrid
    // ranking uses vectors; without it recall is keyword-ranked.
    // Recall runs on a worker thread, so a busy engine cannot stall the window.
    // The embedding is used only if it answers in time; otherwise recall is keyword-ranked.
    let recall_text = req.message.clone();
    let rag_res = with_app_state(&state, move |owned| {
        let engine = owned.embedding_engine.clone();
        let vector_text = recall_text.clone();
        let query_vector = answer_within(RECALL_EMBED_LIMIT, move || {
            engine.embed_query(&vector_text).ok()
        });
        Ok(owned
            .memory
            .recall(&recall_text, query_vector.as_deref(), Some(512))
            .unwrap_or_else(|_| crate::services::memory::rag::RagRecallResult {
                citations: Vec::new(),
                context_prompt: String::new(),
                total_tokens: 0,
            }))
    })
    .await?;

    // Send citation events to frontend channel before generation
    for cit in &rag_res.citations {
        let _ = channel.send(ChatStreamEvent::Citation {
            index: cit.index,
            source: cit.source.clone(),
            title: cit.title.clone(),
            snippet: cit.snippet.clone(),
            score: cit.score,
        });
    }

    // Build effective system prompt with RAG context
    let effective_system_prompt = match req.system_prompt.as_deref() {
        Some(sys) if !rag_res.context_prompt.is_empty() => {
            Some(format!("{sys}\n{}", rag_res.context_prompt))
        }
        None if !rag_res.context_prompt.is_empty() => Some(rag_res.context_prompt),
        Some(sys) => Some(sys.to_string()),
        None => None,
    };

    // Apply context budget
    let budget = ContextBudget::new(status.context_size as usize, max_tokens as usize);
    let messages = budget.build_messages(
        effective_system_prompt.as_deref(),
        &req.history,
        &req.message,
        &req.images,
    );

    let (job_id, cancel_flag) = state.jobs.create_job("inference_chat");
    let engine = state.inference_engine.clone();
    let jobs = state.jobs.clone();
    let jid = job_id.clone();

    std::thread::spawn(move || {
        let ch_tok = channel.clone();
        let eval_res = engine.generate(
            &messages,
            max_tokens,
            temperature,
            cancel_flag,
            Box::new(move |tok| {
                let _ = ch_tok.send(ChatStreamEvent::Token { token: tok.token });
            }),
        );

        match eval_res {
            Ok(metrics) => {
                jobs.complete_job(&jid);
                let _ = channel.send(ChatStreamEvent::Done {
                    finish_reason: if metrics.cancelled {
                        "cancelled".to_string()
                    } else {
                        "stop".to_string()
                    },
                    total_tokens: metrics.total_tokens,
                });
            }
            Err(e) => {
                jobs.fail_job(&jid, &e.message);
                let _ = channel.send(ChatStreamEvent::Error { error: e.message });
            }
        }
    });

    Ok(job_id)
}

#[tauri::command]
pub fn chat_cancel(state: State<'_, AppState>, job_id: String) -> Result<(), AppError> {
    state.jobs.cancel_job(&job_id);
    Ok(())
}

#[tauri::command]
pub async fn summarize(
    state: State<'_, AppState>,
    req: SummarizeRequest,
) -> Result<SummarizeResponse, AppError> {
    let status = state.inference_engine.status();
    if !status.loaded {
        return Err(AppError::model_not_installed(
            "No model loaded for summarization.",
        ));
    }

    let system_prompt =
        "You are a concise summarizer. Summarize the following text in a few clear sentences:";
    let messages = vec![
        PromptMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            images: Vec::new(),
        },
        PromptMessage {
            role: "user".to_string(),
            content: req.text.clone(),
            images: Vec::new(),
        },
    ];
    let max_tokens = req.max_words.unwrap_or(256) as i32;

    let (_job_id, cancel_flag) = state.jobs.create_job("summarize");
    let accumulated = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let acc_cb = accumulated.clone();

    // Run on a plain thread: the engine uses a blocking HTTP client.
    let engine = state.inference_engine.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = engine.generate(
            &messages,
            max_tokens,
            0.3,
            cancel_flag,
            Box::new(move |tok| {
                if let Ok(mut lock) = acc_cb.lock() {
                    lock.push_str(&tok.token);
                }
            }),
        );
        let _ = tx.send(result.map(|_| ()));
    });
    rx.await.map_err(|e| AppError::internal(e.to_string()))??;

    let summary = accumulated
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(SummarizeResponse { summary })
}

#[tauri::command]
pub async fn chat_thread_list() -> Result<Vec<ChatThread>, AppError> {
    run_blocking(chat_thread_list_work).await
}

fn chat_thread_list_work() -> Result<Vec<ChatThread>, AppError> {
    let dir = conversations_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut list = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json")
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(stored) = serde_json::from_str::<StoredThread>(&content)
        {
            list.push(ChatThread {
                id: stored.id,
                title: stored.title,
                system_prompt: stored.system_prompt,
                created_at: stored.created_at,
                updated_at: stored.updated_at,
                message_count: stored.messages.len(),
            });
        }
    }
    list.sort_by_key(|t| std::cmp::Reverse(t.updated_at));
    Ok(list)
}

#[tauri::command]
pub async fn chat_thread_save(
    thread_id: String,
    title: String,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
) -> Result<(), AppError> {
    run_blocking(move || chat_thread_save_work(thread_id, title, system_prompt, messages)).await
}

fn chat_thread_save_work(
    thread_id: String,
    title: String,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
) -> Result<(), AppError> {
    validate_thread_id(&thread_id)?;
    let dir = conversations_dir();
    fs::create_dir_all(&dir)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let path = dir.join(format!("{thread_id}.json"));
    let created_at = if path.exists() {
        if let Ok(existing) = fs::read_to_string(&path) {
            serde_json::from_str::<StoredThread>(&existing)
                .map(|t| t.created_at)
                .unwrap_or(now)
        } else {
            now
        }
    } else {
        now
    };

    let stored = StoredThread {
        id: thread_id,
        title,
        system_prompt,
        created_at,
        updated_at: now,
        messages,
    };

    let json = serde_json::to_string_pretty(&stored).map_err(AppError::from)?;
    fs::write(&path, json)?;
    Ok(())
}

#[tauri::command]
pub async fn chat_thread_load(thread_id: String) -> Result<Vec<ChatMessage>, AppError> {
    run_blocking(move || chat_thread_load_work(thread_id)).await
}

fn chat_thread_load_work(thread_id: String) -> Result<Vec<ChatMessage>, AppError> {
    validate_thread_id(&thread_id)?;
    let path = conversations_dir().join(format!("{thread_id}.json"));
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(&path)?;
    let stored: StoredThread = serde_json::from_str(&data).map_err(AppError::from)?;
    Ok(stored.messages)
}

#[tauri::command]
pub async fn chat_thread_delete(thread_id: String) -> Result<(), AppError> {
    run_blocking(move || chat_thread_delete_work(thread_id)).await
}

fn chat_thread_delete_work(thread_id: String) -> Result<(), AppError> {
    validate_thread_id(&thread_id)?;
    let path = conversations_dir().join(format!("{thread_id}.json"));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_thread_export(
    req: ThreadExportRequest,
) -> Result<ThreadExportResponse, AppError> {
    run_blocking(move || chat_thread_export_work(req)).await
}

fn chat_thread_export_work(req: ThreadExportRequest) -> Result<ThreadExportResponse, AppError> {
    validate_thread_id(&req.thread_id)?;
    let path = conversations_dir().join(format!("{}.json", req.thread_id));
    if !path.exists() {
        return Err(AppError::invalid_input("Thread not found"));
    }
    let data = fs::read_to_string(&path)?;
    let stored: StoredThread = serde_json::from_str(&data).map_err(AppError::from)?;

    if req.format.to_lowercase() == "json" {
        Ok(ThreadExportResponse {
            content: data,
            filename: format!("{}.json", stored.title.replace(' ', "_")),
        })
    } else {
        // Markdown format
        let mut md = format!("# {}\n\n", stored.title);
        if let Some(sys) = stored.system_prompt
            && !sys.trim().is_empty()
        {
            md.push_str(&format!("**System Prompt:** {}\n\n---\n\n", sys));
        }
        for msg in stored.messages {
            let role_label = if msg.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            md.push_str(&format!("### {}\n\n{}\n\n", role_label, msg.content));
        }
        Ok(ThreadExportResponse {
            content: md,
            filename: format!("{}.md", stored.title.replace(' ', "_")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{answer_within, validate_thread_id};
    use std::time::Duration;

    #[test]
    fn answer_within_gives_up_on_slow_work() {
        assert_eq!(answer_within(Duration::from_secs(5), || Some(7)), Some(7));
        let slow = answer_within(Duration::from_millis(50), || {
            std::thread::sleep(Duration::from_millis(500));
            Some(7)
        });
        assert_eq!(slow, None);
    }

    #[test]
    fn thread_ids_must_be_plain_tokens() {
        assert!(validate_thread_id("t_1731542400000").is_ok());
        assert!(validate_thread_id("a-b_c").is_ok());
        for bad in ["", "..", "../config", "..\\config", "a.b", "a b", "C:x"] {
            assert!(validate_thread_id(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(validate_thread_id(&"a".repeat(65)).is_err());
    }
}
