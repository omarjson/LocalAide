use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemProbeResult {
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    /// No GPU source exists in this stack (sysinfo reports none), so this
    /// stays None and the UI says "Not detected" instead of inventing one.
    pub gpu_vendor: Option<String>,
    /// CPU brand as the OS reports it, e.g. "Apple M1" / "Intel Core i7".
    /// None when the OS gives nothing usable.
    pub cpu_brand: Option<String>,
    /// Physical cores, when the OS reports them.
    pub cpu_cores: Option<u32>,
    pub recommended_preset: String,
    pub budget_gb: f64,
}
