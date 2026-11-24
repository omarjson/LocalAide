//! The tools chat can offer the model, and how each one runs.
//!
//! Built-in tools reach LocalAide's own features: web search and page
//! reading (only when the user turned Web on), the user's notes and
//! documents, saved skills, and the workspace folder. MCP servers add their
//! own tools. Tools that change something ask the user first.

use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::McpServerConfig;
use crate::domain::memory::Citation;
use crate::domain::tool::{ToolAction, ToolActionType, ToolExecutionRequest};
use crate::services::office;
use crate::services::tools::shell;

mod specs;
pub use specs::{builtin_specs, describe, mcp_specs, tool_name_for};

/// Which groups of tools this chat may use.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub web: bool,
    pub notes: bool,
    pub files: bool,
    pub skills: bool,
    /// Run terminal commands in the workspace.
    pub shell: bool,
    /// Ask the user a question with buttons (off in Autonomous mode).
    pub ask: bool,
    /// Keep a to-do list for a longer task (`update_todos`).
    pub todos: bool,
    /// Hand parts of a task to read-only subagents (`run_subagent`).
    pub subagents: bool,
    /// Save and improve skills learned from the chat (`save_skill`).
    pub learn_skills: bool,
    /// MCP servers switched on for this chat.
    pub mcp: Vec<McpServerConfig>,
}

impl Permissions {
    pub fn any(&self) -> bool {
        self.web || self.notes || self.files || self.skills || self.shell || !self.mcp.is_empty()
    }
}

/// What a tool is, for the picker of icons and the approval rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    WebSearch,
    ReadPage,
    SearchNotes,
    SaveNote,
    SuggestNote,
    ListSkills,
    UseSkill,
    ListFiles,
    ReadFile,
    SearchFiles,
    WriteFile,
    MakeDocument,
    RunCommand,
    AskUser,
    Todos,
    Subagent,
    SaveSkill,
    Mcp,
}

impl ToolKind {
    /// The icon group the chat shows for this tool.
    pub fn group(self) -> &'static str {
        match self {
            Self::WebSearch | Self::ReadPage => "web",
            Self::SearchNotes | Self::SaveNote | Self::SuggestNote => "notes",
            Self::ListSkills | Self::UseSkill | Self::SaveSkill => "skills",
            Self::Todos => "todos",
            Self::Subagent => "subagent",
            Self::ListFiles
            | Self::ReadFile
            | Self::SearchFiles
            | Self::WriteFile
            | Self::MakeDocument => "files",
            Self::RunCommand => "shell",
            Self::AskUser => "question",
            Self::Mcp => "mcp",
        }
    }

    /// Tools that change something wait for the user.
    pub fn changes_something(self) -> bool {
        matches!(
            self,
            Self::SaveNote | Self::WriteFile | Self::MakeDocument | Self::Mcp
        )
    }
}

/// One tool as offered to the model.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    /// The name the model calls.
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub kind: ToolKind,
    /// For MCP tools: the server and the tool's own name.
    pub mcp: Option<(McpServerConfig, String)>,
}

impl ToolSpec {
    pub fn schema(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters,
            }
        })
    }

    pub fn needs_approval(&self) -> bool {
        match &self.mcp {
            Some((server, _)) => !server.auto_approve,
            None => self.kind.changes_something(),
        }
    }

    /// Whether this call waits for the user. Terminal commands ask only
    /// when they change something; read-only ones run straight away.
    pub fn needs_approval_for(&self, args: &Value) -> bool {
        if self.kind == ToolKind::RunCommand {
            let command = args.get("command").and_then(Value::as_str).unwrap_or("");
            return shell::classify(command) != shell::Verdict::ReadOnly;
        }
        self.needs_approval()
    }

    /// Whether Autonomous mode still asks first: commands whose effects are
    /// hard to undo, replacing a file that exists, and MCP tools (LocalAide
    /// cannot tell what they do).
    pub fn risky(
        &self,
        args: &Value,
        jail: &crate::services::tools::sandbox::WorkspaceJail,
    ) -> bool {
        match self.kind {
            ToolKind::RunCommand => {
                let command = args.get("command").and_then(Value::as_str).unwrap_or("");
                shell::risky(command)
            }
            ToolKind::WriteFile => {
                let path = args.get("path").and_then(Value::as_str).unwrap_or("");
                jail.resolve_path(path).map(|p| p.exists()).unwrap_or(true)
            }
            ToolKind::Mcp => true,
            _ => false,
        }
    }

    /// A reason this call may never run, checked before asking the user.
    pub fn refusal(&self, args: &Value) -> Option<String> {
        if self.kind == ToolKind::RunCommand {
            let command = args.get("command").and_then(Value::as_str).unwrap_or("");
            if let shell::Verdict::Blocked(reason) = shell::classify(command) {
                return Some(reason);
            }
        }
        None
    }
}

