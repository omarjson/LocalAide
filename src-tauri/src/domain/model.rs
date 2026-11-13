use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub filename: String,
    /// One of: "chat", "code", "embedding", "reranker".
    pub kind: String,
    pub size_bytes: u64,
    pub ram_estimate_gb: f64,
    pub quant: String,
    pub license: String,
    pub sha256: String,
    pub download_url: String,
    pub installed: bool,
    pub verified: bool,
    pub active: bool,
    pub download_progress: Option<f32>,
}

/// Where the active model catalog came from: "built-in" (compiled into the
/// app) or "refreshed" (fetched from the project repository, ADR-0008).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogStatus {
    pub version: u32,
    pub updated_at: u64,
    pub model_count: usize,
    pub source: String,
    pub fetched_at: Option<u64>,
    /// Where a refresh fetches from; shown next to the refresh control.
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub models: Vec<String>,
    pub ram_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVerifyResult {
    pub id: String,
    pub verified: bool,
    pub expected_sha256: String,
    pub actual_sha256: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSizeResult {
    pub id: String,
    pub size_bytes: u64,
    pub complete: bool,
}
