use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::system::SystemProbeResult;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, System};
use tauri::State;

const OS_RESERVE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB

#[tauri::command]
pub async fn system_probe(state: State<'_, AppState>) -> Result<SystemProbeResult, AppError> {
    // The share models may use comes from Settings → Memory for models.
    let percent = state
        .config_store
        .lock()
        .ok()
        .and_then(|store| store.load().ok())
        .map(|cfg| cfg.memory_percent_for_models())
        .unwrap_or(crate::config::DEFAULT_MODEL_MEMORY_PERCENT);
    crate::app::blocking::run_blocking(move || probe_system(percent)).await
}

/// Reads RAM figures. sysinfo can take a moment, so this runs off the UI thread.
fn probe_system(memory_percent: u8) -> Result<SystemProbeResult, AppError> {
    // Only RAM and the CPU brand are needed here. `new_all()` +
    // `refresh_all()` would enumerate every process, CPU, disk and network
    // interface twice; targeted refreshes read just the figures we use.
    let mut sys = System::new();
    sys.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
    sys.refresh_cpu_specifics(CpuRefreshKind::new().with_frequency());
    let total_ram_bytes = sys.total_memory();
    let available_ram_bytes = sys.available_memory();
    let total_ram_gb = total_ram_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    let available_ram_gb = available_ram_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    // The brand string is per-CPU and identical across them; an empty one
    // is the same as no answer, so it becomes None downstream.
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty());
    let cpu_cores = sys
        .physical_core_count()
        .map(|cores| cores as u32)
        .filter(|cores| *cores > 0);

    let usable_bytes = total_ram_bytes.saturating_sub(OS_RESERVE_BYTES);
    let budget_bytes = (usable_bytes as f64 * f64::from(memory_percent) / 100.0) as u64;
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
        cpu_brand,
        cpu_cores,
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