/// Where a tool call comes from and how it was allowed, for the audit log.
pub struct RunContext<'a> {
    /// "chat", or "share" for requests through a shared link.
    pub source: &'a str,
    /// Set when approval was skipped by the chat's approval mode.
    pub note: Option<&'a str>,
    pub cancel: &'a std::sync::atomic::AtomicBool,
    /// The folder this chat works in.
    pub jail: &'a crate::services::tools::sandbox::WorkspaceJail,
}

/// Office Maker: builds a real .docx, .xlsx or .pptx from the blocks the
/// model sends, and writes it inside the chat's folder.
fn make_document(
    state: &AppState,
    args: &Value,
    jail: &crate::services::tools::sandbox::WorkspaceJail,
    max_chars: usize,
) -> ToolOutcome {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if path.is_empty() {
        return failed("The document needs a file name.");
    }
    let lower = path.to_lowercase();
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            if lower.ends_with(".xlsx") {
                "xlsx".to_string()
            } else if lower.ends_with(".pptx") {
                "pptx".to_string()
            } else {
                "docx".to_string()
            }
        });
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let blocks: Vec<office::DocBlock> = args
        .get("blocks")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|b| serde_json::from_value(b.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let sheets: Vec<office::Sheet> = args
        .get("sheets")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|b| serde_json::from_value(b.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let slides: Vec<office::Slide> = args
        .get("slides")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|b| serde_json::from_value(b.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let built = match kind.as_str() {
        "xlsx" => office::make_xlsx(&sheets),
        "pptx" => office::make_pptx(&title, &slides),
        _ => office::make_docx(&title, &blocks),
    };
    let bytes = match built {
        Ok(bytes) => bytes,
        Err(e) => return failed(e.message),
    };
    // The right ending beats whatever the model named it.
    let wanted = match kind.as_str() {
        "xlsx" => ".xlsx",
        "pptx" => ".pptx",
        _ => ".docx",
    };
    let path = if lower.ends_with(wanted) {
        path
    } else {
        let trimmed = path
            .trim_end_matches(".docx")
            .trim_end_matches(".xlsx")
            .trim_end_matches(".pptx");
        format!("{trimmed}{wanted}")
    };

    let written = match jail.write_bytes(&path, &bytes) {
        Ok(size) => size,
        Err(e) => return failed(e.message),
    };
    state.tools.record_event(
        "chat",
        "make_document",
        &path,
        "written",
        Some(&format!("{kind}, {written} bytes")),
    );
    let name = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
    let (for_model, _truncated) = cut(
        &format!(
            "The document {name} is on disk: {kind}, {written} bytes, in the chat's folder. Tell the user the file name."
        ),
        max_chars,
    );
    ToolOutcome {
        ok: true,
        for_model,
        summary: format!("Made {name}"),
        for_user: format!("{name} — {kind}, {} KB", written.div_ceil(1024)),
        citations: Vec::new(),
    }
}

/// Run a terminal command and describe the result for the model and the
/// chat card. Long output is cut; the full text is saved in the workspace.
fn run_command(state: &AppState, args: &Value, max_chars: usize, ctx: &RunContext) -> ToolOutcome {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let cwd = args.get("cwd").and_then(Value::as_str);
    let secs = args
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(shell::DEFAULT_TIMEOUT_SECS)
        .clamp(1, shell::MAX_TIMEOUT_SECS);
    let run = match state.tools.run_command_in(
        ctx.jail,
        ctx.source,
        &command,
        cwd,
        std::time::Duration::from_secs(secs),
        ctx.cancel,
        ctx.note,
    ) {
        Ok(r) => r,
        Err(e) => return failed(e.message),
    };
    let out = &run.output;
    let mut text = String::new();
    if !out.stdout.is_empty() {
        text.push_str(&out.stdout);
    }
    if !out.stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&out.stderr);
    }
    let limit = max_chars.max(4000);
    let (shown, cut_here) = cut(&text, limit);
    let mut saved = None;
    if cut_here || out.capture_cut {
        let dir = run.workspace.join(".localaide").join("command-output");
        let name = format!(
            "{}.txt",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(dir.join(&name), &text).is_ok() {
            saved = Some(format!(".localaide/command-output/{name}"));
        }
    }
    let exit = out
        .exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "none".into());
    let status_line = if out.timed_out {
        format!("The command was stopped after {secs} s (time limit).")
    } else if out.cancelled {
        "The command was stopped because the answer was cancelled.".to_string()
    } else {
        format!("Exit code {exit}, after {} ms.", out.millis)
    };
    let mut notes = Vec::new();
    if cut_here || out.capture_cut {
        notes.push(format!(
            "The output was shortened ({} bytes in total).{}",
            out.total_bytes,
            saved
                .as_ref()
                .map(|p| format!(" The full output is in the workspace file {p}; read it with read_file if you need more."))
                .unwrap_or_default()
        ));
    }
    let body = if shown.trim().is_empty() {
        "(no output)".to_string()
    } else {
        shown
    };
    let ok = out.exit_code == Some(0) && !out.timed_out && !out.cancelled;
    let for_model = format!(
        "$ {command}\n{status_line}\n{}\n--- output ---\n{body}",
        notes.join(" ")
    );
    let summary = if out.timed_out {
        format!("Stopped after {secs} s")
    } else if out.cancelled {
        "Stopped".to_string()
    } else if ok {
        format!("Done in {} ms", out.millis)
    } else {
        format!("Exit code {exit}")
    };
    // The chat card reads the first line as metadata.
    let for_user = format!(
        "{}\n{}",
        serde_json::json!({
            "exit": out.exit_code,
            "ms": out.millis,
            "timed_out": out.timed_out,
            "cut": cut_here || out.capture_cut,
            "saved": saved,
            "cwd": run.cwd,
        }),
        cut(&body, 4000).0
    );
    ToolOutcome {
        ok,
        for_model,
        summary,
        for_user,
        citations: Vec::new(),
    }
}

