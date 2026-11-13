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
