use crate::app::error::AppError;
use crate::app::events::DownloadProgressEvent;
use crate::app::state::AppState;
use crate::domain::model::{
    CatalogStatus, ModelInfo, ModelPreset, ModelSizeResult, ModelVerifyResult,
};
use crate::services::models::refresh::CATALOG_URL;
use crate::services::models::verify::compute_sha256_streaming;
use tauri::State;

#[tauri::command]
pub fn model_list(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, AppError> {
    let catalog = state.model_service.list_catalog();
    let active_id = state.model_manager.active_model().map(|m| m.id);

    let infos = catalog
        .models
        .iter()
        .map(|e| {
            let path = state.model_service.model_file_path(&e.id);
            let installed = path.is_file() || state.model_manager.is_installed(&e.id);
            let is_active = active_id.as_deref() == Some(&e.id);
            let filename = e.dest.clone().unwrap_or_else(|| format!("{}.gguf", e.id));
            ModelInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                filename,
                kind: e.kind.clone(),
                size_bytes: e.file_size,
                ram_estimate_gb: e.ram_estimate_gb as f64,
                quant: e.quant.clone(),
                license: e.license.clone(),
                sha256: e.sha256.clone(),
                download_url: e.url.clone(),
                // Split shards are companions too; only an mmproj projector
                // makes a model accept images.
                vision: e.companions.iter().any(|c| c.dest.contains("mmproj")),
                installed,
                verified: installed,
                active: is_active,
                download_progress: None,
            }
        })
        .collect();
    Ok(infos)
}

#[tauri::command]
pub fn model_catalog_status(state: State<'_, AppState>) -> CatalogStatus {
    let manifest = state.model_service.manifest();
    let refreshed = state.model_service.fetched_at().is_some();
    CatalogStatus {
        version: manifest.version,
        updated_at: manifest.updated_at,
        model_count: manifest.models.len(),
        source: if refreshed { "refreshed" } else { "built-in" }.to_string(),
        fetched_at: state.model_service.fetched_at(),
        url: CATALOG_URL.to_string(),
    }
}

/// Fetch the manifest from the project repository over HTTPS, validate it,
/// cache it, and swap the active catalog. Fails closed: on any validation
/// problem the previously active catalog stays in place.
#[tauri::command]
pub async fn model_catalog_refresh(state: State<'_, AppState>) -> Result<CatalogStatus, AppError> {
    state.model_service.refresh_catalog().await?;
    Ok(model_catalog_status(state))
}

/// Delete the cached catalog and restore the list compiled into the app.
#[tauri::command]
pub fn model_catalog_reset(state: State<'_, AppState>) -> Result<CatalogStatus, AppError> {
    state.model_service.reset_catalog()?;
    Ok(model_catalog_status(state))
}

#[tauri::command]
pub fn model_presets() -> Vec<ModelPreset> {
    use crate::model_manager::Preset;
    let list = vec![
        Preset::tiny(),
        Preset::small(),
        Preset::balanced(),
        Preset::advanced(),
    ];
    list.into_iter()
        .map(|p| ModelPreset {
            id: p.id,
            name: p.name,
            description: format!("Fits {}", p.fits_ram_gb),
            models: p.models,
            ram_gb: p.total_ram_gb as f64,
        })
        .collect()
}

#[tauri::command]
pub fn model_activate(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state
        .model_manager
        .activate(&id)
        .map_err(|e| AppError::model_load_failed(&id, &e))
}

#[tauri::command]
pub fn model_remove(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let path = state.model_service.model_file_path(&id);
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    let part = path.with_extension("part");
    if part.exists() {
        let _ = std::fs::remove_file(&part);
    }
    // Companion files (vision projectors) go with the model.
    if let Some(entry) = state.model_service.get_model_entry(&id) {
        for c in &entry.companions {
            let dest = state.model_service.models_dir().join(&c.dest);
            if dest.exists() {
                let _ = std::fs::remove_file(&dest);
            }
            let dest_part = dest.with_extension("part");
            if dest_part.exists() {
                let _ = std::fs::remove_file(&dest_part);
            }
        }
    }

    state.model_manager.remove(&id).map_err(AppError::internal)
}

#[tauri::command]
pub fn model_verify(state: State<'_, AppState>, id: String) -> Result<ModelVerifyResult, AppError> {
    let entry = state
        .model_service
        .get_model_entry(&id)
        .ok_or_else(|| AppError::model_not_installed(&id))?;

    let expected = entry.sha256.clone();
    let path = state.model_service.model_file_path(&id);
    if !path.exists() {
        return Ok(ModelVerifyResult {
            id,
            verified: false,
            expected_sha256: expected,
            actual_sha256: None,
            error: Some("Model file not found on disk".to_string()),
        });
    }

    match compute_sha256_streaming(&path) {
        Ok(actual) => {
            let verified = actual.eq_ignore_ascii_case(&expected);
            Ok(ModelVerifyResult {
                id,
                verified,
                expected_sha256: expected,
                actual_sha256: Some(actual),
                error: None,
            })
        }
        Err(err) => Ok(ModelVerifyResult {
            id,
            verified: false,
            expected_sha256: expected,
            actual_sha256: None,
            error: Some(err.to_string()),
        }),
    }
}