/// What a finished tool returns.
#[derive(Debug, Clone, Default)]
pub struct ToolOutcome {
    pub ok: bool,
    /// Sent back to the model.
    pub for_model: String,
    /// One line for the chat, for example "5 results".
    pub summary: String,
    /// Shown when the user opens the tool card.
    pub for_user: String,
    pub citations: Vec<Citation>,
}

fn failed(message: impl Into<String>) -> ToolOutcome {
    let message = message.into();
    ToolOutcome {
        ok: false,
        for_model: format!("Error: {message}"),
        summary: message.clone(),
        for_user: message,
        citations: Vec::new(),
    }
}

fn cut(text: &str, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        return (text.to_string(), false);
    }
    (text.chars().take(max).collect(), true)
}

/// Run one built-in or MCP tool. `next_citation` numbers new sources.
pub fn run(
    state: &AppState,
    spec: &ToolSpec,
    args: &Value,
    max_chars: usize,
    next_citation: &mut usize,
    ctx: &RunContext,
) -> ToolOutcome {
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let outcome = match spec.kind {
        ToolKind::WebSearch => web_search(state, &text("query"), args, next_citation),
        ToolKind::ReadPage => read_page(state, &text("url"), max_chars, next_citation),
        ToolKind::SearchNotes => search_notes(state, &text("query"), next_citation),
        ToolKind::SaveNote => save_note(state, &text("title"), &text("content")),
        ToolKind::SuggestNote => suggest_note(state, &text("title"), &text("content")),
        ToolKind::ListSkills => list_skills(state),
        ToolKind::UseSkill => use_skill(state, &text("name"), max_chars),
        ToolKind::ListFiles => workspace(
            state,
            ctx.jail,
            ToolActionType::ListDir,
            &text("path"),
            None,
            None,
            max_chars,
        ),
        ToolKind::ReadFile => workspace(
            state,
            ctx.jail,
            ToolActionType::ReadFile,
            &text("path"),
            None,
            None,
            max_chars,
        ),
        ToolKind::SearchFiles => workspace(
            state,
            ctx.jail,
            ToolActionType::SearchFiles,
            &text("path"),
            None,
            Some(text("query")),
            max_chars,
        ),
        ToolKind::WriteFile => workspace(
            state,
            ctx.jail,
            ToolActionType::WriteFile,
            &text("path"),
            Some(text("content")),
            None,
            max_chars,
        ),
        ToolKind::MakeDocument => make_document(state, args, ctx.jail, max_chars),
        ToolKind::RunCommand => run_command(state, args, max_chars, ctx),
        // Handled by the chat loop, which ends the answer with the question.
        ToolKind::AskUser => ToolOutcome {
            ok: true,
            for_model: "The question was shown to the user.".into(),
            summary: "Waiting for your answer".into(),
            ..Default::default()
        },
        // Run by the chat loop, which needs the stream and the model.
        ToolKind::Todos | ToolKind::Subagent | ToolKind::SaveSkill => {
            failed("This tool runs in the chat loop.")
        }
        ToolKind::Mcp => mcp_call(state, spec, args, max_chars),
    };
    // Workspace actions and commands write their own audit rows; the rest
    // are recorded here.
    if !matches!(
        spec.kind,
        ToolKind::ListFiles
            | ToolKind::ReadFile
            | ToolKind::SearchFiles
            | ToolKind::WriteFile
            | ToolKind::RunCommand
            | ToolKind::AskUser
            | ToolKind::Todos
            | ToolKind::Subagent
            | ToolKind::SaveSkill
    ) {
        let (_, detail) = describe(spec, args);
        state.tools.record_event(
            &format!("{}:{}", ctx.source, spec.name),
            spec.kind.group(),
            &detail,
            if outcome.ok { "success" } else { "error" },
            if outcome.ok {
                None
            } else {
                Some(outcome.summary.as_str())
            },
        );
    }
    outcome
}

