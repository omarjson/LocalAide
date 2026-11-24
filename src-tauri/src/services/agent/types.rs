//! The shared types of the chat loop: what a run is, what it returns,
//! and the state it shares. The loop itself lives in `super` (`mod.rs`).
//!
//! The helpers on `AgentState` and `ApprovalMode::note` are `pub(crate)`:
//! the loop in `super` uses them, and they are not part of the public API.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::app::events::ChatStreamEvent;
use crate::services::inference::openai_stream::{ChatTurn, Effort};
use crate::services::inference::router::ChatTarget;
use crate::services::tools::sandbox::WorkspaceJail;

use super::approval::ApprovalHub;
use super::tools::Permissions;

/// Where the chat loop sends its events. The Tauri channel in the app, a
/// list in tests.
pub trait Sink: Send + Sync {
    fn emit(&self, event: ChatStreamEvent);
}

impl Sink for tauri::ipc::Channel<ChatStreamEvent> {
    fn emit(&self, event: ChatStreamEvent) {
        let _ = self.send(event);
    }
}

/// Shared state of the chat loop.
#[derive(Default)]
pub struct AgentState {
    pub approvals: ApprovalHub,
    /// Endpoints and models that refused tools, so the next message skips
    /// straight to the tool-less path.
    no_tools: Mutex<HashSet<String>>,
    /// Notes the user sent to a running answer, by job id, waiting for the
    /// model's next step.
    steers: Mutex<HashMap<String, Vec<String>>>,
}

impl AgentState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hands a note to the running answer `job_id`. The model reads it
    /// before its next step.
    pub fn steer(&self, job_id: &str, text: String) {
        self.steers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(job_id.to_string())
            .or_default()
            .push(text);
    }

    /// Takes the notes waiting for `job_id`, oldest first.
    pub(crate) fn take_steers(&self, job_id: &str) -> Vec<String> {
        self.steers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(job_id)
            .unwrap_or_default()
    }

    /// True when a note is waiting for `job_id`.
    pub(crate) fn has_steers(&self, job_id: &str) -> bool {
        self.steers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .is_some_and(|notes| !notes.is_empty())
    }

    pub(crate) fn knows_no_tools(&self, key: &str) -> bool {
        self.no_tools
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(key)
    }

    pub(crate) fn mark_no_tools(&self, key: &str) {
        self.no_tools
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_string());
    }
}

/// How much the chat asks before a tool runs (the permission menu in the
/// chat box). Blocked commands never run, whatever the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalMode {
    /// Plan mode: tools only read; changes are written into a plan
    /// instead. Subagents always run like this.
    Plan,
    /// Ask before changes: every tool that changes something asks.
    #[default]
    Ask,
    /// Allowed for this chat (from 0.1.6): no questions until the chat ends.
    Chat,
    /// Autonomous: changes inside the folder run; only risky ones ask
    /// (see `ToolSpec::risky`).
    Smart,
    /// Full access: no questions.
    Auto,
}

impl ApprovalMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("plan") => Self::Plan,
            Some("chat") => Self::Chat,
            Some("smart") | Some("autonomous") => Self::Smart,
            Some("auto") | Some("full") => Self::Auto,
            _ => Self::Ask,
        }
    }

    pub(crate) fn note(self) -> Option<&'static str> {
        match self {
            Self::Plan | Self::Ask => None,
            Self::Chat => Some("allowed for this chat"),
            Self::Smart => Some("allowed by Autonomous mode"),
            Self::Auto => Some("full access"),
        }
    }
}

/// One answer to run.
pub struct AgentRun {
    pub job_id: String,
    pub target: ChatTarget,
    /// System prompt, kept history and the new user message, in order.
    pub turns: Vec<ChatTurn>,
    /// The longest answer the user allows, not counting thinking.
    pub answer_tokens: i32,
    pub temperature: f32,
    pub effort: Effort,
    /// The model thinks before it answers.
    pub thinks: bool,
    pub perms: Permissions,
    /// False for models known not to call tools.
    pub use_tools: bool,
    pub max_rounds: usize,
    pub context_tokens: usize,
    /// Tools the user allowed for this whole chat.
    pub auto_approve: HashSet<String>,
    pub approval_mode: ApprovalMode,
    /// "chat", or "share" for a request through a shared link.
    pub source: &'static str,
    /// The number of the next source, after any the chat already listed.
    pub first_citation: usize,
    /// The user's message, for the tool-less fallback.
    pub user_message: String,
    /// Autonomous mode: the tool-less fallback also reads the top page.
    pub read_top_page: bool,
    /// The folder the file tools and commands work in: the workspace, or
    /// the project folder the user picked for this chat.
    pub jail: Arc<WorkspaceJail>,
    /// The project folder the chat works in, when one was picked —
    /// remembered so usage can be counted per project.
    pub folder: Option<String>,
    /// The chat thread this run answers in, so usage groups into sessions.
    pub session: Option<String>,
}

pub struct RunResult {
    pub tokens: usize,
    pub cancelled: bool,
    pub finish_reason: String,
    pub tool_calls: usize,
    /// Time to the first token of the first round, when known.
    pub ttft_ms: Option<u64>,
    /// Prompt size the server reported for the last round, when known.
    pub prompt_tokens: Option<usize>,
    /// Cached prompt tokens the server reported for the last round.
    pub cache_tokens: Option<usize>,
    /// The server's prompt count for the first round: the system prompt,
    /// tools, history and the new message, before any tool round. This is
    /// what the next message starts from, so the chat's meter uses it.
    pub first_prompt_tokens: Option<usize>,
    /// Estimated tokens of the system turns (prompt, pinned notes).
    pub system_tokens: usize,
    /// Estimated tokens of the tool schemas sent with each request.
    pub tool_tokens: usize,
}
