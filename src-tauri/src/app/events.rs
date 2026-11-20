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
    /// What LocalAide is doing before the answer starts, for example
    /// "Searching the web". Shown as a short activity line.
    Status {
        label: String,
    },
    Done {
        finish_reason: String,
        total_tokens: usize,
    },
    Error {
        error: String,
    },
    /// A piece of the model's thinking, shown in the collapsible
    /// "Thinking" block above the answer.
    Reasoning {
        text: String,
    },
    /// The model asked for a tool. `group` is "web", "notes", "skills",
    /// "files" or "mcp", for the icon.
    ToolCall {
        id: String,
        name: String,
        title: String,
        detail: String,
        group: String,
        /// The arguments as the model wrote them, pretty-printed.
        arguments: String,
    },
    /// The tool changes something, so the chat asks the user. The answer
    /// goes back through `chat_tool_decide` with `approval_id`.
    ToolApproval {
        approval_id: String,
        id: String,
        title: String,
        detail: String,
        /// What will be written or sent, shortened.
        preview: String,
    },
    ToolResult {
        id: String,
        ok: bool,
        summary: String,
        output: String,
    },
    /// The model asked the user a question with options (`ask_user`). The
    /// answer ends here; the user's choice is the next message.
    Question {
        id: String,
        question: String,
        options: Vec<String>,
        multiple: bool,
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

/// Engine startup progress for the chat screen. `state` is "loading", "ready" or "failed".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatusEvent {
    pub state: String,
    pub model_id: Option<String>,
    pub message: Option<String>,
}
