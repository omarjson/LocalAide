use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::system::SystemProbeResult;
use sysinfo::System;
use tauri::State;

const OS_RESERVE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB
const BUDGET_SHARE: f64 = 0.7; // 70% of usable RAM

#[tauri::command]
pub fn system_probe() -> Result<SystemProbeResult, AppError> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let total_ram_bytes = sys.total_memory();
    let available_ram_bytes = sys.available_memory();
    let total_ram_gb = total_ram_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    let available_ram_gb = available_ram_bytes as f64 / 1024.0 / 1024.0 / 1024.0;

    let usable_bytes = total_ram_bytes.saturating_sub(OS_RESERVE_BYTES);
    let budget_bytes = (usable_bytes as f64 * BUDGET_SHARE) as u64;
    let budget_gb = budget_bytes as f64 / 1024.0 / 1024.0 / 1024.0;

    let recommended_preset = if total_ram_gb >= 28.0 {
        "advanced"
    } else if total_ram_gb >= 14.0 {
        "balanced"
    } else if total_ram_gb >= 7.0 {
        "small"
    } else {
        "tiny"
    }
    .to_string();

    Ok(SystemProbeResult {
        total_ram_gb,
        available_ram_gb,
        gpu_vendor: None,
        recommended_preset,
        budget_gb,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingStatusResponse {
    /// "llama-server" when the embedding sidecar is up, "not-running" otherwise.
    pub engine: String,
    pub model_id: Option<String>,
    pub dimension: Option<usize>,
    pub ready: bool,
    pub total_chunks: usize,
    pub embedded_chunks: usize,
}

#[tauri::command]
pub fn embedding_status(state: State<'_, AppState>) -> Result<EmbeddingStatusResponse, AppError> {
    let engine_state = state.embedding_engine.state();
    let (total_chunks, embedded_chunks) = state.library.embedding_coverage()?;
    Ok(EmbeddingStatusResponse {
        engine: if engine_state.ready {
            "llama-server".to_string()
        } else {
            "not-running".to_string()
        },
        model_id: engine_state.model_id,
        dimension: engine_state.dimension,
        ready: engine_state.ready,
        total_chunks,
        embedded_chunks,
    })
}