fn web_search(
    state: &AppState,
    query: &str,
    args: &Value,
    next_citation: &mut usize,
) -> ToolOutcome {
    if query.trim().is_empty() {
        return failed("The search needs some words.");
    }
    let limit = args
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 8) as u32;
    let (provider, keys) = match crate::commands::web::web_settings(state) {
        Ok(v) => v,
        Err(e) => return failed(e.message),
    };
    let hits = match state.web.search(&provider, query, limit, &keys) {
        Ok(h) => h,
        Err(e) => return failed(format!("The web search failed: {}", e.message)),
    };
    if hits.is_empty() {
        return ToolOutcome {
            ok: true,
            for_model: "The search found nothing. Try other words.".into(),
            summary: "No results".into(),
            for_user: "The search found nothing.".into(),
            citations: Vec::new(),
        };
    }
    let mut lines = Vec::new();
    let mut citations = Vec::new();
    for hit in &hits {
        let index = *next_citation;
        *next_citation += 1;
        lines.push(format!(
            "[{index}] {}\n{}\n{}",
            hit.title,
            hit.url,
            hit.snippet.trim()
        ));
        citations.push(Citation {
            index,
            source: hit.url.clone(),
            title: hit.title.clone(),
            snippet: hit.snippet.clone(),
            score: 1.0,
        });
    }
    let body = lines.join("\n\n");
    ToolOutcome {
        ok: true,
        for_model: format!(
            "Web results for \"{query}\" (searched just now; they may be incomplete):\n\n{body}\n\nCite them as [n]. Read a page with read_web_page when a snippet is not enough."
        ),
        summary: format!("{} results", hits.len()),
        for_user: body,
        citations,
    }
}

