//! The chat loop with tools.
//!
//! The model gets the user's message and the tools this chat may use. It
//! decides for itself whether to call one: a question it can answer from
//! what it knows gets a direct answer, and a question about today's news
//! gets a web search. Each round streams the model's thinking and text to
//! the window; tool calls run (after the user's approval where needed), and
//! their results go back to the model for the next round. A model that
//! cannot use tools gets the old behaviour: LocalAide gathers web results
//! and notes first and puts them in the prompt.

pub mod approval;
pub mod oneshot;
pub mod tools;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::app::error::AppError;
use crate::app::events::{ChatStreamEvent, QuestionItem, QuestionOption, TodoItem};
use crate::app::state::AppState;
use crate::domain::memory::Citation;
use crate::services::inference::openai_stream::{
    Adjustment, ChatTurn, Delta, Effort, TOOLS_UNSUPPORTED, ToolCall, initial_options,
    stream_with_retries,
};
use crate::services::inference::router::ChatTarget;
use crate::services::tools::sandbox::WorkspaceJail;
use approval::{APPROVAL_TIMEOUT, ApprovalHub, Decision};
use tools::{Permissions, ToolSpec};

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
}

impl AgentState {
    pub fn new() -> Self {
        Self::default()
    }

    fn knows_no_tools(&self, key: &str) -> bool {
        self.no_tools
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(key)
    }

    fn mark_no_tools(&self, key: &str) {
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

    fn note(self) -> Option<&'static str> {
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
}

pub struct RunResult {
    pub tokens: usize,
    pub cancelled: bool,
    pub finish_reason: String,
    pub tool_calls: usize,
}

fn estimate_tokens(turns: &[ChatTurn]) -> usize {
    let chars: usize = turns
        .iter()
        .map(|t| match t {
            ChatTurn::System(s) => s.len(),
            ChatTurn::User { text, images } => text.len() + images.len() * 3000,
            ChatTurn::Assistant { text, tool_calls } => {
                text.len()
                    + tool_calls
                        .iter()
                        .map(|c| c.arguments.len() + c.name.len() + 16)
                        .sum::<usize>()
            }
            ChatTurn::Tool { content, .. } => content.len() + 16,
        })
        .sum();
    chars.div_ceil(4) + turns.len() * 4
}

/// Keep the conversation inside the context: older tool results are
/// replaced by a short note first, then the oldest history turns go.
/// The system prompt and the newest user message always stay.
pub fn fit_turns(turns: &mut Vec<ChatTurn>, budget: usize) {
    const REMOVED: &str = "[Earlier tool result removed to save space.]";
    let tool_slots: Vec<usize> = turns
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, ChatTurn::Tool { .. }))
        .map(|(i, _)| i)
        .collect();
    // The newest tool result is the one the model needs most; keep it.
    for &i in &tool_slots[..tool_slots.len().saturating_sub(1)] {
        if estimate_tokens(turns) <= budget {
            return;
        }
        if let ChatTurn::Tool { content, .. } = &mut turns[i] {
            *content = REMOVED.to_string();
        }
    }
    let last_user = turns
        .iter()
        .rposition(|t| matches!(t, ChatTurn::User { .. }))
        .unwrap_or(0);
    let first = usize::from(matches!(turns.first(), Some(ChatTurn::System(_))));
    let mut removable = last_user.saturating_sub(first);
    while estimate_tokens(turns) > budget && removable > 0 {
        turns.remove(first);
        removable -= 1;
    }
}

/// The answer limit for one round: the user's answer length plus room to
/// think, capped by what is left of the context.
fn round_limit(run: &AgentRun, turns: &[ChatTurn]) -> i32 {
    let thinking = if run.thinks {
        run.effort.thinking_allowance()
    } else {
        0
    };
    let wanted = run.answer_tokens.max(64) + thinking;
    let left = run
        .context_tokens
        .saturating_sub(estimate_tokens(turns) + 64)
        .min(i32::MAX as usize) as i32;
    wanted.min(left).max(256)
}

fn emit_citations(sink: &dyn Sink, citations: Vec<Citation>) {
    for cit in citations {
        sink.emit(ChatStreamEvent::Citation {
            index: cit.index,
            source: cit.source,
            title: cit.title,
            snippet: cit.snippet,
            score: cit.score,
        });
    }
}

fn status(sink: &dyn Sink, label: impl Into<String>) {
    sink.emit(ChatStreamEvent::Status {
        label: label.into(),
    });
}

