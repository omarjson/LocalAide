//! Commands for the opt-in cloud provider settings.
//!
//! Providers live in the app config file on this machine. The API key never
//! leaves the backend except as a bearer token to the provider's base URL.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::{CloudProvider, validate_base_url};
use crate::domain::cloud::{
    CloudConfigStatus, CloudModeRequest, CloudProviderDto, CloudProviderSaveRequest,
};
use crate::services::inference::remote::RemoteTarget;
use serde::Serialize;
use tauri::State;

/// A curated starting point for the provider form. The list is aggregated
/// from the open 9Router registry (github.com/decolua/9router, MIT) plus
/// first-party endpoints; free tiers change without notice, so the user's
/// own key and the provider's docs decide what actually works.
#[derive(Debug, Clone, Serialize)]
pub struct CloudProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    /// A sensible first model to prefill; every provider serves more.
    pub model: &'static str,
    /// Where the user signs up for a key. Empty for local proxies.
    pub key_url: &'static str,
    pub note: &'static str,
}

pub const PROVIDER_PRESETS: &[CloudProviderPreset] = &[
    CloudProviderPreset {
        id: "ollama-local",
        label: "Ollama (this machine)",
        base_url: "http://127.0.0.1:11434/v1",
        model: "llama3.3",
        key_url: "",
        note: "Your own local Ollama server; no key needed.",
    },
    CloudProviderPreset {
        id: "llm7",
        label: "LLM7 (free)",
        base_url: "https://api.llm7.io/v1",
        model: "gpt-5.5",
        key_url: "https://llm7.io",
        note: "Free community router; grab a free token, no card.",
    },
    CloudProviderPreset {
        id: "synthetic",
        label: "Synthetic (free tier)",
        base_url: "https://api.synthetic.new/openai/v1",
        model: "hf:zai-org/GLM-5.3",
        key_url: "https://synthetic.new",
        note: "Free tier backed by Hugging Face inference.",
    },
    CloudProviderPreset {
        id: "api-airforce",
        label: "api.airforce (free tier)",
        base_url: "https://api.airforce/v1",
        model: "gpt-oss-120b",
        key_url: "https://api.airforce",
        note: "Free tier; reliability varies.",
    },
    CloudProviderPreset {
        id: "groq",
        label: "Groq",
        base_url: "https://api.groq.com/openai/v1",
        model: "llama-3.3-70b-versatile",
        key_url: "https://console.groq.com/keys",
        note: "Generous free tier with strict rate limits.",
    },
    CloudProviderPreset {
        id: "cerebras",
        label: "Cerebras",
        base_url: "https://api.cerebras.ai/v1",
        model: "llama-3.3-70b",
        key_url: "https://cloud.cerebras.ai",
        note: "Free tier with a daily token cap.",
    },
    CloudProviderPreset {
        id: "nvidia",
        label: "NVIDIA NIM",
        base_url: "https://integrate.api.nvidia.com/v1",
        model: "meta/llama-3.3-70b-instruct",
        key_url: "https://build.nvidia.com",
        note: "Free tier, about 40 requests per minute.",
    },
    CloudProviderPreset {
        id: "openrouter",
        label: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        model: "meta-llama/llama-3.3-70b-instruct:free",
        key_url: "https://openrouter.ai/keys",
        note: "Many :free models at 50 requests/day.",
    },
    CloudProviderPreset {
        id: "chutes",
        label: "Chutes",
        base_url: "https://llm.chutes.ai/v1",
        model: "deepseek-ai/DeepSeek-R1",
        key_url: "https://chutes.ai",
        note: "Free tier with per-chute quotas.",
    },
    CloudProviderPreset {
        id: "mistral",
        label: "Mistral",
        base_url: "https://api.mistral.ai/v1",
        model: "mistral-small-latest",
        key_url: "https://console.mistral.ai",
        note: "Free Experiment tier; data-training opt-in.",
    },
];

