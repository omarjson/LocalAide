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
pub mod tools;

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use crate::app::error::AppError;
use crate::app::events::ChatStreamEvent;
use crate::app::state::AppState;
use crate::domain::memory::Citation;
use crate::services::inference::openai_stream::{
    Adjustment, ChatTurn, Delta, Effort, TOOLS_UNSUPPORTED, ToolCall, initial_options,
    stream_with_retries,
};
use crate::services::inference::router::ChatTarget;
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

/// How much the chat asks before a tool runs (the switch in the chat
/// header). Blocked commands never run, whatever the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalMode {
    /// Ask for every tool that changes something.
    #[default]
    Ask,
    /// Allowed for this chat: no questions until the chat ends.
    Chat,
    /// Every chat: no questions.
    Auto,
}

impl ApprovalMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("chat") => Self::Chat,
            Some("auto") => Self::Auto,
            _ => Self::Ask,
        }
    }

    fn note(self) -> Option<&'static str> {
        match self {
            Self::Ask => None,
            Self::Chat => Some("allowed for this chat"),
            Self::Auto => Some("auto-approved"),
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
        for call in &calls {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            tool_calls_run += 1;
            if call.name == "ask_user" && specs.iter().any(|s| s.name == "ask_user") {
                if ask_question(call, sink) {
                    asked = true;
                    break;
                }
                run.turns.push(ChatTurn::Tool {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    content: "Error: ask_user needs a question and 2 to 6 options.".into(),
                });
                continue;
            }
            let content = run_call(
                state,
                &run.job_id,
                call,
                &specs,
                &mut run.auto_approve,
                (run.approval_mode, run.source),
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

/// Show an `ask_user` question. False when the arguments are unusable.
fn ask_question(call: &ToolCall, sink: &dyn Sink) -> bool {
    let Ok(args) = tools::parse_arguments(&call.arguments) else {
        return false;
    };
    let question: String = args
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .chars()
        .take(400)
        .collect();
    let options: Vec<String> = args
        .get("options")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(|o| o.trim().chars().take(120).collect::<String>())
                .filter(|o| !o.is_empty())
                .take(6)
                .collect()
        })
        .unwrap_or_default();
    if question.is_empty() || options.len() < 2 {
        return false;
    }
    sink.emit(ChatStreamEvent::Question {
        id: call.id.clone(),
        question,
        options,
        multiple: args
            .get("multiple")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    });
    true
}

/// Run one tool call and return what goes back to the model.
#[allow(clippy::too_many_arguments)]
fn run_call(
    state: &AppState,
    job_id: &str,
    call: &ToolCall,
    specs: &[ToolSpec],
    auto_approve: &mut HashSet<String>,
    (mode, source): (ApprovalMode, &'static str),
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
    let mut note = None;
    if needs && mode != ApprovalMode::Ask {
        note = mode.note();
    }
    if needs && mode == ApprovalMode::Ask && auto_approve.contains(&spec.name) {
        note = Some("allowed for this chat");
    }
    if needs && mode == ApprovalMode::Ask && !auto_approve.contains(&spec.name) {
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
    let gather_ctx = tools::RunContext {
        source: run.source,
        note: None,
        cancel: &never,
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
