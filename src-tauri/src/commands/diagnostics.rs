//! Local diagnostics. Every field here describes this
//! machine's own state; nothing is uploaded anywhere.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::inference::InferenceEngine;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Aggregate health snapshot of every local subsystem, for the Settings
/// diagnostics panel and the fatal-error screen's "Copy Diagnostics".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthResponse {
    pub app_version: String,
    pub catalog_version: u32,
    pub engine: EngineHealth,
    pub embedding: EmbeddingHealth,
    pub rerank: RerankHealth,
    pub library: LibraryHealth,
    /// True when config.json parsed on this call.
    pub config_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineHealth {
    pub loaded: bool,
    pub active_model: Option<String>,
    pub context_size: i32,
    /// Whether a llama-server binary can be located at all.
    pub sidecar_binary_found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankHealth {
    pub ready: bool,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingHealth {
    pub ready: bool,
    pub model_id: Option<String>,
    pub dimension: Option<usize>,
    pub total_chunks: usize,
    pub embedded_chunks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryHealth {
    pub doc_count: usize,
}

#[tauri::command]
pub fn system_health(state: State<'_, AppState>) -> Result<SystemHealthResponse, AppError> {
    let engine_status = state.inference_engine.status();
    let embedding_state = state.embedding_engine.state();
    let rerank_state = state.rerank_engine.state();
    let (total_chunks, embedded_chunks) = state.library.embedding_coverage()?;
    let stats = state.library.stats()?;
    let manifest = state.model_service.manifest();
    let config_ok = {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        store.load().is_ok()
    };

    Ok(SystemHealthResponse {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        catalog_version: manifest.version,
        engine: EngineHealth {
            loaded: engine_status.loaded,
            active_model: engine_status.active_model,
            context_size: engine_status.context_size,
            sidecar_binary_found: crate::services::inference::sidecar::locate_binary().is_ok(),
        },
        embedding: EmbeddingHealth {
            ready: embedding_state.ready,
            model_id: embedding_state.model_id,
            dimension: embedding_state.dimension,
            total_chunks,
            embedded_chunks,
        },
        rerank: RerankHealth {
            ready: rerank_state.ready,
            model_id: rerank_state.model_id,
        },
        library: LibraryHealth {
            doc_count: stats.doc_count as usize,
        },
        config_ok,
    })
}

/// Tail of a local log for the Settings diagnostics panel. `source` "app" is
/// LocalAide's own log; anything else is the model engine's log.
#[tauri::command]
pub fn logs_tail(lines: Option<usize>, source: Option<String>) -> Result<Vec<String>, AppError> {
    let file = match source.as_deref() {
        Some("app") => "localaide.log",
        _ => "llama-server.log",
    };
    let path = crate::app::paths::app_dir().join("logs").join(file);
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::storage(format!("Could not read log: {e}"))),
    };
    // Keep the tail only, so a huge log cannot push megabytes over IPC.
    const TAIL_CAP: usize = 256 * 1024;
    let start = data.len().saturating_sub(TAIL_CAP);
    let text = String::from_utf8_lossy(&data[start..]);
    let keep = lines.unwrap_or(200).min(1000);
    let mut collected: Vec<String> = text
        .lines()
        .rev()
        .take(keep)
        .map(|s| s.to_string())
        .collect();
    collected.reverse();
    Ok(collected)
}
