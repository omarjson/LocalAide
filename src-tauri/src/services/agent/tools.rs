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

/// Which groups of tools this chat may use.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub web: bool,
    pub notes: bool,
    pub files: bool,
    pub skills: bool,
    /// MCP servers switched on for this chat.
    pub mcp: Vec<McpServerConfig>,
}

impl Permissions {
    pub fn any(&self) -> bool {
        self.web || self.notes || self.files || self.skills || !self.mcp.is_empty()
    }
}

/// What a tool is, for the picker of icons and the approval rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    WebSearch,
    ReadPage,
    SearchNotes,
    SaveNote,
    ListSkills,
    UseSkill,
    ListFiles,
    ReadFile,
    SearchFiles,
    WriteFile,
    Mcp,
}

impl ToolKind {
    /// The icon group the chat shows for this tool.
    pub fn group(self) -> &'static str {
        match self {
            Self::WebSearch | Self::ReadPage => "web",
            Self::SearchNotes | Self::SaveNote => "notes",
            Self::ListSkills | Self::UseSkill => "skills",
            Self::ListFiles | Self::ReadFile | Self::SearchFiles | Self::WriteFile => "files",
            Self::Mcp => "mcp",
        }
    }

    /// Tools that change something wait for the user.
    pub fn changes_something(self) -> bool {
        matches!(self, Self::SaveNote | Self::WriteFile | Self::Mcp)
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
}

fn builtin(name: &str, kind: ToolKind, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
        kind,
        mcp: None,
    }
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

/// The built-in tools this chat may use.
pub fn builtin_specs(perms: &Permissions) -> Vec<ToolSpec> {
    let mut specs = Vec::new();
    if perms.web {
        specs.push(builtin(
            "web_search",
            ToolKind::WebSearch,
            "Search the web. Use it for recent events, prices, versions, or facts you are not sure about. Do not use it for things you already know well. Results come back numbered; cite them as [n].",
            object(
                json!({
                    "query": {"type": "string", "description": "What to search for, in a few words."},
                    "max_results": {"type": "integer", "description": "How many results, 1 to 8. Default 5."}
                }),
                &["query"],
            ),
        ));
        specs.push(builtin(
            "read_web_page",
            ToolKind::ReadPage,
            "Read a web page as text, for example a search result you need in full.",
            object(
                json!({"url": {"type": "string", "description": "The full http or https address."}}),
                &["url"],
            ),
        ));
    }
    if perms.notes {
        specs.push(builtin(
            "search_notes",
            ToolKind::SearchNotes,
            "Search the user's own memory notes and document library on this computer. Use it when the question may be about the user's own information, files or earlier notes.",
            object(
                json!({"query": {"type": "string", "description": "Words to look for."}}),
                &["query"],
            ),
        ));
        specs.push(builtin(
            "save_note",
            ToolKind::SaveNote,
            "Save a memory note for later chats, when the user asks you to remember something. The user approves each note.",
            object(
                json!({
                    "title": {"type": "string", "description": "A short title."},
                    "content": {"type": "string", "description": "The note, in markdown."}
                }),
                &["title", "content"],
            ),
        ));
    }
    if perms.skills {
        specs.push(builtin(
            "list_skills",
            ToolKind::ListSkills,
            "List the user's saved skills: named instructions for tasks such as writing in a style or following a checklist.",
            object(json!({}), &[]),
        ));
        specs.push(builtin(
            "use_skill",
            ToolKind::UseSkill,
            "Get the full instructions of one saved skill by name, then follow them.",
            object(json!({"name": {"type": "string"}}), &["name"]),
        ));
    }
    if perms.files {
        specs.push(builtin(
            "list_files",
            ToolKind::ListFiles,
            "List files in the LocalAide workspace folder. Paths are relative to it.",
            object(
                json!({"path": {"type": "string", "description": "A folder inside the workspace. Empty for the top."}}),
                &[],
            ),
        ));
        specs.push(builtin(
            "read_file",
            ToolKind::ReadFile,
            "Read a text file from the LocalAide workspace folder.",
            object(json!({"path": {"type": "string"}}), &["path"]),
        ));
        specs.push(builtin(
            "search_files",
            ToolKind::SearchFiles,
            "Find files in the workspace whose name contains the words.",
            object(
                json!({"query": {"type": "string"}, "path": {"type": "string"}}),
                &["query"],
            ),
        ));
        specs.push(builtin(
            "write_file",
            ToolKind::WriteFile,
            "Write a text file in the LocalAide workspace folder. Replaces the file if it exists. The user approves each write.",
            object(
                json!({"path": {"type": "string"}, "content": {"type": "string"}}),
                &["path", "content"],
            ),
        ));
    }
    specs
}

/// A tool name the providers accept: letters, digits, `_` and `-`, up to
/// 64 characters.
pub fn tool_name_for(server: &str, tool: &str) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    };
    let server: String = clean(server)
        .to_ascii_lowercase()
        .chars()
        .take(20)
        .collect();
    let mut name = format!("mcp_{server}_{}", clean(tool));
    name.truncate(64);
    name
}

