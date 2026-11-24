//! Running one tool call: finding its spec, asking the user where needed,
//! and returning what goes back to the model.

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

use serde_json::Value;

use crate::app::events::ChatStreamEvent;
use crate::app::state::AppState;
use crate::config::Capability;
use crate::services::inference::openai_stream::ToolCall;
use crate::services::tools::sandbox::WorkspaceJail;

use super::approval::{APPROVAL_TIMEOUT, Decision};
use super::tools;
use super::tools::ToolSpec;
use super::{ApprovalMode, Sink, emit_citations};

/// Run one tool call and return what goes back to the model.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_call(
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
    // Per-capability auto-approve (N9): the Settings toggles. A capability
    // approval is not "allowed for this chat", so the note below names it
    // separately; the decision itself just stops the prompt.
    let capability_approved = capability_auto_approved(state, spec, &args, jail);
    let wait = must_ask(mode, needs, allowed_here || capability_approved, || {
        spec.risky(&args, jail)
    });
    let mut note = None;
    if needs && !wait {
        note = if allowed_here {
            Some("allowed for this chat")
        } else if capability_approved {
            Some("auto-approved in Settings")
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
/// Which capability a tool call belongs to, for per-capability
/// auto-approve (N9). Reads and web lookups never need approval on their
/// own, so their toggles are posture; the ones that change behaviour are
/// edits, commands and external tools. A command that is hard to undo
/// counts as "all commands", a tame one as "safe commands" — and "all
/// commands" covers safe ones (see `CapabilityAutoApprove::approves`).
fn capability_for(spec: &ToolSpec, args: &Value, jail: &WorkspaceJail) -> Capability {
    match spec.kind {
        tools::ToolKind::WebSearch | tools::ToolKind::ReadPage => Capability::Browser,
        tools::ToolKind::WriteFile
        | tools::ToolKind::MakeDocument
        | tools::ToolKind::SaveNote
        | tools::ToolKind::SaveSkill => Capability::EditFiles,
        tools::ToolKind::RunCommand => {
            if spec.risky(args, jail) {
                Capability::AllCommands
            } else {
                Capability::SafeCommands
            }
        }
        tools::ToolKind::Mcp => Capability::ExternalTools,
        _ => Capability::ReadFiles,
    }
}

/// Whether the configured capabilities auto-approve this call. Fail-open:
/// the policy snapshot defaults to read-only, so a policy that was never
/// set (or a fresh install) asks instead of auto-approving.
fn capability_auto_approved(
    state: &AppState,
    spec: &ToolSpec,
    args: &Value,
    jail: &WorkspaceJail,
) -> bool {
    let caps = state.tools.policy().capabilities();
    caps.approves(capability_for(spec, args, jail))
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
