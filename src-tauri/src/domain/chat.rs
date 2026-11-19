use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub ts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<crate::domain::memory::Citation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatThread {
    pub id: String,
    pub title: String,
    pub system_prompt: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamRequest {
    pub message: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    /// Earlier turns of this conversation, oldest first. Excludes `message`.
    #[serde(default)]
    pub history: Vec<ChatMessage>,
    /// Images attached to `message`, as `data:image/...;base64,` URLs.
    #[serde(default)]
    pub images: Vec<String>,
    /// "standard" (default), "strict", "unrestricted" or "autonomous".
    #[serde(default)]
    pub mode: Option<String>,
    /// Look in the user's notes and documents. Default true.
    #[serde(default)]
    pub use_recall: Option<bool>,
    /// Search the web for this message. Default false; the user turns it
    /// on for each chat, so no web request happens without their choice.
    #[serde(default)]
    pub use_web: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeRequest {
    pub text: String,
    pub max_words: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeResponse {
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadExportRequest {
    pub thread_id: String,
    pub format: String, // "markdown" | "json"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadExportResponse {
    pub content: String,
    pub filename: String,
}