/// The MCP servers' tools as specs. A server that cannot start is skipped,
/// with a note for the chat.
pub fn mcp_specs(state: &AppState, servers: &[McpServerConfig]) -> (Vec<ToolSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut problems = Vec::new();
    for server in servers {
        match state.mcp.list_tools(server) {
            Ok(tools) => {
                for tool in tools {
                    let name = tool_name_for(&server.name, &tool.name);
                    if specs.iter().any(|s: &ToolSpec| s.name == name) {
                        continue;
                    }
                    let description = if tool.description.trim().is_empty() {
                        format!("{} (from {})", tool.name, server.name)
                    } else {
                        let short: String = tool.description.chars().take(1000).collect();
                        format!("{short} (from {})", server.name)
                    };
                    specs.push(ToolSpec {
                        name,
                        description,
                        parameters: tool.input_schema,
                        kind: ToolKind::Mcp,
                        mcp: Some((server.clone(), tool.name)),
                    });
                }
            }
            Err(e) => problems.push(format!("{} is not available: {}", server.name, e.message)),
        }
    }
    (specs, problems)
}

/// A short title and detail for the chat, from the tool and its arguments.
pub fn describe(spec: &ToolSpec, args: &Value) -> (String, String) {
    let text = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .chars()
            .take(120)
            .collect::<String>()
    };
    let title = match spec.kind {
        ToolKind::WebSearch => "Web search".to_string(),
        ToolKind::ReadPage => "Read a page".to_string(),
        ToolKind::SearchNotes => "Searched your notes".to_string(),
        ToolKind::SaveNote => "Save a note".to_string(),
        ToolKind::ListSkills => "Looked at your skills".to_string(),
        ToolKind::UseSkill => "Used a skill".to_string(),
        ToolKind::ListFiles => "Listed workspace files".to_string(),
        ToolKind::ReadFile => "Read a file".to_string(),
        ToolKind::SearchFiles => "Searched workspace files".to_string(),
        ToolKind::WriteFile => "Write a file".to_string(),
        ToolKind::Mcp => match &spec.mcp {
            Some((server, tool)) => format!("{} · {}", server.name, tool),
            None => spec.name.clone(),
        },
    };
    let detail = match spec.kind {
        ToolKind::WebSearch | ToolKind::SearchNotes | ToolKind::SearchFiles => text("query"),
        ToolKind::ReadPage => text("url"),
        ToolKind::SaveNote => text("title"),
        ToolKind::UseSkill => text("name"),
        ToolKind::ListFiles | ToolKind::ReadFile | ToolKind::WriteFile => text("path"),
        ToolKind::ListSkills => String::new(),
        ToolKind::Mcp => {
            // The first short string argument says the most.
            args.as_object()
                .and_then(|m| m.values().find_map(Value::as_str))
                .map(|s| s.chars().take(120).collect())
                .unwrap_or_default()
        }
    };
    (title, detail)
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
        ToolKind::ListSkills => list_skills(state),
        ToolKind::UseSkill => use_skill(state, &text("name"), max_chars),
        ToolKind::ListFiles => workspace(
            state,
            ToolActionType::ListDir,
            &text("path"),
            None,
            None,
            max_chars,
        ),
        ToolKind::ReadFile => workspace(
            state,
            ToolActionType::ReadFile,
            &text("path"),
            None,
            None,
            max_chars,
        ),
        ToolKind::SearchFiles => workspace(
            state,
            ToolActionType::SearchFiles,
            &text("path"),
            None,
            Some(text("query")),
            max_chars,
        ),
        ToolKind::WriteFile => workspace(
            state,
            ToolActionType::WriteFile,
            &text("path"),
            Some(text("content")),
            None,
            max_chars,
        ),
        ToolKind::Mcp => mcp_call(state, spec, args, max_chars),
    };
    // Workspace actions write their own audit rows; the rest are recorded here.
    if !matches!(
        spec.kind,
        ToolKind::ListFiles | ToolKind::ReadFile | ToolKind::SearchFiles | ToolKind::WriteFile
    ) {
        let (_, detail) = describe(spec, args);
        state.tools.record_event(
            &format!("chat:{}", spec.name),
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
    let recall = match state.memory.recall(query, vector.as_deref(), Some(900)) {
        Ok(r) => r,
        Err(e) => return failed(format!("Your notes could not be searched: {}", e.message)),
    };
    if recall.citations.is_empty() {
        return ToolOutcome {
            ok: true,
            for_model: "Nothing in the user's notes or documents matches. Say so if it matters."
                .into(),
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
        lines.push(format!(
            "[{index}] {} ({})\n{}",
            cit.title,
            cit.source,
            cit.snippet.trim()
        ));
        citations.push(Citation { index, ..cit });
    }
    let body = lines.join("\n\n");
    ToolOutcome {
        ok: true,
        for_model: format!("From the user's notes and documents:\n\n{body}\n\nCite them as [n]."),
        summary: format!("{} matches", citations.len()),
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
    match state.tools.execute(req) {
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
            mcp: vec![],
        });
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
