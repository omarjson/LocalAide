//! Commands for the opt-in cloud provider settings.
//!
//! Providers live in the app config file on this machine. The API key never
//! leaves the backend except as a bearer token to the provider's base URL.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::{CloudProvider, is_local_host, validate_base_url};
use crate::domain::cloud::{
    CloudConfigStatus, CloudModeRequest, CloudModelsRequest, CloudProviderDto,
    CloudProviderSaveRequest, ProviderEnableRequest,
};
use crate::services::inference::provider_models::{ProviderModel, list_models};
use crate::services::inference::remote::RemoteTarget;
use serde::Serialize;
use tauri::State;

/// A curated starting point for the provider form. The list started from
/// the open 9Router registry (github.com/decolua/9router, MIT) plus
/// first-party endpoints. Free tiers change without notice, so "Load models"
/// in Settings asks the provider what it serves today.
#[derive(Debug, Clone, Serialize)]
pub struct CloudProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    /// A first model to prefill. Empty when the user should load the list.
    pub model: &'static str,
    /// Where the user signs up for a key. Empty when no key is needed.
    pub key_url: &'static str,
    pub note: &'static str,
    /// False when the provider works without any key.
    pub needs_key: bool,
    /// True when this is a router or server on the user's own computer.
    pub local: bool,
    /// True when the provider has a free way in (keyless or a free key).
    pub free: bool,
    /// A shared, public key some keyless services expect (OpenCode Zen
    /// uses "public"). Prefilled into the form; empty for most providers.
    pub public_key: &'static str,
}

// One positional constructor keeps the table above readable as rows.
#[allow(clippy::too_many_arguments)]
const fn preset(
    id: &'static str,
    label: &'static str,
    base_url: &'static str,
    model: &'static str,
    key_url: &'static str,
    note: &'static str,
    needs_key: bool,
    local: bool,
    free: bool,
) -> CloudProviderPreset {
    CloudProviderPreset {
        id,
        label,
        base_url,
        model,
        key_url,
        note,
        needs_key,
        local,
        free,
        public_key: "",
    }
}

