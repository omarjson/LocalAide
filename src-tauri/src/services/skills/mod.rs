//! Prompt skills: user-saved prompt templates as markdown files with a
//! small frontmatter header, stored under `~/.aiassistant/skills/`. Skills
//! are plain files — no registry, no runtime — and every mutation is local.
//! Usage counters (for honest, heuristic "save this as a skill?" hints)
//! live in a sibling `usage.json`.

use crate::app::error::AppError;
use crate::app::paths::skills_dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Maximum body size for one skill file, so a runaway paste cannot bloat
/// the folder (mirrors the 1 MB read cap in the workspace jail).
const MAX_SKILL_BYTES: u64 = 256 * 1024;
const MAX_SKILLS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    /// Short identifier shown in the slash menu (from frontmatter `name`).
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The prompt body, below the frontmatter.
    pub body: String,
    /// Absolute path of the backing markdown file.
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSuggestion {
    pub prompt: String,
    pub use_count: u32,
}

pub struct SkillsService {
    dir: PathBuf,
    /// Prompt-fragment usage counts, persisted next to the skill files.
    usage: Mutex<BTreeMap<String, u32>>,
}

impl SkillsService {
    pub fn new() -> Self {
        Self::with_dir(skills_dir())
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        let service = Self {
            dir,
            usage: Mutex::new(BTreeMap::new()),
        };
        service.load_usage();
        service
    }

    fn load_usage(&self) {
        let path = self.usage_path();
        if let Ok(data) = std::fs::read_to_string(&path)
            && let Ok(map) = serde_json::from_str::<BTreeMap<String, u32>>(&data)
        {
            *self.usage.lock().expect("usage lock poisoned") = map;
        }
    }

