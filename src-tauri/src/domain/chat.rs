use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub ts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<crate::domain::memory::Citation>>,
    /// The answer in order: thinking, tool calls and text. Saved so a chat
    /// opened later still shows what the model did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<MessagePart>>,
    /// Files attached to a user message, with the text the model read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<ChatAttachment>>,
    /// Which model wrote an answer, for example "Qwen3 4B" or
    /// "gpt-oss-120b · OpenRouter".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The answer failed. Failed answers are not sent back as history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<bool>,
}

/// One piece of an answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        text: String,
    },
    Reasoning {
        text: String,
        /// How long the model thought, in milliseconds.
        #[serde(default)]
        ms: Option<u64>,
    },
    Tool {
        id: String,
        name: String,
        title: String,
        #[serde(default)]
        detail: String,
        #[serde(default)]
        group: String,
        #[serde(default)]
        arguments: String,
        /// "running", "waiting", "done", "failed" or "denied".
        status: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        output: String,
    },
}

/// A file attached to a message. Images travel as data URLs and are not
/// saved with the chat; text files keep their text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatAttachment {
    pub name: String,
    /// "text" or "image".
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub data_url: Option<String>,
    /// Characters of text read from the file.
    #[serde(default)]
    pub chars: usize,
    /// The text was shortened to fit.
    #[serde(default)]
    pub truncated: bool,
    /// Why the file could not be read, when it could not.
    #[serde(default)]
    pub error: Option<String>,
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

/// The model the picker chose. No provider = the local model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatTargetRequest {
    #[serde(default)]
    pub provider_id: Option<String>,
    pub model: String,
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
    /// Let the model search the user's notes and documents. Default true.
    #[serde(default)]
    pub use_recall: Option<bool>,
    /// Let the model search the web. Default false; the user turns it on,
    /// so no web request happens without their choice.
    #[serde(default)]
    pub use_web: Option<bool>,
    /// Let the model read and (with approval) write workspace files.
    /// Default true.
    #[serde(default)]
    pub use_files: Option<bool>,
    /// Let the model read saved skills. Default true.
    #[serde(default)]
    pub use_skills: Option<bool>,
    /// MCP servers switched on for this chat. None = every enabled server.
    #[serde(default)]
    pub mcp_servers: Option<Vec<String>>,
    /// Tools the user allowed for the rest of this chat.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// The model chosen in the picker. None = the current default.
    #[serde(default)]
    pub target: Option<ChatTargetRequest>,
    /// How hard the model thinks: "auto", "off", "on", "low", "medium",
    /// "high". None = auto.
    #[serde(default)]
    pub effort: Option<String>,
    /// Text files attached to `message`. Their text is added for the model.
    #[serde(default)]
    pub attachments: Vec<ChatAttachment>,
    /// The user's local date and time, for example "Thursday, 19 November
    /// 2026, 14:05 (Europe/Berlin)". Models do not know today's date.
    #[serde(default)]
    pub now: Option<String>,
}

/// The user's answer to a tool approval question.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolDecisionRequest {
    pub approval_id: String,
    /// "allow", "allow_chat" or "deny".
    pub decision: String,
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
