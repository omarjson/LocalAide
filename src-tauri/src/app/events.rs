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
        /// The server's own prompt count for the last round, when reported.
        prompt_tokens: Option<usize>,
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
        /// Every question of this call, with option descriptions. The
        /// fields above repeat the first one, for older readers.
        #[serde(default)]
        items: Vec<QuestionItem>,
    },
    /// The model's to-do list for this task (`update_todos`), shown as
    /// "Completed 2/4" in the work status above the chat box.
    Todos {
        items: Vec<TodoItem>,
    },
    /// What a subagent (`run_subagent`, tool call `id`) is doing now.
    SubagentStep {
        id: String,
        step: String,
    },
    /// The model saved or improved a skill on its own (`save_skill`). The
    /// chat shows a note with Undo, which calls `skills_undo(change_id)`.
    SkillChange {
        id: String,
        change_id: String,
        name: String,
        /// "created" or "updated".
        action: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionOption {
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuestionItem {
    /// A short heading such as "Clarify", may be empty.
    #[serde(default)]
    pub header: String,
    pub question: String,
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multiple: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoItem {
    pub text: String,
    /// "pending", "in_progress" or "done".
    pub status: String,
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
