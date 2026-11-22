//! Agent personas: markdown files with a small frontmatter header, stored
//! under `~/.aiassistant/agents/`. They are plain files — no registry, no
//! runtime — and every mutation is local to this machine.

use crate::app::error::AppError;
use crate::app::paths::app_dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Maximum body size for one agent file, so a runaway paste cannot bloat the
/// folder (well below the 1 MB workspace read cap).
pub const MAX_AGENT_BYTES: u64 = 256 * 1024;
const MAX_AGENTS: usize = 200;

/// One agent persona. The prompt is the markdown body below the frontmatter;
/// a `prompt:` frontmatter key overrides it when present.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub model: String,
    pub tools: Vec<String>,
    pub prompt: String,
    /// Absolute path of the backing markdown file.
    pub path: String,
}

/// Read-only agent store. The directory is created on the first `list`/`save`.
pub struct AgentsService {
    dir: PathBuf,
}

impl AgentsService {
    pub fn new() -> Self {
        Self::with_dir(app_dir().join("agents"))
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every agent in the folder, sorted by name. Unreadable files are
    /// skipped so one malformed file cannot hide the rest.
    pub fn list(&self) -> Result<Vec<Agent>, AppError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::storage(format!("Failed to create agents folder: {e}")))?;
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| AppError::storage(format!("Could not read agents folder: {e}")))?;
        let mut agents = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Ok(agent) = Self::read_agent_file(&path) {
                agents.push(agent);
            }
        }
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(agents)
    }

    /// Read one agent by its file path (a name or a relative file name is
    /// also accepted). Paths are confined to the agents folder.
    pub fn read(&self, path: &str) -> Result<Agent, AppError> {
        let path = self.guard_path(path)?;
        Self::read_agent_file(&path)
    }

    /// Add or update an agent. An agent with the same name is overwritten in
    /// place; otherwise a new `slug.md` file is created.
    pub fn save(
        &self,
        name: &str,
        model: &str,
        tools: Vec<String>,
        prompt: &str,
    ) -> Result<Agent, AppError> {
        let name = one_line(name);
        if name.is_empty() || prompt.trim().is_empty() {
            return Err(AppError::invalid_input(
                "An agent needs a name and a prompt.",
            ));
        }
        if prompt.len() as u64 > MAX_AGENT_BYTES {
            return Err(AppError::invalid_input(
                "This agent prompt is too long to save (over 256 KB).",
            ));
        }
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::storage(format!("Failed to create agents folder: {e}")))?;

        let existing = self
            .list()?
            .into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(&name));
        let path = match &existing {
            Some(agent) => PathBuf::from(&agent.path),
            None => {
                let count = self.list()?.len();
                if count >= MAX_AGENTS {
                    return Err(AppError::invalid_input(format!(
                        "The agents folder holds {MAX_AGENTS} agents already; remove one first"
                    )));
                }
                self.free_path(&name)
            }
        };
        // A hand-edited file may have been removed between list and write.
        let path = if path.exists() || existing.is_some() {
            path
        } else {
            self.free_path(&name)
        };
        let agent = Agent {
            name,
            model: one_line(model),
            tools: tools
                .into_iter()
                .map(|t| one_line(&t))
                .filter(|t| !t.is_empty())
                .collect(),
            prompt: prompt.to_string(),
            path: path.display().to_string(),
        };
        std::fs::write(&path, render_agent(&agent))
            .map_err(|e| AppError::storage(format!("Could not write agent file: {e}")))?;
        Ok(agent)
    }

    /// A file name for a new agent that does not overwrite another one.
    fn free_path(&self, name: &str) -> PathBuf {
        let mut slug = slugify(name);
        if slug.is_empty() {
            slug = "agent".to_string();
        }
        let mut candidate = self.dir.join(format!("{slug}.md"));
        let mut n = 2;
        while candidate.exists() {
            candidate = self.dir.join(format!("{slug}-{n}.md"));
            n += 1;
        }
        candidate
    }

    /// Only files inside the agents folder are addressable — no traversal.
    fn guard_path(&self, path: &str) -> Result<PathBuf, AppError> {
        let requested = Path::new(path);
        let canonical_dir = std::fs::canonicalize(&self.dir)
            .map_err(|e| AppError::storage(format!("Agents folder is unavailable: {e}")))?;
        let candidate = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.dir.join(requested)
        };
        // A bare name is convenient for callers: try it as a file name first.
        let candidate = if !candidate.exists() && requested.extension().is_none() {
            self.dir.join(format!("{path}.md"))
        } else {
            candidate
        };
        let canonical = std::fs::canonicalize(&candidate)
            .map_err(|_| AppError::invalid_input("Agent file does not exist"))?;
        if !canonical.starts_with(&canonical_dir) {
            return Err(AppError::invalid_input(
                "Agent paths must stay inside the agents folder",
            ));
        }
        Ok(canonical)
    }

    fn read_agent_file(path: &Path) -> Result<Agent, AppError> {
        let meta = std::fs::metadata(path)
            .map_err(|e| AppError::storage(format!("Could not stat agent file: {e}")))?;
        if meta.len() > MAX_AGENT_BYTES {
            return Err(AppError::invalid_input("Agent file exceeds the size limit"));
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|e| AppError::storage(format!("Could not read agent file: {e}")))?;
        Ok(parse_agent_at(&raw, path.display().to_string()))
    }
}

