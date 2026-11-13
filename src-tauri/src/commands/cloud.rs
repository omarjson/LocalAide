//! Commands for the opt-in cloud provider settings.
//!
//! Providers live in the app config file on this machine. The API key never
//! leaves the backend except as a bearer token to the provider's base URL.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::{CloudProvider, validate_base_url};
use crate::domain::cloud::{CloudConfigStatus, CloudModeRequest, CloudProviderDto, CloudProviderSaveRequest};
use crate::services::inference::remote::RemoteTarget;
use tauri::State;

/// Short hint of a stored key for the frontend: fixed dots plus the last four
/// characters for longer keys.
fn key_hint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "••••••••".to_string();
    }
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("••••••••{tail}")
}

fn provider_dto(p: &CloudProvider, active_id: Option<&str>) -> CloudProviderDto {
    CloudProviderDto {
        id: p.id.clone(),
        label: p.label.clone(),
        base_url: p.base_url.clone(),
        model: p.model.clone(),
        api_key_hint: key_hint(&p.api_key),
    }
    .with_active(active_id == Some(p.id.as_str()))
}

impl CloudProviderDto {
    fn with_active(mut self, active: bool) -> Self {
        self.is_active = active;
        self
    }
}

fn load_config(state: &State<'_, AppState>) -> Result<crate::config::AppConfig, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.load().map_err(AppError::internal)
}

fn save_config(state: &State<'_, AppState>, cfg: &crate::config::AppConfig) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.save(cfg).map_err(AppError::internal)
}

/// Switch the runtime engine to match the stored config.
fn apply_engine_mode(state: &State<'_, AppState>, cfg: &crate::config::AppConfig) {
    if cfg.cloud_llm_enabled {
        if let Some(id) = cfg.active_cloud_provider.as_deref() {
            if let Some(p) = cfg.cloud_providers.iter().find(|p| p.id == id) {
                state.inference_engine.set_cloud_target(RemoteTarget {
                    label: p.label.clone(),
                    base_url: p.base_url.clone(),
                    model: p.model.clone(),
                    api_key: p.api_key.clone(),
                });
                return;
            }
        }
    }
    state.inference_engine.set_local();
}

#[tauri::command]
pub fn cloud_config_get(state: State<'_, AppState>) -> Result<CloudConfigStatus, AppError> {
    let cfg = load_config(&state)?;
    let active = cfg.active_cloud_provider.clone();
    Ok(CloudConfigStatus {
        cloud_enabled: cfg.cloud_llm_enabled,
        providers: cfg
            .cloud_providers
            .iter()
            .map(|p| provider_dto(p, active.as_deref()))
            .collect(),
        active_provider_id: active,
    })
}

#[tauri::command]
pub fn cloud_provider_save(
    state: State<'_, AppState>,
    req: CloudProviderSaveRequest,
) -> Result<CloudConfigStatus, AppError> {
    let label = req.label.trim().to_string();
    let base_url = req.base_url.trim().to_string();
    let model = req.model.trim().to_string();
    if label.is_empty() {
        return Err(AppError::invalid_input("Give the provider a name."));
    }
    if base_url.is_empty() {
        return Err(AppError::invalid_input("A base URL is required."));
    }
    validate_base_url(&base_url).map_err(AppError::invalid_input)?;
    if model.is_empty() {
        return Err(AppError::invalid_input(
            "Enter the model name the provider should use, for example gpt-4o-mini.",
        ));
    }

    let mut cfg = load_config(&state)?;

    let id = match req.id.as_deref() {
        Some(existing) => {
            let provider = cfg.cloud_providers.iter_mut().find(|p| p.id == existing);
            let Some(provider) = provider else {
                return Err(AppError::invalid_input("That provider no longer exists."));
            };
            provider.label = label;
            provider.base_url = base_url;
            provider.model = model;
            if let Some(key) = req.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
                provider.api_key = key.to_string();
            }
            provider.id.clone()
        }
        None => {
            let Some(key) = req.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) else {
                return Err(AppError::invalid_input("An API key is required for a new provider."));
            };
            let id = format!("cp-{}", uuid_v4());
            cfg.cloud_providers.push(CloudProvider {
                id: id.clone(),
                label,
                base_url,
                model,
                api_key: key.to_string(),
            });
            id
        }
    };

    // A first provider becomes the active one, but cloud mode stays off until
    // the user switches it on explicitly.
    if cfg.active_cloud_provider.is_none() {
        cfg.active_cloud_provider = Some(id);
    }

    apply_engine_mode(&state, &cfg);
    save_config(&state, &cfg)?;

    let active = cfg.active_cloud_provider.clone();
    Ok(CloudConfigStatus {
        cloud_enabled: cfg.cloud_llm_enabled,
        providers: cfg
            .cloud_providers
            .iter()
            .map(|p| provider_dto(p, active.as_deref()))
            .collect(),
        active_provider_id: active,
    })
}

#[tauri::command]
pub fn cloud_provider_delete(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<CloudConfigStatus, AppError> {
    let mut cfg = load_config(&state)?;
    let before = cfg.cloud_providers.len();
    cfg.cloud_providers.retain(|p| p.id != provider_id);
    if cfg.cloud_providers.len() == before {
        return Err(AppError::invalid_input("That provider no longer exists."));
    }
    if cfg.active_cloud_provider.as_deref() == Some(provider_id.as_str()) {
        cfg.active_cloud_provider = cfg.cloud_providers.first().map(|p| p.id.clone());
    }
    // Deleting the active provider while cloud mode is on would leave chat
    // pointing at nothing, so cloud mode turns off.
    if cfg.active_cloud_provider.is_none() {
        cfg.cloud_llm_enabled = false;
    }

    apply_engine_mode(&state, &cfg);
    save_config(&state, &cfg)?;

    let active = cfg.active_cloud_provider.clone();
    Ok(CloudConfigStatus {
        cloud_enabled: cfg.cloud_llm_enabled,
        providers: cfg
            .cloud_providers
            .iter()
            .map(|p| provider_dto(p, active.as_deref()))
            .collect(),
        active_provider_id: active,
    })
}

#[tauri::command]
pub fn cloud_mode_set(
    state: State<'_, AppState>,
    req: CloudModeRequest,
) -> Result<CloudConfigStatus, AppError> {
    let mut cfg = load_config(&state)?;

    if let Some(id) = req.provider_id {
        if !cfg.cloud_providers.iter().any(|p| p.id == id) {
            return Err(AppError::invalid_input("That provider no longer exists."));
        }
        cfg.active_cloud_provider = Some(id);
    }

    cfg.cloud_llm_enabled = req.enabled;
    if req.enabled && cfg.active_cloud_provider.is_none() {
        return Err(AppError::invalid_input(
            "Add and choose an online provider before switching cloud mode on.",
        ));
    }

    apply_engine_mode(&state, &cfg);
    save_config(&state, &cfg)?;

    let active = cfg.active_cloud_provider.clone();
    Ok(CloudConfigStatus {
        cloud_enabled: cfg.cloud_llm_enabled,
        providers: cfg
            .cloud_providers
            .iter()
            .map(|p| provider_dto(p, active.as_deref()))
            .collect(),
        active_provider_id: active,
    })
}

/// A random id for a provider entry, same scheme as `app::jobs`.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let rand_part = (nanos ^ 0x5deece66d) as u64;
    format!("{:016x}{:016x}", nanos as u64, rand_part)
}
