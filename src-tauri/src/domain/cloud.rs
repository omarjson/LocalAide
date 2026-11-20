//! DTOs for the opt-in cloud provider settings.

use serde::{Deserialize, Serialize};

/// One configured provider as shown in Settings. The API key is never sent
/// back to the frontend; only a short hint of its last four characters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProviderDto {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub model: String,
    pub api_key_hint: String,
    pub is_active: bool,
    /// Shown in the chat's model picker.
    pub enabled: bool,
    /// Models saved from the last "Load models".
    pub model_count: usize,
}

/// One model in the chat's model picker.
#[derive(Debug, Clone, Serialize)]
pub struct ModelChoice {
    /// "local:<model id>" or "<provider id>:<model>".
    pub key: String,
    /// None for models on this computer.
    pub provider_id: Option<String>,
    pub provider_label: String,
    /// The id sent to the engine.
    pub model: String,
    pub name: String,
    pub local: bool,
    pub reasoning: bool,
    pub vision: bool,
    pub tools: bool,
    pub context: Option<u64>,
    /// Thinking levels to offer, lowest first; "off" when it can be switched off.
    pub efforts: Vec<String>,
    pub free: bool,
    pub favorite: bool,
    /// The picker's current choice.
    pub active: bool,
    /// For local models: the file size, to show how big it is.
    pub size_bytes: Option<u64>,
}

/// An online provider's group in the picker.
#[derive(Debug, Clone, Serialize)]
pub struct ModelGroup {
    pub provider_id: String,
    pub label: String,
    pub enabled: bool,
    pub local_router: bool,
    /// True when the model list was never loaded.
    pub needs_refresh: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelChoices {
    pub models: Vec<ModelChoice>,
    pub groups: Vec<ModelGroup>,
    pub active_key: Option<String>,
    /// The local engine is running a model.
    pub local_loaded: bool,
    /// Online providers are switched on in Settings. Off: none are listed.
    pub online_allowed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderEnableRequest {
    pub provider_id: String,
    pub enabled: bool,
}

/// Save request for one provider. An existing `id` updates that provider;
/// a missing id creates a new one.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudProviderSaveRequest {
    pub id: Option<String>,
    pub label: String,
    pub base_url: String,
    pub model: String,
    /// The full key. Optional when updating so an edit can keep the stored key.
    pub api_key: Option<String>,
}

/// Full cloud settings state: whether cloud mode is on, the providers, and
/// which provider is active.
#[derive(Debug, Clone, Serialize)]
pub struct CloudConfigStatus {
    /// Master switch: online providers may be used.
    pub online_allowed: bool,
    /// Chat is set to an online model right now.
    pub cloud_enabled: bool,
    pub providers: Vec<CloudProviderDto>,
    pub active_provider_id: Option<String>,
}

/// Switch cloud mode on or off. `provider_id` optionally moves the active
/// provider at the same time.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudModeRequest {
    pub enabled: bool,
    pub provider_id: Option<String>,
}

/// Ask a provider for its model list. `api_key` is a key typed into the form;
/// without it, the saved key of `provider_id` is used for the same site.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudModelsRequest {
    pub base_url: String,
    pub api_key: Option<String>,
    pub provider_id: Option<String>,
}
