//! Local diagnostics and the update check. Every field here describes this
//! machine's own state; nothing is uploaded anywhere.

use crate::app::error::AppError;
use crate::app::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

const RELEASES_API: &str = "https://api.github.com/repos/omarjson/LocalAide/releases/latest";

/// Aggregate health snapshot of every local subsystem, for the Settings
/// diagnostics panel and the fatal-error screen's "Copy Diagnostics".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthResponse {
    pub app_version: String,
    pub catalog_version: u32,
    pub engine: EngineHealth,
    pub embedding: EmbeddingHealth,
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
        library: LibraryHealth {
            doc_count: stats.doc_count,
        },
        config_ok,
    })
}

/// Check GitHub Releases for a newer version. Display-only: the app never
/// downloads or installs anything by itself, and offline just yields
/// `update_available: false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResponse {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: String,
}

#[tauri::command]
pub async fn update_check() -> Result<UpdateCheckResponse, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::internal(e.to_string()))?;

    let response = client
        .get(RELEASES_API)
        .header("User-Agent", "LocalAide-update-check")
        .send()
        .await;
    let latest = match response {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| AppError::internal(e.to_string()))?;
            body["tag_name"]
                .as_str()
                .map(|t| t.trim_start_matches('v').to_string())
        }
        _ => None,
    };

    let update_available = latest
        .as_ref()
        .map(|latest| version_newer(&current, latest))
        .unwrap_or(false);

    Ok(UpdateCheckResponse {
        current_version: current,
        latest_version: latest,
        update_available,
        release_url: "https://github.com/omarjson/LocalAide/releases/latest".to_string(),
    })
}

/// True when `latest` is strictly newer than `current` (dotted numerics).
fn version_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(current), parse(latest));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if y > x {
            return true;
        }
        if y < x {
            return false;
        }
    }
    false
}

/// Tail of the llama-server log for the Settings diagnostics panel.
#[tauri::command]
pub fn logs_tail(lines: Option<usize>) -> Result<Vec<String>, AppError> {
    let path = crate::app::paths::app_dir()
        .join("logs")
        .join("llama-server.log");
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
    let collected: Vec<String> = text
        .lines()
        .rev()
        .take(keep)
        .map(|s| s.to_string())
        .collect();
    collected.reverse();
    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::version_newer;

    #[test]
    fn detects_newer_versions() {
        assert!(version_newer("0.1.0", "0.1.1"));
        assert!(version_newer("0.1.0", "1.0.0"));
        assert!(version_newer("0.1.9", "0.2.0"));
        assert!(!version_newer("0.1.0", "0.1.0"));
        assert!(!version_newer("0.2.0", "0.1.9"));
        assert!(!version_newer("0.1.0", "0.1.0-beta"));
    }
}