fn read_page(
    state: &AppState,
    url: &str,
    max_chars: usize,
    next_citation: &mut usize,
) -> ToolOutcome {
    if url.trim().is_empty() {
        return failed("No address was given.");
    }
    let keys = match crate::commands::web::web_settings(state) {
        Ok((_, keys)) => keys,
        Err(e) => return failed(e.message),
    };
    let page = match state.web.read_page(url.trim(), "auto", &keys) {
        Ok(p) => p,
        Err(e) => return failed(format!("The page could not be read: {}", e.message)),
    };
    let (text, truncated) = cut(&page.content, max_chars);
    let index = *next_citation;
    *next_citation += 1;
    let title = page.title.clone().unwrap_or_else(|| page.url.clone());
    let chars = page.content.chars().count();
    ToolOutcome {
        ok: true,
        for_model: format!(
            "[{index}] {title}\n{}\n\n{text}{}",
            page.url,
            if truncated {
                "\n\n[The page was shortened to fit.]"
            } else {
                ""
            }
        ),
        summary: format!("Read {} characters", fmt_count(chars)),
        for_user: cut(&page.content, 4000).0,
        citations: vec![Citation {
            index,
            source: page.url.clone(),
            title,
            snippet: page.content.chars().take(300).collect(),
            score: 1.0,
        }],
    }
}

fn search_notes(state: &AppState, query: &str, next_citation: &mut usize) -> ToolOutcome {
    if query.trim().is_empty() {
        return failed("The search needs some words.");
    }
    // The meaning vector is used only when the embedding engine answers in
    // time; otherwise the search ranks by keywords.
    let vector = {
        let engine = state.embedding_engine.clone();
        let q = query.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(engine.embed_query(&q).ok());
        });
        rx.recv_timeout(std::time::Duration::from_secs(3))
            .ok()
            .flatten()
    };
    // Never degrade silently: say when the search could only match words.
    let keyword_note = if vector.is_none() {
        if state.embedding_engine.is_running() {
            " (Matched by words only this time: the meaning model did not answer in time.)"
        } else {
            " (Matched by words only: no meaning model is running, so related wording may be missed. The user can pick one in Search → Library status.)"
        }
    } else {
        ""
    };
    let recall = match state.memory.recall(query, vector.as_deref(), Some(900)) {
        Ok(r) => r,
        Err(e) => return failed(format!("Your notes could not be searched: {}", e.message)),
    };
    if recall.citations.is_empty() {
        return ToolOutcome {
            ok: true,
            for_model: format!(
                "Nothing in the user's notes or documents matches.{keyword_note} Say so if it matters."
            ),
            summary: "Nothing found".into(),
            for_user: "Nothing matched in your notes and documents.".into(),
            citations: Vec::new(),
        };
    }
    let mut lines = Vec::new();
    let mut citations = Vec::new();
    for cit in recall.citations {
        let index = *next_citation;
        *next_citation += 1;
        let name = std::path::Path::new(&cit.source)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| cit.title.clone());
        lines.push(format!(
            "[{index}] {name} — {}\n{}",
            cit.source,
            cit.snippet.trim()
        ));
        citations.push(Citation { index, ..cit });
    }
    let body = lines.join("\n\n");
    ToolOutcome {
        ok: true,
        for_model: format!(
            "From the user's notes and documents{keyword_note}:\n\n{body}\n\nAnswer from these, name the file you used, and cite it as [n]."
        ),
        summary: format!(
            "{} {}{}",
            citations.len(),
            if citations.len() == 1 {
                "match"
            } else {
                "matches"
            },
            if vector.is_none() {
                " · words only"
            } else {
                ""
            }
        ),
        for_user: body,
        citations,
    }
}

fn save_note(state: &AppState, title: &str, content: &str) -> ToolOutcome {
    if title.trim().is_empty() || content.trim().is_empty() {
        return failed("A note needs a title and some text.");
    }
    match state.memory.create_note(title.trim(), content, None) {
        Ok(note) => ToolOutcome {
            ok: true,
            for_model: format!("Saved the note \"{}\".", note.title),
            summary: "Saved".into(),
            for_user: content.chars().take(4000).collect(),
            citations: Vec::new(),
        },
        Err(e) => failed(format!("The note could not be saved: {}", e.message)),
    }
}