pub const PROVIDER_PRESETS: &[CloudProviderPreset] = &[
    // No key at all.
    preset(
        "kilo",
        "Kilo Code (free, no key)",
        "https://api.kilo.ai/api/openrouter",
        "kilo-auto/free",
        "https://kilo.ai",
        "Kilo's gateway serves free models without a key or sign-up. \"kilo-auto/free\" picks the best free model for you; press Load models to choose one yourself. Free models change over time.",
        false,
        false,
        true,
    ),
    CloudProviderPreset {
        public_key: "public",
        ..preset(
            "opencode-zen",
            "OpenCode Zen (free models)",
            "https://opencode.ai/zen/v1",
            "",
            "https://opencode.ai/zen",
            "OpenCode's model gateway. Its free models (names ending in -free, and big-pickle) work with the shared key \"public\", which is filled in for you. Press Load models and pick one. Paste your own Zen key to use the paid models.",
            false,
            false,
            true,
        )
    },
    preset(
        "pollinations",
        "Pollinations (free, no key)",
        "https://text.pollinations.ai/v1",
        "openai",
        "https://enter.pollinations.ai",
        "Works without a key or sign-up. Free use is limited to one request about every 45 seconds per computer, and answers may be filtered. Add a Pollinations key for more models and no wait.",
        false,
        false,
        true,
    ),
    // Routers on this computer. 9Router and OmniRoute both listen on port
    // 20128 by default and pool many free tiers behind one address.
    preset(
        "9router",
        "9Router (on this computer)",
        "http://localhost:20128/v1",
        "",
        "https://github.com/decolua/9router",
        "A free router you run yourself. It switches between free providers when one runs out. Start it, then press Load models. Paste a key only if you turned keys on in its dashboard.",
        false,
        true,
        true,
    ),
    preset(
        "omniroute",
        "OmniRoute (on this computer)",
        "http://localhost:20128/v1",
        "",
        "https://github.com/diegosouzapw/OmniRoute",
        "A free router you run yourself, with automatic fallback across many providers. Start it, then press Load models. Paste a key only if you set one in OmniRoute.",
        false,
        true,
        true,
    ),
    preset(
        "ollama-local",
        "Ollama (on this computer)",
        "http://127.0.0.1:11434/v1",
        "",
        "",
        "Your own Ollama server. No key needed. Press Load models to see what you have pulled.",
        false,
        true,
        true,
    ),
    preset(
        "lmstudio-local",
        "LM Studio (on this computer)",
        "http://127.0.0.1:1234/v1",
        "",
        "",
        "LM Studio's local server. Turn it on in LM Studio, then press Load models.",
        false,
        true,
        true,
    ),
    // Free keys.
    preset(
        "gemini",
        "Google Gemini",
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "gemini-flash-latest",
        "https://aistudio.google.com/apikey",
        "Free key from Google AI Studio, with daily limits. On the free tier Google may use your prompts to improve its products.",
        true,
        false,
        true,
    ),
    preset(
        "openrouter",
        "OpenRouter",
        "https://openrouter.ai/api/v1",
        "",
        "https://openrouter.ai/keys",
        "One key for hundreds of models. Models marked Free cost nothing, with a daily request limit.",
        true,
        false,
        true,
    ),
    preset(
        "groq",
        "Groq",
        "https://api.groq.com/openai/v1",
        "llama-3.3-70b-versatile",
        "https://console.groq.com/keys",
        "Very fast. Free key with strict rate limits.",
        true,
        false,
        true,
    ),
    preset(
        "cerebras",
        "Cerebras",
        "https://api.cerebras.ai/v1",
        "",
        "https://cloud.cerebras.ai",
        "Free key with a daily token cap.",
        true,
        false,
        true,
    ),
    preset(
        "huggingface",
        "Hugging Face Inference",
        "https://router.huggingface.co/v1",
        "",
        "https://huggingface.co/settings/tokens",
        "Open models from many hosts with one Hugging Face token. Free accounts get small monthly credits.",
        true,
        false,
        true,
    ),
    preset(
        "nvidia",
        "NVIDIA NIM",
        "https://integrate.api.nvidia.com/v1",
        "",
        "https://build.nvidia.com",
        "Free key, about 40 requests per minute.",
        true,
        false,
        true,
    ),
    preset(
        "mistral",
        "Mistral",
        "https://api.mistral.ai/v1",
        "mistral-small-latest",
        "https://console.mistral.ai",
        "Free Experiment tier. Mistral may train on free-tier prompts.",
        true,
        false,
        true,
    ),
    preset(
        "llm7",
        "LLM7",
        "https://api.llm7.io/v1",
        "",
        "https://llm7.io",
        "Community router. Get a free token, no card needed.",
        true,
        false,
        true,
    ),
    preset(
        "chutes",
        "Chutes",
        "https://llm.chutes.ai/v1",
        "",
        "https://chutes.ai",
        "Open models with per-model quotas.",
        true,
        false,
        true,
    ),
    // Paid.
    preset(
        "openai",
        "OpenAI",
        "https://api.openai.com/v1",
        "",
        "https://platform.openai.com/api-keys",
        "Paid by use.",
        true,
        false,
        false,
    ),
    preset(
        "deepseek",
        "DeepSeek",
        "https://api.deepseek.com/v1",
        "deepseek-chat",
        "https://platform.deepseek.com/api_keys",
        "Paid by use, low prices.",
        true,
        false,
        false,
    ),
    preset(
        "together",
        "Together AI",
        "https://api.together.xyz/v1",
        "",
        "https://api.together.ai/settings/api-keys",
        "Paid by use, many open models.",
        true,
        false,
        false,
    ),
];

#[tauri::command]
pub fn cloud_provider_presets() -> Vec<CloudProviderPreset> {
    PROVIDER_PRESETS.to_vec()
}