#[tauri::command]
pub fn cloud_provider_presets() -> Vec<CloudProviderPreset> {
    PROVIDER_PRESETS.to_vec()
}

/// Plain-HTTP proxies on this machine (Ollama, LM Studio, llama-server)
/// legitimately run without any key.
fn is_loopback_http(base_url: &str) -> bool {
    url::Url::parse(base_url).is_ok_and(|u| {
        u.scheme() == "http"
            && u.host_str().is_some_and(|h| h == "localhost" || h.starts_with("127."))
    })
}

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
        is_active: active_id == Some(p.id.as_str()),
    }
}

fn load_config(state: &State<'_, AppState>) -> Result<crate::config::AppConfig, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.load().map_err(AppError::internal)
}

fn save_config(
    state: &State<'_, AppState>,
    cfg: &crate::config::AppConfig,
) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.save(cfg).map_err(AppError::internal)
}

/// Switch the runtime engine to match the stored config.
fn apply_engine_mode(state: &State<'_, AppState>, cfg: &crate::config::AppConfig) {
    if cfg.cloud_llm_enabled
        && let Some(id) = cfg.active_cloud_provider.as_deref()
        && let Some(p) = cfg.cloud_providers.iter().find(|p| p.id == id)
    {
        state.inference_engine.set_cloud_target(RemoteTarget {
            label: p.label.clone(),
            base_url: p.base_url.clone(),
            model: p.model.clone(),
            api_key: p.api_key.clone(),
        });
        return;
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

    // Two entries for the same endpoint and model are always a mistake; the
    // entry being edited is exempt from its own match.
    let duplicate = cfg.cloud_providers.iter().any(|p| {
        p.base_url == base_url && p.model == model && req.id.as_deref() != Some(p.id.as_str())
    });
    if duplicate {
        return Err(AppError::invalid_input(
            "A provider with this base URL and model already exists.",
        ));
    }

    let id = match req.id.as_deref() {
        Some(existing) => {
            let provider = cfg.cloud_providers.iter_mut().find(|p| p.id == existing);
            let Some(provider) = provider else {
                return Err(AppError::invalid_input("That provider no longer exists."));
            };
            provider.label = label;
            provider.base_url = base_url;
            provider.model = model;
            if let Some(key) = req
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty())
            {
                provider.api_key = key.to_string();
            }
            provider.id.clone()
        }
        None => {
            let key = req
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty());
            if key.is_none() && !is_loopback_http(&base_url) {
                return Err(AppError::invalid_input(
                    "An API key is required for a new provider. (Local proxies on this machine are the exception.)",
                ));
            }
            let id = format!("cp-{}", uuid_v4());
            cfg.cloud_providers.push(CloudProvider {
                id: id.clone(),
                label,
                base_url,
                model,
                api_key: key.unwrap_or_default().to_string(),
            });
            id
        }
    };

    // A first provider becomes the active one, but cloud mode stays off until
    // the user switches it on explicitly.
    if cfg.active_cloud_provider.is_none() {
        cfg.active_cloud_provider = Some(id);
    }

    // Persist first: a failed disk write must not leave the engine switched to
    // a state the config file disagrees with.
    save_config(&state, &cfg)?;
    apply_engine_mode(&state, &cfg);

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
        // Chat pointed at the deleted provider. Turn cloud mode off instead of
        // silently re-routing to whichever provider is first; the remaining
        // provider stays marked active so re-enabling is one click.
        cfg.cloud_llm_enabled = false;
    }

    save_config(&state, &cfg)?;
    apply_engine_mode(&state, &cfg);

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

    if req.enabled && cfg.active_cloud_provider.is_none() {
        return Err(AppError::invalid_input(
            "Add and choose an online provider before switching cloud mode on.",
        ));
    }
    cfg.cloud_llm_enabled = req.enabled;

    save_config(&state, &cfg)?;
    apply_engine_mode(&state, &cfg);

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