/// Offers a note for the review queue: nothing is saved until the user
/// accepts it on the Memory screen, so this needs no approval.
fn suggest_note(state: &AppState, title: &str, content: &str) -> ToolOutcome {
    if title.trim().is_empty() || content.trim().is_empty() {
        return failed("A note needs a title and some text.");
    }
    match state.memory.suggest_note(title.trim(), content.trim()) {
        Ok(_) => ToolOutcome {
            ok: true,
            for_model: format!(
                "The note \"{}\" is now waiting in the user's review queue on the Memory screen. Say in one short sentence that you suggested it for their notes.",
                title.trim()
            ),
            summary: "Suggested".into(),
            for_user: format!(
                "Suggested a note: \"{}\". Review it in Memory.",
                title.trim()
            ),
            citations: Vec::new(),
        },
        Err(e) => failed(format!("The suggestion could not be saved: {}", e.message)),
    }
}

fn list_skills(state: &AppState) -> ToolOutcome {
    match state.skills.list() {
        Ok(skills) if skills.is_empty() => ToolOutcome {
            ok: true,
            for_model: "The user has no saved skills.".into(),
            summary: "No skills".into(),
            for_user: "You have no saved skills yet.".into(),
            citations: Vec::new(),
        },
        Ok(skills) => {
            let lines: Vec<String> = skills
                .iter()
                .take(60)
                .map(|s| {
                    format!(
                        "- {}: {}",
                        s.name,
                        s.description.chars().take(160).collect::<String>()
                    )
                })
                .collect();
            let body = lines.join("\n");
            ToolOutcome {
                ok: true,
                for_model: format!(
                    "Saved skills:\n{body}\n\nCall use_skill with a name to read one."
                ),
                summary: format!("{} skills", skills.len()),
                for_user: body,
                citations: Vec::new(),
            }
        }
        Err(e) => failed(e.message),
    }
}

fn use_skill(state: &AppState, name: &str, max_chars: usize) -> ToolOutcome {
    let wanted = name.trim().trim_start_matches('/').to_lowercase();
    let skills = match state.skills.list() {
        Ok(s) => s,
        Err(e) => return failed(e.message),
    };
    let Some(skill) = skills
        .iter()
        .find(|s| s.name.to_lowercase() == wanted)
        .or_else(|| {
            skills
                .iter()
                .find(|s| s.name.to_lowercase().contains(&wanted))
        })
    else {
        return failed(format!("No skill is called \"{name}\"."));
    };
    let (body, _) = cut(&skill.body, max_chars);
    ToolOutcome {
        ok: true,
        for_model: format!(
            "Skill \"{}\":\n\n{body}\n\nFollow these instructions for the user's request.",
            skill.name
        ),
        summary: format!("/{}", skill.name),
        for_user: cut(&skill.body, 4000).0,
        citations: Vec::new(),
    }
}

fn workspace(
    state: &AppState,
    jail: &crate::services::tools::sandbox::WorkspaceJail,
    action_type: ToolActionType,
    path: &str,
    content: Option<String>,
    query: Option<String>,
    max_chars: usize,
) -> ToolOutcome {
    let req = ToolExecutionRequest {
        tool_name: "chat".to_string(),
        action: ToolAction {
            action_type: action_type.clone(),
            path: path.trim().to_string(),
            content,
            query,
        },
        // Reaching this point means the user approved the write in chat, or
        // the action only reads.
        approved: Some(true),
        session_token: None,
    };
    match state.tools.execute_in(jail, req) {
        Ok(res) if res.success => {
            let (text, truncated) = cut(&res.output, max_chars);
            let summary = match action_type {
                ToolActionType::ReadFile => {
                    format!("Read {} characters", fmt_count(res.output.chars().count()))
                }
                ToolActionType::WriteFile => "Written".to_string(),
                _ => res
                    .output
                    .lines()
                    .next()
                    .unwrap_or("Done")
                    .trim()
                    .to_string(),
            };
            ToolOutcome {
                ok: true,
                for_model: format!(
                    "{text}{}",
                    if truncated || res.truncated {
                        "\n\n[Shortened to fit.]"
                    } else {
                        ""
                    }
                ),
                summary,
                for_user: cut(&res.output, 4000).0,
                citations: Vec::new(),
            }
        }
        Ok(res) => failed(res.error.unwrap_or_else(|| "The action failed.".into())),
        Err(e) => failed(e.message),
    }
}

