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
use std::path::Path;
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

#[tauri::command]
pub async fn inference_load(state: State<'_, AppState>, id: String) -> Result<String, AppError> {
    let entry = state
        .model_manager
        .list_models()
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| AppError::model_not_installed(&id))?;

    if !entry.installed {
        return Err(AppError::model_not_installed(&id));
    }
    let path = state.model_manager.primary_path(&id);
    // The engine uses a blocking HTTP client. Run it on a plain thread, not the async runtime.
    let engine = state.inference_engine.clone();
    let load_path = path.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(engine.load(&load_path, 2048));
    });
    let res = rx.await.map_err(|e| AppError::internal(e.to_string()))??;
    let _ = state.model_manager.activate(&id);

    // Persist active model in config for auto-reload
    if let Ok(store) = state.config_store.lock() {
        if let Ok(mut cfg) = store.load() {
            if !cfg.selected_models.contains(&id) {
                cfg.selected_models.push(id.clone());
            }
            let _ = store.save(&cfg);
        }
    }

    Ok(res)
}

/// Stop the local engine and free the model's RAM. The model stays installed
/// and active in the catalog; loading it again restarts the sidecar.
#[tauri::command]
pub fn inference_unload(state: State<'_, AppState>) -> Result<(), AppError> {
    state.inference_engine.unload()
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

    // RAG Recall: Retrieve context from MemoryService / LibraryService
    let rag_res = state
        .memory
        .recall(&req.message, Some(512))
        .unwrap_or_else(|_| crate::services::memory::rag::RagRecallResult {
            citations: Vec::new(),
            context_prompt: String::new(),
            total_tokens: 0,
        });

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
pub fn chat_thread_list() -> Result<Vec<ChatThread>, AppError> {
    let dir = conversations_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut list = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(stored) = serde_json::from_str::<StoredThread>(&content) {
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
        }
    }
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(list)
}

#[tauri::command]
pub fn chat_thread_save(
    thread_id: String,
    title: String,
    system_prompt: Option<String>,
    messages: Vec<ChatMessage>,
) -> Result<(), AppError> {
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
pub fn chat_thread_load(thread_id: String) -> Result<Vec<ChatMessage>, AppError> {
    let path = conversations_dir().join(format!("{thread_id}.json"));
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(&path)?;
    let stored: StoredThread = serde_json::from_str(&data).map_err(AppError::from)?;
    Ok(stored.messages)
}

#[tauri::command]
pub fn chat_thread_delete(thread_id: String) -> Result<(), AppError> {
    let path = conversations_dir().join(format!("{thread_id}.json"));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn chat_thread_export(req: ThreadExportRequest) -> Result<ThreadExportResponse, AppError> {
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
        if let Some(sys) = stored.system_prompt {
            if !sys.trim().is_empty() {
                md.push_str(&format!("**System Prompt:** {}\n\n---\n\n", sys));
            }
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
