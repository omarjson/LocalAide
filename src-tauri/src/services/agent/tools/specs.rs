//! The tool catalogue: which built-in and MCP tools a chat offers the
//! model, their JSON schemas, and the one-line title shown for a call.
//! Moved out of `tools.rs` unchanged in 0.2.4; how each tool runs stays in
//! `mod.rs`.

use serde_json::{Value, json};

use super::{Permissions, ToolKind, ToolSpec};
use crate::app::state::AppState;
use crate::config::McpServerConfig;
use crate::services::tools::shell;

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
        specs.push(builtin(
            "suggest_note",
            ToolKind::SuggestNote,
            "Offer to remember a fact for later chats: the suggestion lands in a review queue on the Memory screen, where the user accepts or dismisses it. Use it freely when you learn something durable about the user, their projects or their preferences; use save_note instead only when the user asks you to remember something right now.",
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
            "List files in the folder this chat works in. Paths are relative to it.",
            object(
                json!({"path": {"type": "string", "description": "A folder inside the workspace. Empty for the top."}}),
                &[],
            ),
        ));
        specs.push(builtin(
            "read_file",
            ToolKind::ReadFile,
            "Read a text file from the folder this chat works in.",
            object(json!({"path": {"type": "string"}}), &["path"]),
        ));
        specs.push(builtin(
            "search_files",
            ToolKind::SearchFiles,
            "Find files in the chat's folder whose name contains the words.",
            object(
                json!({"query": {"type": "string"}, "path": {"type": "string"}}),
                &["query"],
            ),
        ));
        specs.push(builtin(
            "write_file",
            ToolKind::WriteFile,
            "Write a text file in the folder this chat works in. Replaces the file if it exists. Depending on the user's permission mode, the user approves the write first.",
            object(
                json!({"path": {"type": "string"}, "content": {"type": "string"}}),
                &["path", "content"],
            ),
        ));
        specs.push(builtin(
            "make_document",
            ToolKind::MakeDocument,
            "Make a real Word, Excel or PowerPoint file in the folder this chat works in. Pick a kind: docx for reports (headings, paragraphs, bullets, tables), xlsx for spreadsheets (sheets of rows), pptx for a slide deck (slides with titles and bullets). The user approves the write first.",
            object(
                json!({
                    "path": {"type": "string", "description": "File name ending in .docx, .xlsx or .pptx."},
                    "kind": {"type": "string", "enum": ["docx", "xlsx", "pptx"], "description": "Which Office format to write. Defaults from the path ending when it is clear."},
                    "title": {"type": "string", "description": "The document or deck title, shown at the top."},
                    "blocks": {"type": "array", "description": "Word content, in order: {kind: heading|paragraph|bullet|table, text, items, rows, level}.", "items": {"type": "object"}},
                    "sheets": {"type": "array", "description": "Excel sheets: {name, rows: [[cell, ...]]}.", "items": {"type": "object"}},
                    "slides": {"type": "array", "description": "Deck slides: {title, bullets: []}.", "items": {"type": "object"}}
                }),
                &["path"],
            ),
        ));
    }
    if perms.shell {
        specs.push(builtin(
            "execute_command",
            ToolKind::RunCommand,
            &format!(
                "Run a terminal command on the user's computer with {}, in the folder this chat works in. Use it when the user asks you to run, build, check or inspect something with a command. Read-only commands (ls, cat, git status and similar) run at once; commands that change something wait for the user's OK; destructive commands (recursive deletes, disk formatting, piping downloads into a shell, administrator rights) are refused. The result has the exit code and the output, which may be shortened.",
                shell::shell_name()
            ),
            object(
                json!({
                    "command": {"type": "string", "description": "The command line to run."},
                    "cwd": {"type": "string", "description": "A folder inside the workspace to run in. Empty for the top."},
                    "timeout_seconds": {"type": "integer", "description": "Time limit, 1 to 300. Default 60."}
                }),
                &["command"],
            ),
        ));
    }
    if perms.ask {
        specs.push(builtin(
            "ask_user",
            ToolKind::AskUser,
            "Ask the user one or more short questions with answer options they can pick, when you need their choice before you can continue. Give 2 to 6 options per question, each with a short label and an optional one-line description. Mark at most one option per question \"recommended\": true when you have a clear pick — it is shown first and marked Recommended. The user can also type their own answer. Do not use it for questions you can answer yourself.",
            object(
                json!({
                    "questions": {
                        "type": "array",
                        "description": "1 to 4 questions, asked together.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "header": {"type": "string", "description": "One or two words, for example Clarify or Format."},
                                "question": {"type": "string", "description": "The question, in one sentence."},
                                "options": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": {"type": "string"},
                                            "description": {"type": "string"},
                                            "recommended": {"type": "boolean", "description": "True for your pick when you have one; at most one per question. Shown first, marked Recommended."}
                                        },
                                        "required": ["label"]
                                    },
                                    "description": "2 to 6 answers."
                                },
                                "multiple": {"type": "boolean", "description": "True when the user may pick more than one."}
                            },
                            "required": ["question", "options"]
                        }
                    },
                    "question": {"type": "string", "description": "A single question (older form; use questions)."},
                    "options": {"type": "array", "items": {"type": "string"}, "description": "Its answers (older form)."},
                    "multiple": {"type": "boolean"}
                }),
                &[],
            ),
        ));
    }
    if perms.todos {
        specs.push(builtin(
            "update_todos",
            ToolKind::Todos,
            "Keep a short to-do list for a task with several steps, so the user sees progress. Send the whole list each time, with each step's status. Use it only for tasks of 3 or more steps.",
            object(
                json!({
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "done"]}
                            },
                            "required": ["text", "status"]
                        }
                    }
                }),
                &["items"],
            ),
        ));
    }
    if perms.subagents {
        specs.push(builtin(
            "run_subagent",
            ToolKind::Subagent,
            "Give one self-contained part of a bigger task to a subagent: a helper with its own fresh context that can search and read (web, notes, files, read-only commands) but cannot change anything. Several calls in one turn run at the same time. Use it to research or inspect several things in parallel; it returns its findings.",
            object(
                json!({
                    "title": {"type": "string", "description": "A few words naming the job, for example Check the tests."},
                    "task": {"type": "string", "description": "Everything the subagent needs to know: the goal, where to look, and what to report back."}
                }),
                &["task"],
            ),
        ));
    }
    if perms.learn_skills {
        specs.push(builtin(
            "save_skill",
            ToolKind::SaveSkill,
            "Save a reusable skill, or improve the user's skill with the same name: instructions for a kind of task, learned from this chat. Use it when the user teaches you how they want a recurring task done, or corrects how you did a task a skill covers. Not for one-off facts (those are notes). It saves at once; the user can undo it. Tell the user in one short sentence.",
            object(
                json!({
                    "name": {"type": "string", "description": "A short name, for example Weekly report."},
                    "description": {"type": "string", "description": "One line: when to use it."},
                    "instructions": {"type": "string", "description": "The full, improved instructions in markdown. For an existing skill, keep what still applies and add what you learned."}
                }),
                &["name", "instructions"],
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
        ToolKind::SuggestNote => "Suggested a note".to_string(),
        ToolKind::ListSkills => "Looked at your skills".to_string(),
        ToolKind::UseSkill => "Used a skill".to_string(),
        ToolKind::ListFiles => "Listed workspace files".to_string(),
        ToolKind::ReadFile => "Read a file".to_string(),
        ToolKind::SearchFiles => "Searched workspace files".to_string(),
        ToolKind::WriteFile => "Write a file".to_string(),
        ToolKind::MakeDocument => "Make a document".to_string(),
        ToolKind::RunCommand => "Run a command".to_string(),
        ToolKind::AskUser => "Asked you".to_string(),
        ToolKind::Todos => "Updated the plan".to_string(),
        ToolKind::Subagent => "Subagent".to_string(),
        ToolKind::SaveSkill => "Learned a skill".to_string(),
        ToolKind::Mcp => match &spec.mcp {
            Some((server, tool)) => format!("{} · {}", server.name, tool),
            None => spec.name.clone(),
        },
    };
    let detail = match spec.kind {
        ToolKind::WebSearch | ToolKind::SearchNotes | ToolKind::SearchFiles => text("query"),
        ToolKind::ReadPage => text("url"),
        ToolKind::SaveNote | ToolKind::SuggestNote => text("title"),
        ToolKind::UseSkill => text("name"),
        ToolKind::ListFiles | ToolKind::ReadFile | ToolKind::WriteFile | ToolKind::MakeDocument => {
            text("path")
        }
        ToolKind::ListSkills => String::new(),
        ToolKind::RunCommand => text("command"),
        ToolKind::AskUser => text("question"),
        ToolKind::Todos => String::new(),
        ToolKind::Subagent => {
            let title = text("title");
            if title.is_empty() {
                text("task")
            } else {
                title
            }
        }
        ToolKind::SaveSkill => text("name"),
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