fn mcp_call(state: &AppState, spec: &ToolSpec, args: &Value, max_chars: usize) -> ToolOutcome {
    let Some((server, tool)) = &spec.mcp else {
        return failed("This tool has no server.");
    };
    match state.mcp.call_tool(server, tool, args.clone()) {
        Ok(out) => {
            let (text, truncated) = cut(&out.text, max_chars);
            let chars = out.text.chars().count();
            ToolOutcome {
                ok: !out.is_error,
                for_model: format!(
                    "{text}{}",
                    if truncated {
                        "\n\n[Shortened to fit.]"
                    } else {
                        ""
                    }
                ),
                summary: if out.is_error {
                    "The tool reported an error".to_string()
                } else {
                    format!("{} characters", fmt_count(chars))
                },
                for_user: cut(&out.text, 4000).0,
                citations: Vec::new(),
            }
        }
        Err(e) => failed(e.message),
    }
}

fn fmt_count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Parse the model's arguments. Small models sometimes wrap them in a code
/// fence or send an empty string.
pub fn parse_arguments(raw: &str) -> Result<Value, AppError> {
    let trimmed = raw.trim();
    let trimmed = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|s| s.trim_end_matches("```").trim())
        .unwrap_or(trimmed);
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v @ Value::Object(_)) => Ok(v),
        Ok(Value::String(inner)) => parse_arguments(&inner),
        Ok(_) => Err(AppError::invalid_input(
            "The tool arguments must be a JSON object.",
        )),
        Err(e) => Err(AppError::invalid_input(format!(
            "The tool arguments were not valid JSON: {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_pick_the_tools() {
        let none = builtin_specs(&Permissions::default());
        assert!(none.is_empty());
        let web = builtin_specs(&Permissions {
            web: true,
            ..Default::default()
        });
        assert_eq!(
            web.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["web_search", "read_web_page"]
        );
        let all = builtin_specs(&Permissions {
            web: true,
            notes: true,
            files: true,
            skills: true,
            shell: true,
            ask: true,
            todos: true,
            subagents: true,
            learn_skills: true,
            mcp: vec![],
        });
        for name in ["update_todos", "run_subagent", "save_skill", "ask_user"] {
            assert!(all.iter().any(|s| s.name == name), "{name} is offered");
        }
        assert!(
            all.iter()
                .all(|s| s.name != "save_skill" || !s.needs_approval()),
            "learned skills save at once, with Undo"
        );
        let cmd = all.iter().find(|s| s.name == "execute_command").unwrap();
        assert!(!cmd.needs_approval_for(&serde_json::json!({"command": "git status"})));
        assert!(cmd.needs_approval_for(&serde_json::json!({"command": "npm install"})));
        assert!(
            cmd.refusal(&serde_json::json!({"command": "rm -rf /"}))
                .is_some()
        );
        assert!(cmd.refusal(&serde_json::json!({"command": "ls"})).is_none());
        assert!(
            all.iter()
                .any(|s| s.name == "write_file" && s.needs_approval())
        );
        assert!(
            all.iter()
                .any(|s| s.name == "read_file" && !s.needs_approval())
        );
        for spec in &all {
            assert_eq!(spec.schema()["function"]["parameters"]["type"], "object");
        }
    }

    #[test]
    fn mcp_names_are_safe_for_every_provider() {
        assert_eq!(
            tool_name_for("Context 7", "resolve-library-id"),
            "mcp_context_7_resolve-library-id"
        );
        let long = tool_name_for(&"x".repeat(80), &"y".repeat(80));
        assert!(long.len() <= 64);
        assert!(
            long.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        );
    }

    #[test]
    fn arguments_from_small_models_are_forgiven() {
        assert_eq!(parse_arguments("").unwrap(), json!({}));
        assert_eq!(parse_arguments("```json\n{\"a\":1}\n```").unwrap()["a"], 1);
        assert_eq!(
            parse_arguments("\"{\\\"q\\\":\\\"x\\\"}\"").unwrap()["q"],
            "x"
        );
        assert!(parse_arguments("[1]").is_err());
        assert!(parse_arguments("{nope").is_err());
        assert_eq!(fmt_count(1234567), "1,234,567");
        assert_eq!(fmt_count(12), "12");
    }
}
