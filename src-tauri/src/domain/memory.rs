use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNote {
    pub id: String,
    pub path: String,
    pub title: String,
    pub content: Option<String>,
    pub tags: Vec<String>,
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCreateResponse {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Citation {
    pub index: usize,
    pub source: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetAllResponse {
    pub deleted_count: usize,
}
