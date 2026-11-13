use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    Token {
        token: String,
    },
    Citation {
        index: usize,
        source: String,
        title: String,
        snippet: String,
        score: f32,
    },
    Done {
        finish_reason: String,
        total_tokens: usize,
    },
    Error {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub speed_bps: f64,
    pub status: String,
}
