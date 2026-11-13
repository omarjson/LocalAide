use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::{AppConfig, UserPreset};
use crate::domain::settings::{AppConfigDto, RamComputationResponse, UserPresetDto};
use tauri::State;

#[tauri::command]
pub fn config_load(state: State<'_, AppState>) -> Result<AppConfigDto, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;

    let presets = cfg
        .presets
        .into_iter()
        .map(|p| UserPresetDto {
            id: p.id,
            name: p.name,
            model_ids: p.model_ids,
            total_ram_gb: p.total_ram_gb,
        })
        .collect();

    Ok(AppConfigDto {
        selected_models: cfg.selected_models,
        presets,
        ram_budget_mb: cfg.ram_budget_mb,
        cloud_llm_enabled: cfg.cloud_llm_enabled,
    })
}

#[tauri::command]
pub fn config_save(state: State<'_, AppState>, cfg: AppConfigDto) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let presets = cfg
        .presets
        .into_iter()
        .map(|p| UserPreset {
            id: p.id,
            name: p.name,
            model_ids: p.model_ids,
            total_ram_gb: p.total_ram_gb,
        })
        .collect();

    // Cloud provider settings are managed by their own commands. Load the
    // current config so a preset save here cannot wipe them.
    let mut internal_cfg = store.load().unwrap_or_default();
    internal_cfg.selected_models = cfg.selected_models;
    internal_cfg.presets = presets;
    internal_cfg.ram_budget_mb = cfg.ram_budget_mb;
    internal_cfg.cloud_llm_enabled = cfg.cloud_llm_enabled;

    store.save(&internal_cfg).map_err(AppError::internal)

    store.save(&internal_cfg).map_err(AppError::internal)
}

#[tauri::command]
pub fn config_compute_ram(
    state: State<'_, AppState>,
    model_ids: Vec<String>,
) -> Result<RamComputationResponse, AppError> {
    let catalog = state.model_manager.list_models();
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let total = store.compute_total_ram_gb(&model_ids, &catalog);
    Ok(RamComputationResponse {
        total_ram_gb: total as f64,
    })
}