/// Ask a provider which models it serves. Runs only when the user presses
/// "Load models". A key typed into the form wins; otherwise the saved key of
/// `provider_id` is used, and only when the address is still the same site.
#[tauri::command]
pub async fn cloud_list_models(
    state: State<'_, AppState>,
    req: CloudModelsRequest,
) -> Result<Vec<ProviderModel>, AppError> {
    let base_url = req.base_url.trim().to_string();
    if base_url.is_empty() {
        return Err(AppError::invalid_input("Enter the provider address first."));
    }
    validate_base_url(&base_url).map_err(AppError::invalid_input)?;
    let typed = req
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string);
    let key = match typed {
        Some(key) => key,
        None => match req.provider_id.as_deref() {
            Some(id) => {
                let cfg = load_config(&state)?;
                cfg.cloud_providers
                    .iter()
                    .find(|p| p.id == id && same_origin(&p.base_url, &base_url))
                    .map(|p| p.api_key.clone())
                    .unwrap_or_default()
            }
            None => String::new(),
        },
    };
    let models = list_models(&base_url, &key).await?;
    // An edit of a saved provider also refreshes the picker's list.
    if let Some(id) = req.provider_id.as_deref() {
        store_models(&state, id, &base_url, &models)?;
    }
    Ok(models)
}

/// Plain-HTTP proxies on this machine (Ollama, LM Studio, llama-server)
/// legitimately run without any key.
fn is_loopback_http(base_url: &str) -> bool {
    url::Url::parse(base_url).is_ok_and(|u| u.scheme() == "http" && is_local_host(&u))
}

/// Short hint of a stored key for the frontend: fixed dots plus the last four
/// characters for longer keys.
fn key_hint(key: &str) -> String {
    if key.trim().is_empty() {
        return String::new();
    }
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
        enabled: p.enabled,
        model_count: p.models.len(),
    }
}

fn status_of(cfg: &crate::config::AppConfig) -> CloudConfigStatus {
    let active = cfg.active_cloud_provider.clone();
    CloudConfigStatus {
        cloud_enabled: cfg.cloud_llm_enabled,
        providers: cfg
            .cloud_providers
            .iter()
            .map(|p| provider_dto(p, active.as_deref()))
            .collect(),
        active_provider_id: active,
    }
}

/// The engine target for a provider and a model, or its default model.
pub(crate) fn remote_target(p: &CloudProvider, model: Option<&str>) -> RemoteTarget {
    RemoteTarget {
        label: p.label.clone(),
        base_url: p.base_url.clone(),
        model: model
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .unwrap_or(&p.model)
            .to_string(),
        api_key: p.api_key.clone(),
    }
}

/// Show or hide a provider's models in the chat's model picker. Several
/// providers can be on at once. Hiding the provider chat uses moves chat
/// back to this computer.
#[tauri::command]
pub fn cloud_provider_set_enabled(
    state: State<'_, AppState>,
    req: ProviderEnableRequest,
) -> Result<CloudConfigStatus, AppError> {
    let mut cfg = load_config(&state)?;
    let Some(provider) = cfg
        .cloud_providers
        .iter_mut()
        .find(|p| p.id == req.provider_id)
    else {
        return Err(AppError::invalid_input("That provider no longer exists."));
    };
    provider.enabled = req.enabled;
    if !req.enabled && cfg.active_cloud_provider.as_deref() == Some(req.provider_id.as_str()) {
        cfg.cloud_llm_enabled = false;
    }
    save_config(&state, &cfg)?;
    apply_engine_mode(&state, &cfg);
    Ok(status_of(&cfg))
}