impl Default for AgentsService {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a leading `---` frontmatter block into key/value pairs and the body.
/// Values are single lines; unknown keys are ignored rather than rejected.
pub fn split_frontmatter(raw: &str) -> (BTreeMap<String, String>, String) {
    let mut front = BTreeMap::new();
    let rest = raw.trim_start();
    if let Some(after) = rest.strip_prefix("---")
        && let Some(end) = after.find("\n---")
    {
        let block = &after[..end];
        for line in block.lines() {
            if let Some((key, value)) = line.split_once(':') {
                front.insert(
                    one_line(key),
                    value.trim().trim_matches(['"', '\'']).trim().to_string(),
                );
            }
        }
        let body_start = end + 4; // past "\n---"
        let body = after.get(body_start..).unwrap_or("");
        return (front, body.trim_start_matches(['\n', '\r']).to_string());
    }
    (front, raw.to_string())
}

/// Parse an agent from markdown. Pure, so it can be unit tested directly and
/// used by callers that hold the file text themselves.
pub fn parse_agent(raw: &str) -> Agent {
    parse_agent_at(raw, String::new())
}

fn parse_agent_at(raw: &str, path: String) -> Agent {
    let (front, body) = split_frontmatter(raw);
    let fallback_name = Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = front
        .get("name")
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or(fallback_name);
    let model = front.get("model").cloned().unwrap_or_default();
    let tools = front
        .get("tools")
        .map(|value| parse_tools(value))
        .unwrap_or_default();
    let prompt = front
        .get("prompt")
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| body.trim().to_string());
    Agent {
        name,
        model,
        tools,
        prompt,
        path,
    }
}

/// `tools: read, write` and `tools: [read, write]` are both accepted.
fn parse_tools(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|t| one_line(t.trim_matches(['"', '\''])))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Serialize an agent back to its markdown file format.
pub fn render_agent(agent: &Agent) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", one_line(&agent.name)));
    if !agent.model.trim().is_empty() {
        out.push_str(&format!("model: {}\n", one_line(&agent.model)));
    }
    if !agent.tools.is_empty() {
        let tools: Vec<String> = agent.tools.iter().map(|t| one_line(t)).collect();
        out.push_str(&format!("tools: {}\n", tools.join(", ")));
    }
    out.push_str("---\n\n");
    out.push_str(agent.prompt.trim());
    out.push('\n');
    out
}

/// Frontmatter values are single lines. Collapse whitespace so an imported
/// value cannot end the header early.
fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// File-name-safe slug: lowercase, alphanumerics and dashes only.
fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_parses_name_model_tools_and_prompt() {
        let raw = "---\nname: Code Reviewer\nmodel: gpt-4o-mini\ntools: [read_file, write_file]\n---\n\nReview the diff carefully.";
        let agent = parse_agent(raw);
        assert_eq!(agent.name, "Code Reviewer");
        assert_eq!(agent.model, "gpt-4o-mini");
        assert_eq!(agent.tools, vec!["read_file", "write_file"]);
        assert_eq!(agent.prompt, "Review the diff carefully.");
    }

    #[test]
    fn prompt_key_overrides_body_and_unknown_keys_are_ignored() {
        let raw = "---\nname: Tester\nprompt: Do the thing\nx-extra: ignored\n---\n\nBody text.";
        let agent = parse_agent(raw);
        assert_eq!(agent.prompt, "Do the thing");
        assert!(agent.model.is_empty());
        assert!(agent.tools.is_empty());
    }

    #[test]
    fn no_frontmatter_uses_the_whole_text_as_prompt() {
        let agent = parse_agent("Just a prompt.");
        assert!(agent.name.is_empty());
        assert_eq!(agent.prompt, "Just a prompt.");
    }
}
