use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserPresetDto {
    pub id: String,
    pub name: String,
    pub model_ids: Vec<String>,
    pub total_ram_gb: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfigDto {
    pub selected_models: Vec<String>,
    pub presets: Vec<UserPresetDto>,
    pub ram_budget_mb: u64,
    pub cloud_llm_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RamComputationResponse {
    pub total_ram_gb: f64,
}
