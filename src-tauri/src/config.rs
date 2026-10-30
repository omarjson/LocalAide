use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub selected_models: Vec<String>,
    pub presets: Vec<UserPreset>,
    pub ram_budget_mb: u64,
    pub cloud_llm_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreset {
    pub id: String,
    pub name: String,
    pub model_ids: Vec<String>,
    pub total_ram_gb: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            selected_models: vec![],
            presets: vec![],
            ram_budget_mb: 4096,
            cloud_llm_enabled: false,
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Self {
        Self {
            path: config_path(),
        }
    }

    pub fn load(&self) -> Result<AppConfig, String> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }
        let data = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    pub fn save(&self, cfg: &AppConfig) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
        fs::write(&self.path, data).map_err(|e| e.to_string())
    }

    pub fn compute_total_ram_gb(
        &self,
        model_ids: &[String],
        catalog: &[crate::model_manager::ModelEntry],
    ) -> f32 {
        model_ids
            .iter()
            .filter_map(|id| {
                catalog
                    .iter()
                    .find(|m| &m.id == id)
                    .map(|m| m.ram_estimate_gb)
            })
            .sum()
    }
}