/// Ask a saved provider for its models again and keep the list for the
/// picker. Runs only when the user presses Refresh.
#[tauri::command]
pub async fn cloud_models_refresh(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<ProviderModel>, AppError> {
    let cfg = load_config(&state)?;
    let provider = cfg
        .cloud_providers
        .iter()
        .find(|p| p.id == provider_id)
        .cloned()
        .ok_or_else(|| AppError::invalid_input("That provider no longer exists."))?;
    let models = list_models(&provider.base_url, &provider.api_key).await?;
    store_models(&state, &provider_id, &provider.base_url, &models)?;
    Ok(models)
}

/// Keep a provider's model list, when the address is still the one the
/// list came from.
fn store_models(
    state: &AppState,
    provider_id: &str,
    base_url: &str,
    models: &[ProviderModel],
) -> Result<(), AppError> {
    let mut cfg = load_config(state)?;
    if let Some(p) = cfg
        .cloud_providers
        .iter_mut()
        .find(|p| p.id == provider_id && same_origin(&p.base_url, base_url))
    {
        p.models = models.to_vec();
        save_config(state, &cfg)?;
    }
    Ok(())
}

pub(crate) fn load_config(state: &AppState) -> Result<crate::config::AppConfig, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.load().map_err(AppError::internal)
}

pub(crate) fn save_config(
    state: &AppState,
    cfg: &crate::config::AppConfig,
) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.save(cfg).map_err(AppError::internal)
}

/// Switch the runtime engine to match the stored config.
pub(crate) fn apply_engine_mode(state: &AppState, cfg: &crate::config::AppConfig) {
    if cfg.cloud_llm_enabled
        && let Some(id) = cfg.active_cloud_provider.as_deref()
        && let Some(p) = cfg.cloud_providers.iter().find(|p| p.id == id && p.enabled)
    {
        state
            .inference_engine
            .set_cloud_target(remote_target(p, cfg.active_cloud_model.as_deref()));
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
            let new_key = req
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty());
            // A saved key goes only to the address it was entered for. When
            // the address moves to another site, the key must be entered again.
            if new_key.is_none()
                && !provider.api_key.is_empty()
                && !same_origin(&provider.base_url, &base_url)
            {
                if is_loopback_http(&base_url) {
                    provider.api_key.clear();
                } else {
                    return Err(AppError::invalid_input(
                        "The address changed, so enter the API key again. LocalAide sends a key only to the address it was entered for.",
                    ));
                }
            }
            provider.label = label;
            // A list from another address does not describe this one.
            if !same_origin(&provider.base_url, &base_url) {
                provider.models.clear();
            }
            provider.base_url = base_url;
            provider.model = model;
            if let Some(key) = new_key {
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
            // The key is optional: keyless services (Pollinations) and routers
            // on this computer work without one. A provider that needs a key
            // answers chat with a clear "needs an API key" message.
            let id = format!("cp-{}", uuid_v4());
            cfg.cloud_providers.push(CloudProvider {
                id: id.clone(),
                label,
                base_url,
                model,
                api_key: key.unwrap_or_default().to_string(),
                enabled: true,
                models: Vec::new(),
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

/// A random id for a provider entry, from the operating system's random source.
fn uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // No random source: fall back to the clock, which is unique enough
        // for a list the user edits by hand.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        bytes = nanos.to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// True when both addresses point at the same site: scheme, host and port.
fn same_origin(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{same_origin, uuid_v4};

    #[test]
    fn a_key_stays_with_its_site() {
        assert!(same_origin(
            "https://api.example.com/v1",
            "https://api.example.com/v2"
        ));
        assert!(same_origin(
            "https://api.example.com/v1",
            "https://api.example.com:443/v1"
        ));
        assert!(!same_origin(
            "https://api.example.com/v1",
            "https://evil.example.net/v1"
        ));
        assert!(!same_origin(
            "https://api.example.com/v1",
            "http://api.example.com/v1"
        ));
        assert!(!same_origin(
            "https://api.example.com/v1",
            "https://api.example.com:8443/v1"
        ));
        assert!(!same_origin("not a url", "not a url"));
    }

    #[test]
    fn provider_ids_are_random() {
        let a = uuid_v4();
        let b = uuid_v4();
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
