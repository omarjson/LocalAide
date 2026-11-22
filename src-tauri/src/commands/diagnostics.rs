//! Local diagnostics. Every field here describes this
//! machine's own state; nothing is uploaded anywhere.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::inference::InferenceEngine;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;
use sysinfo::System;
use tauri::State;

/// Set on the first health call, so uptime describes this process, not the
/// time since the machine booted.
static FIRST_HEALTH_CALL: OnceLock<Instant> = OnceLock::new();

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Cheap counters for the Settings → About performance panel. These are
/// byte counts and seconds only: no paths, model names or user data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceHealth {
    /// Seconds since the app's first health snapshot.
    pub uptime_secs: u64,
    /// Resident memory of the LocalAide process, in bytes.
    pub rss_bytes: u64,
    /// Combined size of the app and engine log files, in bytes.
    pub logs_bytes: u64,
    /// Size of the local library database, in bytes.
    pub library_db_bytes: u64,
}

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
    /// Counters for the performance panel; no user-visible strings.
    pub performance: PerformanceHealth,
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
    let uptime_secs = FIRST_HEALTH_CALL
        .get_or_init(Instant::now)
        .elapsed()
        .as_secs();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    let rss_bytes = sys.process(pid).map(|p| p.memory()).unwrap_or(0);
    let log_dir = crate::app::paths::app_dir().join("logs");
    let logs_bytes =
        file_len(&log_dir.join("localaide.log")) + file_len(&log_dir.join("llama-server.log"));
    let library_db_bytes = file_len(&crate::app::paths::index_dir().join("library.db"));

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
        performance: PerformanceHealth {
            uptime_secs,
            rss_bytes,
            logs_bytes,
            library_db_bytes,
        },
        config_ok,
    })
}

/// The model engine: whether it is on this computer, and which build.
#[tauri::command]
pub fn engine_info() -> crate::services::engine::EngineInfo {
    crate::services::engine::info()
}

/// Download the model engine now, instead of at the first model load.
#[tauri::command]
pub async fn engine_fetch() -> Result<crate::services::engine::EngineInfo, AppError> {
    crate::services::engine::ensure().await?;
    Ok(crate::services::engine::info())
}

/// Byte offset at which the last `cap` bytes of a `file_len`-byte file begin.
/// Pure math so it can be unit tested without touching the filesystem.
pub fn tail_window_start(file_len: u64, cap: u64) -> u64 {
    file_len.saturating_sub(cap)
}

/// How many leading bytes are UTF-8 continuation bytes (`10xxxxxx`).
/// Skipping these reaches the next character boundary when a window starts
/// in the middle of a multi-byte character.
pub fn utf8_leading_continuations(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|&&b| (b & 0xC0) == 0x80)
        .count()
}

/// Tail of a local log for the Settings diagnostics panel. `source` "app" is
/// LocalAide's own log; anything else is the model engine's log.
#[tauri::command]
pub fn logs_tail(lines: Option<usize>, source: Option<String>) -> Result<Vec<String>, AppError> {
    const TAIL_CAP: u64 = 256 * 1024;
    let file = match source.as_deref() {
        Some("app") => "localaide.log",
        _ => "llama-server.log",
    };
    let path = crate::app::paths::app_dir().join("logs").join(file);
    let mut handle = match std::fs::File::open(&path) {
        Ok(handle) => handle,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(AppError::storage(format!("Could not read log: {e}"))),
    };
    // Seek to the tail window instead of reading a whole (up to 5 MB) file.
    let len = handle
        .metadata()
        .map(|m| m.len())
        .map_err(|e| AppError::storage(format!("Could not read log: {e}")))?;
    let start = tail_window_start(len, TAIL_CAP);
    handle
        .seek(SeekFrom::Start(start))
        .map_err(|e| AppError::storage(format!("Could not seek log: {e}")))?;
    // The window length is bounded by TAIL_CAP (+1 byte in case the file grew
    // between metadata() and read_to_end), so this never buffers the whole file.
    let mut buf = Vec::with_capacity((len - start) as usize + 1);
    handle
        .read_to_end(&mut buf)
        .map_err(|e| AppError::storage(format!("Could not read log: {e}")))?;
    if start > 0 {
        let skip = utf8_leading_continuations(&buf);
        if skip > 0 {
            buf.drain(..skip);
        }
    }
    let text = String::from_utf8_lossy(&buf);
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