/// Run the chat loop until the model answers without asking for a tool.
pub fn run(
    state: &AppState,
    mut run: AgentRun,
    cancel: &AtomicBool,
    sink: &dyn Sink,
) -> Result<RunResult, AppError> {
    let engine = state.inference_engine.clone();
    let endpoint = engine.endpoint_for(&run.target)?;
    let key = format!("{}|{}", endpoint.url, endpoint.model);

    let mut specs: Vec<ToolSpec> = Vec::new();
    let tools_wanted = run.perms.any();
    let skip_tools = !run.use_tools || state.agent.knows_no_tools(&key);
    if tools_wanted && !skip_tools {
        specs = tools::builtin_specs(&run.perms);
        if !run.perms.mcp.is_empty() {
            let (mcp, problems) = tools::mcp_specs(state, &run.perms.mcp);
            for problem in problems {
                status(sink, problem);
            }
            specs.extend(mcp);
        }
    }
    let mut next_citation = run.first_citation.max(1);
    if tools_wanted && skip_tools {
        add_gathered_context(state, &mut run, &mut next_citation, sink);
    }

    let schemas: Vec<Value> = specs.iter().map(ToolSpec::schema).collect();
    let mut opts = initial_options(
        endpoint.flavor,
        run.answer_tokens,
        run.temperature,
        schemas,
        run.effort.clone(),
    );
    let max_chars = run.context_tokens.clamp(2000, 16_000);
    let mut total_tokens = 0usize;
    let mut finish_reason = "stop".to_string();
    let mut tool_calls_run = 0usize;
    let mut seen_ids: HashSet<String> = HashSet::new();
    let client = engine.chat_client();

    let mut round = 0usize;
    while round < run.max_rounds {
        if cancel.load(Ordering::Relaxed) {
            return Ok(RunResult {
                tokens: total_tokens,
                cancelled: true,
                finish_reason: "cancelled".into(),
                tool_calls: tool_calls_run,
            });
        }
        let last_round = round + 1 == run.max_rounds;
        if last_round && !opts.tools.is_empty() {
            opts.forbid_tools = true;
        }
        let reserve = run.answer_tokens.max(256) as usize;
        fit_turns(&mut run.turns, run.context_tokens.saturating_sub(reserve));
        opts.max_tokens = round_limit(&run, &run.turns);

        let result = stream_with_retries(
            client,
            &endpoint,
            &mut run.turns,
            &mut opts,
            cancel,
            &mut |delta| match delta {
                Delta::Content(token) => sink.emit(ChatStreamEvent::Token { token }),
                Delta::Reasoning(text) => sink.emit(ChatStreamEvent::Reasoning { text }),
            },
        );
        let (outcome, adjustments) = match result {
            Ok(v) => v,
            Err(e) if e.code == TOOLS_UNSUPPORTED => {
                state.agent.mark_no_tools(&key);
                specs.clear();
                opts.tools.clear();
                status(
                    sink,
                    "This model cannot use tools, so LocalAide looked things up first",
                );
                add_gathered_context(state, &mut run, &mut next_citation, sink);
                // Same round again, without tools.
                continue;
            }
            Err(e) => return Err(e),
        };
        for adjustment in adjustments {
            match adjustment {
                Adjustment::DroppedImages => {
                    status(sink, "This model cannot read images, so they were left out")
                }
                Adjustment::DroppedEffort => {
                    status(sink, "This model does not take a thinking setting")
                }
                _ => {}
            }
        }
        total_tokens += outcome.completion_tokens.unwrap_or(outcome.pieces);
        if outcome.cancelled {
            return Ok(RunResult {
                tokens: total_tokens,
                cancelled: true,
                finish_reason: "cancelled".into(),
                tool_calls: tool_calls_run,
            });
        }
        let wants_tools = !outcome.tool_calls.is_empty() && !specs.is_empty() && !opts.forbid_tools;
        if !wants_tools {
            finish_reason = outcome
                .finish_reason
                .clone()
                .unwrap_or_else(|| "stop".into());
            if finish_reason == "length"
                && outcome.content.trim().is_empty()
                && !outcome.reasoning.trim().is_empty()
            {
                status(
                    sink,
                    "The model used its whole answer length on thinking. Lower the thinking effort, or raise \"Longest answer\" in chat settings.",
                );
            }
            break;
        }

        // Call ids must be unique across the whole answer; small local
        // models reuse "call_0" every round.
        let calls: Vec<ToolCall> = outcome
            .tool_calls
            .into_iter()
            .map(|mut c| {
                if !seen_ids.insert(c.id.clone()) {
                    c.id = format!("{}_{round}", c.id);
                    seen_ids.insert(c.id.clone());
                }
                c
            })
            .collect();
        run.turns.push(ChatTurn::Assistant {
            text: outcome.content,
            tool_calls: calls.clone(),
        });
        let mut asked = false;
        let offered = |name: &str| specs.iter().any(|s| s.name == name);
        // Subagents asked for in this round run side by side, first.
        let sub_calls: Vec<&ToolCall> = if offered("run_subagent") {
            calls
                .iter()
                .filter(|c| c.name == "run_subagent")
                .take(MAX_SUBAGENTS)
                .collect()
        } else {
            Vec::new()
        };
        let sub_results = if sub_calls.is_empty() {
            HashMap::new()
        } else {
            run_subagents(state, &run, &sub_calls, &mut next_citation, cancel, sink)
        };
        for call in &calls {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            tool_calls_run += 1;
            if let Some(content) = sub_results.get(&call.id) {
                run.turns.push(ChatTurn::Tool {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    content: content.clone(),
                });
                continue;
            }
            let handled = match call.name.as_str() {
                "run_subagent" if offered("run_subagent") => Some(format!(
                    "Error: at most {MAX_SUBAGENTS} subagents run at once. Ask again for this one when the others are done."
                )),
                "update_todos" if offered("update_todos") => Some(update_todos(call, sink)),
                "save_skill" if offered("save_skill") => {
                    Some(save_skill(state, call, run.source, sink))
                }
                _ => None,
            };
            if let Some(content) = handled {
                run.turns.push(ChatTurn::Tool {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    content,
                });
                continue;
            }
            if call.name == "ask_user" && offered("ask_user") {
                if ask_question(call, sink) {
                    asked = true;
                    break;
                }
                run.turns.push(ChatTurn::Tool {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    content:
                        "Error: ask_user needs questions, each with a question and 2 to 6 options."
                            .into(),
                });
                continue;
            }
            let content = run_call(
                state,
                &run.job_id,
                call,
                &specs,
                &mut run.auto_approve,
                (run.approval_mode, run.source, &run.jail),
                max_chars,
                &mut next_citation,
                cancel,
                sink,
            );
            run.turns.push(ChatTurn::Tool {
                call_id: call.id.clone(),
                name: call.name.clone(),
                content,
            });
        }
        if asked {
            // The user's choice is the next message.
            finish_reason = "question".into();
            break;
        }
        round += 1;
    }

    Ok(RunResult {
        tokens: total_tokens,
        cancelled: cancel.load(Ordering::Relaxed),
        finish_reason,
        tool_calls: tool_calls_run,
    })
}