    fn save_usage(&self) -> Result<(), AppError> {
        let path = self.usage_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::storage(format!("Failed to create skills folder: {e}")))?;
        }
        let data = serde_json::to_string_pretty(&*self.usage.lock().expect("usage lock poisoned"))
            .map_err(|e| AppError::storage(format!("Could not serialize skill usage: {e}")))?;
        std::fs::write(&path, data)
            .map_err(|e| AppError::storage(format!("Could not save skill usage: {e}")))?;
        Ok(())
    }

    fn usage_path(&self) -> PathBuf {
        self.dir.join("usage.json")
    }

    /// Record that a custom system prompt was used once, and return its
    /// current count. Purely local statistics for the suggestion heuristic.
    pub fn record_prompt_usage(&self, prompt: &str) -> u32 {
        let key = prompt.trim().to_string();
        if key.is_empty() {
            return 0;
        }
        let mut map = self.usage.lock().expect("usage lock poisoned");
        let count = map.entry(key).or_insert(0);
        *count = count.saturating_add(1);
        let value = *count;
        drop(map);
        let _ = self.save_usage();
        value
    }

    /// Prompts used repeatedly but not yet saved as a skill body — the
    /// honest "you keep typing this, want to save it?" heuristic.
    pub fn suggestions(&self, min_count: u32) -> Result<Vec<SkillSuggestion>, AppError> {
        let saved_bodies: Vec<String> = self.list()?.into_iter().map(|s| s.body).collect();
        let map = self.usage.lock().expect("usage lock poisoned");
        let mut out: Vec<SkillSuggestion> = map
            .iter()
            .filter(|(prompt, count)| {
                **count >= min_count && !saved_bodies.iter().any(|b| b.trim() == **prompt)
            })
            .map(|(prompt, count)| SkillSuggestion {
                prompt: prompt.clone(),
                use_count: *count,
            })
            .collect();
        out.sort_by(|a, b| b.use_count.cmp(&a.use_count));
        Ok(out)
    }

    pub fn list(&self) -> Result<Vec<Skill>, AppError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::storage(format!("Failed to create skills folder: {e}")))?;
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| AppError::storage(format!("Could not read skills folder: {e}")))?;

        let mut skills = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Ok(skill) = Self::read_skill_file(&path) {
                skills.push(skill);
            }
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    pub fn create(
        &self,
        name: &str,
        description: &str,
        tags: Vec<String>,
        body: &str,
    ) -> Result<Skill, AppError> {
        let name = name.trim();
        if name.is_empty() || body.trim().is_empty() {
            return Err(AppError::invalid_input(
                "A skill needs a name and a prompt body",
            ));
        }
        let existing = self.list()?;
        if existing.len() >= MAX_SKILLS {
            return Err(AppError::invalid_input(format!(
                "The skills folder holds {MAX_SKILLS} skills already; delete one first"
            )));
        }
        if existing.iter().any(|s| s.name.eq_ignore_ascii_case(name)) {
            return Err(AppError::invalid_input(format!(
                "A skill named '{name}' already exists"
            )));
        }

        let path = self.dir.join(format!("{}.md", slugify(name)));
        let skill = Skill {
            name: name.to_string(),
            description: description.trim().to_string(),
            tags,
            body: body.to_string(),
            path: path.display().to_string(),
        };
        std::fs::write(&path, render_skill_file(&skill))
            .map_err(|e| AppError::storage(format!("Could not write skill file: {e}")))?;
        Ok(skill)
    }

    pub fn update(
        &self,
        path: &str,
        name: &str,
        description: &str,
        tags: Vec<String>,
        body: &str,
    ) -> Result<Skill, AppError> {
        let path = self.guard_path(path)?;
        let skill = Skill {
            name: name.trim().to_string(),
            description: description.trim().to_string(),
            tags,
            body: body.to_string(),
            path: path.display().to_string(),
        };
        if skill.name.is_empty() || skill.body.trim().is_empty() {
            return Err(AppError::invalid_input(
                "A skill needs a name and a prompt body",
            ));
        }
        std::fs::write(&path, render_skill_file(&skill))
            .map_err(|e| AppError::storage(format!("Could not write skill file: {e}")))?;
        Ok(skill)
    }

    pub fn delete(&self, path: &str) -> Result<(), AppError> {
        let path = self.guard_path(path)?;
        std::fs::remove_file(path)
            .map_err(|e| AppError::storage(format!("Could not delete skill file: {e}")))?;
        Ok(())
    }

    /// Read one skill's raw file (for the editor).
    pub fn read(&self, path: &str) -> Result<Skill, AppError> {
        let path = self.guard_path(path)?;
        Self::read_skill_file(&path)
    }

    /// Only files inside the skills folder are addressable — no traversal.
    fn guard_path(&self, path: &str) -> Result<PathBuf, AppError> {
        let requested = Path::new(path);
        let canonical_dir = std::fs::canonicalize(&self.dir)
            .map_err(|e| AppError::storage(format!("Skills folder is unavailable: {e}")))?;
        let canonical = if requested.is_absolute() {
            std::fs::canonicalize(requested)
        } else {
            std::fs::canonicalize(self.dir.join(requested))
        }
        .map_err(|_| AppError::invalid_input("Skill file does not exist"))?;
        if !canonical.starts_with(&canonical_dir) {
            return Err(AppError::invalid_input(
                "Skill paths must stay inside the skills folder",
            ));
        }
        Ok(canonical)
    }

    fn read_skill_file(path: &Path) -> Result<Skill, AppError> {
        let meta = std::fs::metadata(path)
            .map_err(|e| AppError::storage(format!("Could not stat skill file: {e}")))?;
        if meta.len() > MAX_SKILL_BYTES {
            return Err(AppError::invalid_input("Skill file exceeds the size limit"));
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|e| AppError::storage(format!("Could not read skill file: {e}")))?;
        let (front, body) = split_frontmatter(&raw);
        let file_name = path.file_stem().map(|s| s.to_string_lossy().to_string());
        let name = front
            .get("name")
            .cloned()
            .unwrap_or_else(|| file_name.unwrap_or_default());
        let description = front.get("description").cloned().unwrap_or_default();
        let tags = front
            .get("tags")
            .map(|t| {
                t.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Ok(Skill {
            name,
            description,
            tags,
            body: body.trim().to_string(),
            path: path.display().to_string(),
        })
    }
}

impl Default for SkillsService {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a leading `---` frontmatter block into key/value pairs. Values are
/// single lines; unknown keys are ignored rather than rejected.
fn split_frontmatter(raw: &str) -> (BTreeMap<String, String>, String) {
    let mut front = BTreeMap::new();
    let rest = raw.trim_start();
    if let Some(after) = rest.strip_prefix("---") {
        if let Some(end) = after.find("\n---") {
            let block = &after[..end];
            for line in block.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    front.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
            let body_start = end + 4; // past "\n---"
            let body = after.get(body_start..).unwrap_or("");
            return (front, body.trim_start_matches(['\n', '\r']).to_string());
        }
    }
    (front, raw.to_string())
}

/// Serialize a skill back to its markdown file format.
fn render_skill_file(skill: &Skill) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", skill.name));
    out.push_str(&format!("description: {}\n", skill.description));
    if !skill.tags.is_empty() {
        out.push_str(&format!("tags: {}\n", skill.tags.join(", ")));
    }
    out.push_str("---\n\n");
    out.push_str(skill.body.trim());
    out.push('\n');
    out
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
    fn frontmatter_and_body_split() {
        let raw =
            "---\nname: review\ndescription: Review code\ntags: code, quality\n---\n\nBody here.";
        let (front, body) = split_frontmatter(raw);
        assert_eq!(front.get("name").map(String::as_str), Some("review"));
        assert_eq!(front.get("tags").map(String::as_str), Some("code, quality"));
        assert_eq!(body.trim(), "Body here.");
    }

    #[test]
    fn no_frontmatter_returns_raw_body() {
        let (front, body) = split_frontmatter("Just a prompt.");
        assert!(front.is_empty());
        assert_eq!(body, "Just a prompt.");
    }

    #[test]
    fn slug_is_filename_safe() {
        assert_eq!(slugify("Code Review!"), "code-review");
        assert_eq!(slugify("  --weird__name--  "), "weird-name");
    }
}
