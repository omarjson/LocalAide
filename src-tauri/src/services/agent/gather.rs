//! The tool-less fallback: for models that cannot use tools, search the
//! web and the user's notes first and put the results in the prompt.

use std::sync::atomic::AtomicBool;

use crate::app::state::AppState;
use crate::services::inference::openai_stream::ChatTurn;

use super::tools;
use super::tools::Permissions;
use super::{AgentRun, Sink, emit_citations, status};

/// For models without tools: search the web and the user's notes first,
/// as earlier versions did, and put the results in the system prompt.
pub(crate) fn add_gathered_context(
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