/// The questions of an `ask_user` call: the `questions` list, or the
/// older single `question` with plain `options`.
fn parse_questions(args: &Value) -> Vec<QuestionItem> {
    let clip = |v: Option<&Value>, max: usize| -> String {
        v.and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .chars()
            .take(max)
            .collect()
    };
    let options_of = |v: Option<&Value>| -> Vec<QuestionOption> {
        v.and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|o| match o {
                        Value::String(label) => Some(QuestionOption {
                            label: label.trim().chars().take(120).collect(),
                            description: String::new(),
                        }),
                        Value::Object(_) => Some(QuestionOption {
                            label: clip(o.get("label"), 120),
                            description: clip(o.get("description"), 200),
                        }),
                        _ => None,
                    })
                    .filter(|o| !o.label.is_empty())
                    .take(6)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut items: Vec<QuestionItem> = args
        .get("questions")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(4)
                .map(|q| QuestionItem {
                    header: clip(q.get("header"), 40),
                    question: clip(q.get("question"), 400),
                    options: options_of(q.get("options")),
                    multiple: q.get("multiple").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        items.push(QuestionItem {
            header: clip(args.get("header"), 40),
            question: clip(args.get("question"), 400),
            options: options_of(args.get("options")),
            multiple: args
                .get("multiple")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    items.retain(|q| !q.question.is_empty() && q.options.len() >= 2);
    items
}

/// Show an `ask_user` question. False when the arguments are unusable.
fn ask_question(call: &ToolCall, sink: &dyn Sink) -> bool {
    let Ok(args) = tools::parse_arguments(&call.arguments) else {
        return false;
    };
    let items = parse_questions(&args);
    let Some(first) = items.first().cloned() else {
        return false;
    };
    sink.emit(ChatStreamEvent::Question {
        id: call.id.clone(),
        question: first.question,
        options: first.options.into_iter().map(|o| o.label).collect(),
        multiple: first.multiple,
        items,
    });
    true
}

/// `update_todos`: show the model's to-do list.
fn update_todos(call: &ToolCall, sink: &dyn Sink) -> String {
    let Ok(args) = tools::parse_arguments(&call.arguments) else {
        return "Error: the to-do list could not be read.".into();
    };
    let items: Vec<TodoItem> = args
        .get("items")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(20)
                .filter_map(|item| {
                    let text: String = item
                        .get("text")
                        .and_then(Value::as_str)?
                        .trim()
                        .chars()
                        .take(200)
                        .collect();
                    if text.is_empty() {
                        return None;
                    }
                    let status = match item.get("status").and_then(Value::as_str).unwrap_or("") {
                        "done" | "completed" | "complete" => "done",
                        "in_progress" | "doing" | "active" => "in_progress",
                        _ => "pending",
                    };
                    Some(TodoItem {
                        text,
                        status: status.into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if items.is_empty() {
        return "Error: send the whole list, each item with text and status.".into();
    }
    let done = items.iter().filter(|i| i.status == "done").count();
    let total = items.len();
    sink.emit(ChatStreamEvent::Todos { items });
    format!(
        "The list is shown to the user ({done} of {total} done). Update it as you finish steps."
    )
}

/// `save_skill`: save or improve a skill at once, with a note and Undo in
/// the chat.
fn save_skill(state: &AppState, call: &ToolCall, source: &str, sink: &dyn Sink) -> String {
    let args = tools::parse_arguments(&call.arguments).unwrap_or(Value::Null);
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let (name, description, body) = (text("name"), text("description"), text("instructions"));
    sink.emit(ChatStreamEvent::ToolCall {
        id: call.id.clone(),
        name: "save_skill".into(),
        title: "Learned a skill".into(),
        detail: name.chars().take(120).collect(),
        group: "skills".into(),
        arguments: serde_json::to_string_pretty(&args)
            .unwrap_or_default()
            .chars()
            .take(3000)
            .collect(),
    });
    match state.skills.learn(&name, &description, &body) {
        Ok(learned) => {
            let action = if learned.created {
                "created"
            } else {
                "updated"
            };
            state.tools.record_event(
                &format!("{source}:save_skill"),
                "skills",
                &learned.skill.name,
                "success",
                Some(action),
            );
            sink.emit(ChatStreamEvent::SkillChange {
                id: call.id.clone(),
                change_id: learned.change_id,
                name: learned.skill.name.clone(),
                action: action.into(),
            });
            sink.emit(ChatStreamEvent::ToolResult {
                id: call.id.clone(),
                ok: true,
                summary: if learned.created {
                    "Saved".into()
                } else {
                    "Improved".into()
                },
                output: body.chars().take(4000).collect(),
            });
            format!(
                "{} the skill \"{}\". The user sees a note with Undo.",
                if learned.created { "Saved" } else { "Improved" },
                learned.skill.name
            )
        }
        Err(e) => {
            sink.emit(ChatStreamEvent::ToolResult {
                id: call.id.clone(),
                ok: false,
                summary: e.message.clone(),
                output: String::new(),
            });
            format!("Error: {}", e.message)
        }
    }
}

/// Subagents that may run at once.
const MAX_SUBAGENTS: usize = 4;

/// Passes a subagent's activity to the chat as one-line steps, and keeps
/// its answer.
struct SubSink<'a> {
    parent: &'a dyn Sink,
    id: String,
    text: Mutex<String>,
    thinking_shown: std::sync::atomic::AtomicBool,
}

impl Sink for SubSink<'_> {
    fn emit(&self, event: ChatStreamEvent) {
        let step = |step: String| ChatStreamEvent::SubagentStep {
            id: self.id.clone(),
            step,
        };
        match event {
            ChatStreamEvent::Token { token } => {
                let mut text = self.text.lock().unwrap_or_else(|e| e.into_inner());
                if text.is_empty() {
                    self.parent.emit(step("Writing its report".into()));
                }
                text.push_str(&token);
            }
            ChatStreamEvent::Reasoning { .. } => {
                if !self.thinking_shown.swap(true, Ordering::Relaxed) {
                    self.parent.emit(step("Thinking".into()));
                }
            }
            ChatStreamEvent::Status { label } => self.parent.emit(step(label)),
            ChatStreamEvent::ToolCall { title, detail, .. } => {
                self.thinking_shown.store(false, Ordering::Relaxed);
                self.parent.emit(step(if detail.is_empty() {
                    title
                } else {
                    format!("{title}: {detail}")
                }));
            }
            ChatStreamEvent::Citation { .. } => self.parent.emit(event),
            _ => {}
        }
    }
}

/// Run the round's `run_subagent` calls side by side. Each gets a fresh
/// context with the chat's model and read-only tools. Returns what goes
/// back to the model, by call id.
fn run_subagents(
    state: &AppState,
    parent: &AgentRun,
    calls: &[&ToolCall],
    next_citation: &mut usize,
    cancel: &AtomicBool,
    sink: &dyn Sink,
) -> HashMap<String, String> {
    let system = match parent.turns.first() {
        Some(ChatTurn::System(s)) => format!("{s}\n\n{SUBAGENT_PROMPT}"),
        _ => SUBAGENT_PROMPT.to_string(),
    };
    let base = *next_citation;
    let jobs: Vec<(String, String, String, usize)> = calls
        .iter()
        .enumerate()
        .map(|(i, call)| {
            let args = tools::parse_arguments(&call.arguments).unwrap_or(Value::Null);
            let get = |k: &str| {
                args.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };
            let task = get("task");
            let mut title: String = get("title").chars().take(80).collect();
            if title.is_empty() {
                title = task
                    .split_whitespace()
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            sink.emit(ChatStreamEvent::ToolCall {
                id: call.id.clone(),
                name: "run_subagent".into(),
                title: "Subagent".into(),
                detail: title.clone(),
                group: "subagent".into(),
                arguments: serde_json::to_string_pretty(&args)
                    .unwrap_or_default()
                    .chars()
                    .take(3000)
                    .collect(),
            });
            (call.id.clone(), title, task, base + i * 50)
        })
        .collect();
    *next_citation = base + calls.len() * 50;

    let outcomes: Vec<(String, String, Result<String, String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .into_iter()
            .map(|(id, title, task, first_citation)| {
                let system = system.clone();
                scope.spawn(move || {
                    if task.is_empty() {
                        return (id, title, Err("The subagent needs a task.".to_string()));
                    }
                    let child = AgentRun {
                        job_id: format!("{}:{id}", parent.job_id),
                        target: parent.target.clone(),
                        turns: vec![
                            ChatTurn::System(system),
                            ChatTurn::User {
                                text: task.clone(),
                                images: Vec::new(),
                            },
                        ],
                        answer_tokens: parent.answer_tokens,
                        temperature: parent.temperature,
                        effort: parent.effort.clone(),
                        thinks: parent.thinks,
                        perms: Permissions {
                            web: parent.perms.web,
                            notes: parent.perms.notes,
                            files: parent.perms.files,
                            skills: parent.perms.skills,
                            shell: parent.perms.shell,
                            ..Default::default()
                        },
                        use_tools: true,
                        max_rounds: 10,
                        context_tokens: parent.context_tokens,
                        auto_approve: HashSet::new(),
                        approval_mode: ApprovalMode::Plan,
                        source: parent.source,
                        first_citation,
                        user_message: task,
                        read_top_page: false,
                        jail: parent.jail.clone(),
                    };
                    let sub = SubSink {
                        parent: sink,
                        id: id.clone(),
                        text: Mutex::new(String::new()),
                        thinking_shown: std::sync::atomic::AtomicBool::new(false),
                    };
                    sink.emit(ChatStreamEvent::SubagentStep {
                        id: id.clone(),
                        step: "Starting".into(),
                    });
                    let result = run(state, child, cancel, &sub);
                    let text = sub.text.into_inner().unwrap_or_else(|e| e.into_inner());
                    let result = match result {
                        Ok(done) if done.cancelled => Err("Stopped".to_string()),
                        Ok(_) if text.trim().is_empty() => {
                            Err("It finished without a report.".to_string())
                        }
                        Ok(_) => Ok(text),
                        Err(e) => Err(e.message),
                    };
                    (id, title, result)
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });

    let mut out = HashMap::new();
    for (id, title, result) in outcomes {
        match result {
            Ok(text) => {
                sink.emit(ChatStreamEvent::ToolResult {
                    id: id.clone(),
                    ok: true,
                    summary: "Done".into(),
                    output: text.chars().take(8000).collect(),
                });
                out.insert(
                    id,
                    format!("Report from the subagent \"{title}\":\n\n{text}"),
                );
            }
            Err(message) => {
                sink.emit(ChatStreamEvent::ToolResult {
                    id: id.clone(),
                    ok: false,
                    summary: message.clone(),
                    output: String::new(),
                });
                out.insert(
                    id,
                    format!("Error: the subagent \"{title}\" failed: {message}"),
                );
            }
        }
    }
    out
}

const SUBAGENT_PROMPT: &str = "You are a subagent: a helper that works on one part of a bigger task for LocalAide, the main assistant. Your tools can search and read, but not change anything. Work efficiently and stay on your task. When you are done, reply with a short report for the main assistant: what you found, with file paths, commands and sources [n]. Do not ask questions; if something is unclear, say what you assumed.";

/// Run one tool call and return what goes back to the model.
#[allow(clippy::too_many_arguments)]
fn run_call(
    state: &AppState,
    job_id: &str,
    call: &ToolCall,
    specs: &[ToolSpec],
    auto_approve: &mut HashSet<String>,
    (mode, source, jail): (ApprovalMode, &'static str, &WorkspaceJail),
    max_chars: usize,
    next_citation: &mut usize,
    cancel: &AtomicBool,
    sink: &dyn Sink,
) -> String {
    let fail = |title: &str, message: String| {
        sink.emit(ChatStreamEvent::ToolCall {
            id: call.id.clone(),
            name: call.name.clone(),
            title: title.to_string(),
            detail: String::new(),
            group: "mcp".into(),
            arguments: call.arguments.chars().take(3000).collect(),
        });
        sink.emit(ChatStreamEvent::ToolResult {
            id: call.id.clone(),
            ok: false,
            summary: message.clone(),
            output: String::new(),
        });
        format!("Error: {message}")
    };
    let Some(spec) = specs.iter().find(|s| s.name == call.name) else {
        let names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        return fail(
            &call.name,
            format!(
                "There is no tool called {}. The tools are: {}.",
                call.name,
                names.join(", ")
            ),
        );
    };
    let args = match tools::parse_arguments(&call.arguments) {
        Ok(args) => args,
        Err(e) => return fail(&call.name, e.message),
    };
    let (title, detail) = tools::describe(spec, &args);
    sink.emit(ChatStreamEvent::ToolCall {
        id: call.id.clone(),
        name: spec.name.clone(),
        title: title.clone(),
        detail: detail.clone(),
        group: spec.kind.group().to_string(),
        arguments: serde_json::to_string_pretty(&args)
            .unwrap_or_default()
            .chars()
            .take(3000)
            .collect(),
    });

    if let Some(reason) = spec.refusal(&args) {
        state.tools.record_event(
            &format!("{source}:{}", spec.name),
            spec.kind.group(),
            &detail,
            "blocked",
            Some(&reason),
        );
        sink.emit(ChatStreamEvent::ToolResult {
            id: call.id.clone(),
            ok: false,
            summary: "Blocked".into(),
            output: reason.clone(),
        });
        return format!(
            "Error: this command is not allowed and did not run. {reason} Tell the user, and suggest a safer way if there is one."
        );
    }

    let needs = spec.needs_approval_for(&args);
    if needs && mode == ApprovalMode::Plan {
        state.tools.record_event(
            &format!("{source}:{}", spec.name),
            spec.kind.group(),
            &detail,
            "skipped",
            Some("Plan mode only reads"),
        );
        sink.emit(ChatStreamEvent::ToolResult {
            id: call.id.clone(),
            ok: false,
            summary: "Not run: read-only".into(),
            output: "Plan mode and subagents only read. Switch to a build mode to make changes."
                .into(),
        });
        return "This run only reads (Plan mode or a subagent), so this change did not run. Describe it in the plan or report instead.".into();
    }
    let allowed_here = auto_approve.contains(&spec.name);
    let wait = must_ask(mode, needs, allowed_here, || spec.risky(&args, jail));
    let mut note = None;
    if needs && !wait {
        note = if allowed_here {
            Some("allowed for this chat")
        } else {
            mode.note()
        };
    }
    if wait {
        let approval_id = format!("{job_id}:{}", call.id);
        let rx = state.agent.approvals.ask(&approval_id);
        sink.emit(ChatStreamEvent::ToolApproval {
            approval_id: approval_id.clone(),
            id: call.id.clone(),
            title: title.clone(),
            detail: detail.clone(),
            preview: approval_preview(spec, &args),
        });
        match state
            .agent
            .approvals
            .wait(&approval_id, &rx, cancel, APPROVAL_TIMEOUT)
        {
            Decision::Deny => {
                state.tools.record_event(
                    &format!("{source}:{}", spec.name),
                    spec.kind.group(),
                    &detail,
                    "denied",
                    Some("The user said no"),
                );
                sink.emit(ChatStreamEvent::ToolResult {
                    id: call.id.clone(),
                    ok: false,
                    summary: "You said no".into(),
                    output: String::new(),
                });
                return "The user declined this action. Do not try it again; continue without it and say what you could not do.".into();
            }
            Decision::AllowForChat => {
                auto_approve.insert(spec.name.clone());
            }
            Decision::Allow => {}
        }
    }

    if let Some(n) = note
        && spec.kind != tools::ToolKind::RunCommand
    {
        state.tools.record_event(
            &format!("{source}:{}", spec.name),
            spec.kind.group(),
            &detail,
            "approved",
            Some(n),
        );
    }
    let ctx = tools::RunContext {
        source,
        note,
        cancel,
        jail,
    };
    let outcome = tools::run(state, spec, &args, max_chars, next_citation, &ctx);
    emit_citations(sink, outcome.citations);
    sink.emit(ChatStreamEvent::ToolResult {
        id: call.id.clone(),
        ok: outcome.ok,
        summary: outcome.summary,
        output: outcome.for_user,
    });
    outcome.for_model
}

/// Whether a call waits for the user's OK. Plan mode never gets here for
/// a change (it refuses first).
fn must_ask(
    mode: ApprovalMode,
    needs: bool,
    allowed_here: bool,
    risky: impl FnOnce() -> bool,
) -> bool {
    needs
        && !allowed_here
        && match mode {
            ApprovalMode::Ask => true,
            ApprovalMode::Smart => risky(),
            ApprovalMode::Plan | ApprovalMode::Chat | ApprovalMode::Auto => false,
        }
}

/// What the user sees before saying yes.
fn approval_preview(spec: &ToolSpec, args: &Value) -> String {
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let preview = match spec.kind {
        tools::ToolKind::WriteFile => text("content"),
        tools::ToolKind::SaveNote => format!("{}\n\n{}", text("title"), text("content")),
        tools::ToolKind::RunCommand => {
            let cwd = text("cwd");
            format!(
                "$ {}{}",
                text("command"),
                if cwd.trim().is_empty() {
                    String::new()
                } else {
                    format!("\n(in workspace/{cwd})")
                }
            )
        }
        _ => serde_json::to_string_pretty(args).unwrap_or_default(),
    };
    let cut: String = preview.chars().take(1500).collect();
    if cut.len() < preview.len() {
        format!("{cut}\n…")
    } else {
        cut
    }
}

/// For models without tools: search the web and the user's notes first,
/// as earlier versions did, and put the results in the system prompt.
fn add_gathered_context(
    state: &AppState,
    run: &mut AgentRun,
    next_citation: &mut usize,
    sink: &dyn Sink,
) {
    let mut blocks = Vec::new();
    let query = run.user_message.trim().to_string();
    if query.is_empty() {
        return;
    }
    let never = AtomicBool::new(false);
    let jail = run.jail.clone();
    let gather_ctx = tools::RunContext {
        source: run.source,
        note: None,
        cancel: &never,
        jail: &jail,
    };
    if run.perms.notes {
        status(sink, "Looking in your notes and files");
        let spec = tools::builtin_specs(&Permissions {
            notes: true,
            ..Default::default()
        })
        .into_iter()
        .find(|s| s.name == "search_notes");
        if let Some(spec) = spec {
            let args = serde_json::json!({ "query": query });
            let outcome = tools::run(state, &spec, &args, 6000, next_citation, &gather_ctx);
            if !outcome.citations.is_empty() {
                blocks.push(outcome.for_model);
                emit_citations(sink, outcome.citations);
            }
        }
    }
    if run.perms.web {
        let short: String = query.chars().take(60).collect();
        status(sink, format!("Searching the web for “{}”", short.trim()));
        let specs = tools::builtin_specs(&Permissions {
            web: true,
            ..Default::default()
        });
        if let Some(search) = specs.iter().find(|s| s.name == "web_search") {
            let args = serde_json::json!({ "query": query, "max_results": 5 });
            let outcome = tools::run(state, search, &args, 8000, next_citation, &gather_ctx);
            let top = outcome.citations.first().map(|c| c.source.clone());
            if outcome.ok && !outcome.citations.is_empty() {
                blocks.push(outcome.for_model);
                emit_citations(sink, outcome.citations);
            } else {
                status(sink, outcome.summary);
            }
            if run.read_top_page
                && let Some(url) = top
                && let Some(read) = specs.iter().find(|s| s.name == "read_web_page")
            {
                let host = url::Url::parse(&url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_else(|| url.clone());
                status(sink, format!("Reading {host}"));
                let args = serde_json::json!({ "url": url });
                let outcome = tools::run(state, read, &args, 6000, next_citation, &gather_ctx);
                if outcome.ok {
                    blocks.push(outcome.for_model);
                    emit_citations(sink, outcome.citations);
                }
            }
        }
    }
    if blocks.is_empty() {
        return;
    }
    let context = blocks.join("\n\n");
    match run.turns.first_mut() {
        Some(ChatTurn::System(system)) => {
            system.push_str("\n\n");
            system.push_str(&context);
        }
        _ => run.turns.insert(0, ChatTurn::System(context)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(text: &str) -> ChatTurn {
        ChatTurn::User {
            text: text.into(),
            images: vec![],
        }
    }

    #[test]
    fn each_mode_asks_as_its_menu_says() {
        use ApprovalMode::*;
        assert_eq!(ApprovalMode::parse(Some("plan")), Plan);
        assert_eq!(ApprovalMode::parse(Some("smart")), Smart);
        assert_eq!(ApprovalMode::parse(Some("auto")), Auto);
        assert_eq!(ApprovalMode::parse(None), Ask);
        // Ask before changes: every change asks, unless allowed for the chat.
        assert!(must_ask(Ask, true, false, || false));
        assert!(!must_ask(Ask, true, true, || true));
        assert!(!must_ask(Ask, false, false, || true));
        // Autonomous: only risky changes ask.
        assert!(must_ask(Smart, true, false, || true));
        assert!(!must_ask(Smart, true, false, || false));
        // Full access never asks.
        assert!(!must_ask(Auto, true, false, || true));
    }

    #[test]
    fn questions_come_in_both_forms() {
        let new = serde_json::json!({"questions": [
            {"header": "Clarify", "question": "What did you mean?", "options": [
                {"label": "Random graph", "description": "A network picture"},
                {"label": "Random chart"}
            ]},
            {"question": "Bad one", "options": ["only one"]}
        ]});
        let items = parse_questions(&new);
        assert_eq!(items.len(), 1, "a question with one option is dropped");
        assert_eq!(items[0].header, "Clarify");
        assert_eq!(items[0].options[0].description, "A network picture");
        let old =
            serde_json::json!({"question": "Which?", "options": ["A", "B"], "multiple": true});
        let items = parse_questions(&old);
        assert_eq!(items[0].options.len(), 2);
        assert!(items[0].multiple);
    }

    #[test]
    fn old_tool_results_go_first_then_old_turns() {
        let mut turns = vec![
            ChatTurn::System("system".into()),
            user(&"a".repeat(400)),
            ChatTurn::Assistant {
                text: "b".repeat(400),
                tool_calls: vec![],
            },
            user("now"),
            ChatTurn::Assistant {
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: "web_search".into(),
                    arguments: "{}".into(),
                }],
            },
            ChatTurn::Tool {
                call_id: "1".into(),
                name: "web_search".into(),
                content: "x".repeat(4000),
            },
            ChatTurn::Tool {
                call_id: "2".into(),
                name: "read_file".into(),
                content: "y".repeat(400),
            },
        ];
        fit_turns(&mut turns, 400);
        assert!(matches!(&turns[0], ChatTurn::System(_)));
        assert!(
            matches!(&turns.last().unwrap(), ChatTurn::Tool { content, .. } if content.len() == 400)
        );
        assert!(turns.iter().any(
            |t| matches!(t, ChatTurn::Tool { content, .. } if content.starts_with("[Earlier"))
        ));
        assert!(
            turns
                .iter()
                .any(|t| matches!(t, ChatTurn::User { text, .. } if text == "now"))
        );
        assert!(estimate_tokens(&turns) <= 400);
    }
}
