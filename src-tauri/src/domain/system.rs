use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemProbeResult {
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    pub gpu_vendor: Option<String>,
    pub recommended_preset: String,
    pub budget_gb: f64,
}
