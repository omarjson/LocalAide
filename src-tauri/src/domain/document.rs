use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexFolderResponse {
    pub indexed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIndexRequest {
    pub query: String,
    pub mode: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultDoc {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub modified: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerStatsResponse {
    pub doc_count: u64,
    pub total_bytes: u64,
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchFolderResponse {
    pub folder: String,
    pub status: String,
}