#[tauri::command]
pub fn model_existing_size(
    state: State<'_, AppState>,
    id: String,
) -> Result<ModelSizeResult, AppError> {
    let path = state.model_service.model_file_path(&id);
    let part_path = path.with_extension("part");
    let size_bytes = if path.exists() {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else if part_path.exists() {
        std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
    } else {
        state.model_manager.existing_size(&id)
    };

    let complete = path.is_file();
    Ok(ModelSizeResult {
        id,
        size_bytes,
        complete,
    })
}

#[tauri::command]
pub fn model_download_cancel(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.model_manager.cancel_download(&id);
    let _ = state.jobs.cancel_job(&format!("download_{id}"));
    Ok(())
}

/// Cancel every in-flight download of a preset. Each per-model job's cancel
/// flag makes `download_model` return a cancelled error, which stops the
/// preset loop on the first remaining model.
#[tauri::command]
pub fn model_preset_cancel(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let presets = model_presets();
    let preset = presets
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::invalid_input(format!("Unknown preset: {id}")))?;
    for model_id in preset.models {
        state.jobs.cancel_job(&format!("preset_{model_id}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn model_download(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    channel: tauri::ipc::Channel<DownloadProgressEvent>,
) -> Result<(), AppError> {
    let (job_id, cancel_flag) = state.jobs.create_job(format!("download_{id}"));
    let channel_clone = channel.clone();
    let model_id_clone = id.clone();

    let res = state
        .model_service
        .download_model(&id, cancel_flag.clone(), move |progress| {
            let _ = channel_clone.send(DownloadProgressEvent {
                model_id: model_id_clone.clone(),
                downloaded_bytes: progress.bytes_downloaded,
                total_bytes: progress.bytes_total,
                percent: progress.percent,
                speed_bps: progress.speed_bps,
                status: "downloading".to_string(),
            });
        })
        .await;

    match res {
        Ok(_path) => {
            state.jobs.complete_job(&job_id);
            let _ = state.model_manager.activate(&id);
            let _ = channel.send(DownloadProgressEvent {
                model_id: id,
                downloaded_bytes: 0,
                total_bytes: 0,
                percent: 100.0,
                speed_bps: 0.0,
                status: "complete".to_string(),
            });
            Ok(())
        }
        Err(e) => {
            let err_msg = e.to_string();
            state.jobs.fail_job(&job_id, &err_msg);
            let _ = channel.send(DownloadProgressEvent {
                model_id: id,
                downloaded_bytes: 0,
                total_bytes: 0,
                percent: 0.0,
                speed_bps: 0.0,
                status: "failed".to_string(),
            });
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn model_download_preset(
    state: State<'_, AppState>,
    id: String,
    channel: tauri::ipc::Channel<DownloadProgressEvent>,
) -> Result<(), AppError> {
    let presets = model_presets();
    let preset = presets
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::invalid_input(format!("Unknown preset: {id}")))?;

    // Each model weighs equally in the preset's overall progress bar, and
    // failures stop the preset instead of being swallowed.
    let model_count = preset.models.len() as f32;
    let mut installed: Vec<String> = Vec::new();
    for (index, model_id) in preset.models.into_iter().enumerate() {
        let (job_id, cancel_flag) = state.jobs.create_job(format!("preset_{model_id}"));
        let channel_clone = channel.clone();
        let event_model_id = model_id.clone();
        let res = state
            .model_service
            .download_model(&model_id, cancel_flag, move |progress| {
                let model_share = progress.percent / 100.0;
                let overall = ((index as f32 + model_share) / model_count * 100.0).min(100.0);
                let _ = channel_clone.send(DownloadProgressEvent {
                    model_id: event_model_id.clone(),
                    downloaded_bytes: progress.bytes_downloaded,
                    total_bytes: progress.bytes_total,
                    percent: overall,
                    speed_bps: progress.speed_bps,
                    status: "downloading".to_string(),
                });
            })
            .await;

        match res {
            Ok(_path) => {
                state.jobs.complete_job(&job_id);
                let _ = state.model_manager.activate(&model_id);
                installed.push(model_id);
            }
            Err(e) => {
                let err_msg = e.to_string();
                state.jobs.fail_job(&job_id, &err_msg);
                let _ = channel.send(DownloadProgressEvent {
                    model_id,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    percent: 0.0,
                    speed_bps: 0.0,
                    status: "failed".to_string(),
                });
                return Err(e);
            }
        }
    }

    // Record what was installed so returning users skip the setup wizard
    // (App.svelte gates the wizard on selected_models).
    if !installed.is_empty()
        && let Ok(store) = state.config_store.lock()
    {
        match store.load() {
            Ok(mut cfg) => {
                for id in &installed {
                    if !cfg.selected_models.contains(id) {
                        cfg.selected_models.push(id.clone());
                    }
                }
                if let Err(e) = store.save(&cfg) {
                    log::warn!("Could not record selected models: {e}");
                }
            }
            Err(e) => log::warn!("Could not load config to record selected models: {e}"),
        }
    }
    Ok(())
}
