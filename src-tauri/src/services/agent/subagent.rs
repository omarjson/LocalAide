//! Subagents: one part of a bigger task, run side by side with a fresh
//! context and read-only tools, reporting back to the main assistant.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use crate::app::events::ChatStreamEvent;
use crate::app::state::AppState;
use crate::services::inference::openai_stream::{ChatTurn, ToolCall};

use super::tools;
use super::tools::Permissions;
use super::{AgentRun, ApprovalMode, Sink, run_inner, status};

/// Subagents that may run at once.
pub(crate) const MAX_SUBAGENTS: usize = 4;

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
            tokens: None,
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
pub(crate) fn run_subagents(
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
                        // Subagent answers don't land in the usage log
                        // (run_inner skips them), so the folder is moot.
                        folder: None,
                        session: None,
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
                        tokens: None,
                    });
                    let result = run_inner(state, child, cancel, &sub);
                    let text = sub.text.into_inner().unwrap_or_else(|e| e.into_inner());
                    let result = match result {
                        Ok(done) if done.cancelled => Err("Stopped".to_string()),
                        Ok(_) if text.trim().is_empty() => {
                            Err("It finished without a report.".to_string())
                        }
                        Ok(done) => {
                            // The model only reports exact tokens when a
                            // round ends, so the total lands here, with the
                            // final step, rather than as a mid-run guess.
                            let used = done.prompt_tokens.unwrap_or(0) + done.tokens;
                            sink.emit(ChatStreamEvent::SubagentStep {
                                id: id.clone(),
                                step: "Done".into(),
                                tokens: Some(used),
                            });
                            Ok(text)
                        }
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
