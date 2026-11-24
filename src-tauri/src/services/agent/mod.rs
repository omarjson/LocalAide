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

mod context;
mod dispatch;
mod failover;
mod gather;
mod handlers;
mod subagent;
mod types;

pub use context::fit_turns;
pub use types::{AgentRun, AgentState, ApprovalMode, RunResult, Sink};

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::app::error::AppError;
use crate::app::events::ChatStreamEvent;
use crate::app::state::AppState;
use crate::domain::memory::Citation;
use crate::services::inference::openai_stream::{
    Adjustment, ChatTurn, Delta, TOOLS_UNSUPPORTED, ToolCall, initial_options, stream_with_retries,
};
use crate::services::inference::router::ChatTarget;
use crate::services::usage;

use context::{estimate_tokens, round_limit, schema_tokens};
use dispatch::run_call;
use failover::next_failover_target;
use gather::add_gathered_context;
use handlers::{ask_question, save_skill, update_todos};
use subagent::{MAX_SUBAGENTS, run_subagents};
use tools::ToolSpec;

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
/// Runs one answer and records its usage. The real work lives in
/// `run_inner`; subagent runs call that directly, so only whole answers
/// (app chat and shared links) land in the usage log.
pub fn run(
    state: &AppState,
    run: AgentRun,
    cancel: &AtomicBool,
    sink: &dyn Sink,
) -> Result<RunResult, AppError> {
    let started = Instant::now();
    let input_tokens = estimate_tokens(&run.turns);
    let source = run.source;
    // Owned copies taken before `run` moves into `run_inner`.
    let folder = run.folder.clone();
    let session = run.session.clone();
    let (model, provider, local) = match &run.target {
        ChatTarget::Local => (
            state
                .inference_engine
                .status()
                .active_model
                .unwrap_or_else(|| "local model".to_string()),
            "This computer".to_string(),
            true,
        ),
        ChatTarget::Remote(remote) => (remote.model.clone(), remote.label.clone(), false),
    };
    let job_id = run.job_id.clone();
    let result = run_inner(state, run, cancel, sink);
    // Notes that came after the model's last step are dropped here; the
    // chat sends them as the next message instead.
    let _ = state.agent.take_steers(&job_id);
    if let Ok(done) = &result
        && (source == "chat" || source == "share")
    {
        let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let tok_s = if done.tokens > 0 && ms > 0 {
            Some(done.tokens as f64 / (ms as f64 / 1000.0))
        } else {
            None
        };
        usage::record(&usage::UsageRecord {
            ts: now_secs(),
            model,
            provider,
            local,
            input_tokens: done.prompt_tokens.unwrap_or(input_tokens),
            output_tokens: done.tokens,
            ms,
            ttft_ms: done.ttft_ms,
            tok_s,
            cancelled: done.cancelled,
            tool_calls: done.tool_calls,
            source: source.to_string(),
            folder,
            session,
            cache_tokens: done.cache_tokens,
        });
    }
    result
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_inner(
    state: &AppState,
    mut run: AgentRun,
    cancel: &AtomicBool,
    sink: &dyn Sink,
) -> Result<RunResult, AppError> {
    let engine = state.inference_engine.clone();
    let mut endpoint = engine.endpoint_for(&run.target)?;
    // Notes the user pinned ride in every chat while recall is on.
    if run.perms.notes
        && let Ok(pinned) = state.memory.pinned_context()
        && !pinned.is_empty()
    {
        run.turns.insert(1, ChatTurn::System(pinned));
    }
    // The no-tools tracking key follows the endpoint when failover switches
    // providers mid-run (see below).
    let mut key = format!("{}|{}", endpoint.url, endpoint.model);

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
    let mut ttft_ms: Option<u64> = None;
    let mut prompt_tokens: Option<usize> = None;
    let mut cache_tokens: Option<usize> = None;
    let mut first_prompt_tokens: Option<usize> = None;
    let mut seen_ids: HashSet<String> = HashSet::new();
    let client = engine.chat_client();
    // What the instructions cost, for the chat's context meter.
    let system_tokens: usize = run
        .turns
        .iter()
        .map(|t| match t {
            ChatTurn::System(s) => crate::services::inference::context::estimate_text_tokens(s) + 4,
            _ => 0,
        })
        .sum();
    let mut tool_tokens = schema_tokens(&opts.tools);

    let mut round = 0usize;
    // Providers already tried by failover this run, so a provider that
    // failed is never tried twice.
    let mut tried: HashSet<String> = HashSet::new();
    while round < run.max_rounds {
        if cancel.load(Ordering::Relaxed) {
            return Ok(RunResult {
                tokens: total_tokens,
                cancelled: true,
                finish_reason: "cancelled".into(),
                tool_calls: tool_calls_run,
                ttft_ms,
                prompt_tokens,
                cache_tokens,
                first_prompt_tokens,
                system_tokens,
                tool_tokens,
            });
        }
        // Notes the user sent while the answer ran join the conversation
        // before the model's next step.
        if round > 0 {
            for note in state.agent.take_steers(&run.job_id) {
                sink.emit(ChatStreamEvent::Steered { text: note.clone() });
                run.turns.push(ChatTurn::User {
                    text: format!("(The user added this while you were working.) {note}"),
                    images: Vec::new(),
                });
            }
        }
        let last_round = round + 1 == run.max_rounds;
        if last_round && !opts.tools.is_empty() {
            opts.forbid_tools = true;
        }
        // The tool schemas ride along with every request, so they come out
        // of the budget like any message.
        tool_tokens = schema_tokens(&opts.tools);
        let reserve = run.answer_tokens.max(256) as usize;
        fit_turns(
            &mut run.turns,
            run.context_tokens.saturating_sub(reserve + tool_tokens),
        );
        opts.max_tokens = round_limit(&run, &run.turns, tool_tokens);

        // Tokens delivered this round. A failure after the first token
        // must not fail over: the new provider would answer from scratch
        // and its text would land after the partial answer.
        let mut round_delivered = 0usize;
        let result = stream_with_retries(
            client,
            &endpoint,
            &mut run.turns,
            &mut opts,
            cancel,
            &mut |delta| {
                round_delivered += 1;
                match delta {
                    Delta::Content(token) => sink.emit(ChatStreamEvent::Token { token }),
                    Delta::Reasoning(text) => sink.emit(ChatStreamEvent::Reasoning { text }),
                }
            },
            &mut |label| status(sink, label),
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
            Err(e) if e.retryable && round_delivered == 0 => {
                match next_failover_target(state, &engine, &run.target, &mut tried) {
                    Some((next_target, next_endpoint)) => {
                        let from = endpoint.label.clone();
                        run.target = next_target;
                        endpoint = next_endpoint;
                        key = format!("{}|{}", endpoint.url, endpoint.model);
                        // Rebuild the flavor-dependent options for the new
                        // provider, keeping the tools state (a model that
                        // cannot use tools stays tool-less) and the
                        // last-round tool ban.
                        let forbid = opts.forbid_tools;
                        opts = initial_options(
                            endpoint.flavor,
                            opts.max_tokens,
                            run.temperature,
                            opts.tools.clone(),
                            run.effort.clone(),
                        );
                        opts.forbid_tools = forbid;
                        let to = match &run.target {
                            ChatTarget::Local => "the local model".to_string(),
                            ChatTarget::Remote(_) => endpoint.label.clone(),
                        };
                        status(sink, format!("{from} was busy — continued on {to}"));
                        // Same round again, on the new provider.
                        continue;
                    }
                    // Chain exhausted (or failover itself broke): the
                    // original error stands, unchanged.
                    None => return Err(e),
                }
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
        if ttft_ms.is_none()
            && let Some(first) = outcome.first_token_after
        {
            ttft_ms = Some(u64::try_from(first.as_millis()).unwrap_or(u64::MAX));
        }
        if outcome.prompt_tokens.is_some() {
            prompt_tokens = outcome.prompt_tokens;
            if round == 0 {
                first_prompt_tokens = outcome.prompt_tokens;
            }
        }
        if outcome.cache_tokens.is_some() {
            cache_tokens = outcome.cache_tokens;
        }
        total_tokens += outcome.completion_tokens.unwrap_or(outcome.pieces);
        if outcome.cancelled {
            return Ok(RunResult {
                tokens: total_tokens,
                cancelled: true,
                finish_reason: "cancelled".into(),
                tool_calls: tool_calls_run,
                ttft_ms,
                prompt_tokens,
                cache_tokens,
                first_prompt_tokens,
                system_tokens,
                tool_tokens,
            });
        }
        let wants_tools = !outcome.tool_calls.is_empty() && !specs.is_empty() && !opts.forbid_tools;
        if !wants_tools && !state.agent.has_steers(&run.job_id) {
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
        // A note sent while this round streamed has no tool round to ride
        // in on: steers are only read at the top of a later round, so
        // breaking here dropped the note (the chat then parked it as a new
        // message, which is why Steer looked like it did nothing). Falling
        // through runs one more round instead: the assistant turn is kept
        // below, and the next round reads the note before answering.

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
        ttft_ms,
        prompt_tokens,
        cache_tokens,
        first_prompt_tokens,
        system_tokens,
        tool_tokens,
    })
}
