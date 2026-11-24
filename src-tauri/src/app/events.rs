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
        /// Cached prompt tokens the server reported, when it does.
        cache_tokens: Option<usize>,
        /// The server's prompt count for the first round (system prompt,
        /// tools, history and the new message), when reported.
        #[serde(default)]
        first_prompt_tokens: Option<usize>,
        /// Estimated tokens of the system prompt and pinned notes.
        #[serde(default)]
        system_tokens: usize,
        /// Estimated tokens of the tool schemas sent with the request.
        #[serde(default)]
        tool_tokens: usize,
    },
    Error {
        error: String,
    },
    /// A piece of the model's thinking, shown in the collapsible
    /// "Thinking" block above the answer.
    Reasoning {
        text: String,
    },
    /// A note the user sent while the answer ran reached the model.
    Steered {
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
    /// `tokens` is the subagent's total token use (prompt + answer) once
    /// its run finishes; steps before that carry none.
    SubagentStep {
        id: String,
        step: String,
        #[serde(default)]
        tokens: Option<usize>,
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
    /// The model's pick when the user is unsure: shown first, marked
    /// "Recommended". Old calls omit it, so it defaults to false.
    #[serde(default)]
    pub recommended: bool,
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
