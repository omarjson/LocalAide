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

pub mod discover;

/// Maximum body size for one skill file, so a runaway paste cannot bloat
/// the folder (mirrors the 1 MB read cap in the workspace jail).
pub const MAX_SKILL_BYTES: u64 = 256 * 1024;
const MAX_SKILLS: usize = 200;

/// A skill the model saved or improved, and the id that undoes it.
#[derive(Debug, Clone)]
pub struct Learned {
    pub skill: Skill,
    pub change_id: String,
    pub created: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SkillUndo {
    path: String,
    name: String,
    /// The whole file before the change; None when the change created it.
    previous: Option<String>,
}

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
    /// Where an imported skill came from (a web address). None for skills
    /// the user wrote.
    #[serde(default)]
    pub source: Option<String>,
    /// The license line of an imported skill, when its source had one.
    #[serde(default)]
    pub license: Option<String>,
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
            *self.usage.lock().unwrap_or_else(|e| e.into_inner()) = map;
        }
    }

    fn save_usage(&self) -> Result<(), AppError> {
        let path = self.usage_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::storage(format!("Failed to create skills folder: {e}")))?;
        }
        let data =
            serde_json::to_string_pretty(&*self.usage.lock().unwrap_or_else(|e| e.into_inner()))
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
        let mut map = self.usage.lock().unwrap_or_else(|e| e.into_inner());
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
        let map = self.usage.lock().unwrap_or_else(|e| e.into_inner());
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
        out.sort_by_key(|item| std::cmp::Reverse(item.use_count));
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
        self.create_with_origin(name, description, tags, body, None, None)
    }

    /// Create a skill that remembers where it came from (imports).
    pub fn create_with_origin(
        &self,
        name: &str,
        description: &str,
        tags: Vec<String>,
        body: &str,
        source: Option<String>,
        license: Option<String>,
    ) -> Result<Skill, AppError> {
        let name = one_line(name);
        let name = name.as_str();
        if body.len() as u64 > MAX_SKILL_BYTES {
            return Err(AppError::invalid_input(
                "This skill is too long to save (over 256 KB).",
            ));
        }
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

        std::fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::storage(format!("Failed to create skills folder: {e}")))?;
        let path = self.free_path(name);
        let skill = Skill {
            name: name.to_string(),
            description: one_line(description),
            tags,
            body: body.to_string(),
            path: path.display().to_string(),
            source: source.map(|s| one_line(&s)).filter(|s| !s.is_empty()),
            license: license.map(|s| one_line(&s)).filter(|s| !s.is_empty()),
        };
        std::fs::write(&path, render_skill_file(&skill))
            .map_err(|e| AppError::storage(format!("Could not write skill file: {e}")))?;
        Ok(skill)
    }

    /// A file name for a new skill that does not overwrite another one:
    /// "code-review.md", then "code-review-2.md", and so on.
    fn free_path(&self, name: &str) -> PathBuf {
        let mut slug = slugify(name);
        if slug.is_empty() {
            slug = "skill".to_string();
        }
        let mut candidate = self.dir.join(format!("{slug}.md"));
        let mut n = 2;
        while candidate.exists() {
            candidate = self.dir.join(format!("{slug}-{n}.md"));
            n += 1;
        }
        candidate
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
        // Editing keeps where an imported skill came from.
        let previous = Self::read_skill_file(&path).ok();
        let skill = Skill {
            name: one_line(name),
            description: one_line(description),
            tags,
            body: body.to_string(),
            path: path.display().to_string(),
            source: previous.as_ref().and_then(|p| p.source.clone()),
            license: previous.and_then(|p| p.license),
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

    /// Save a skill the model learned from a chat: a new one, or a better
    /// version of the skill with the same name. What it replaced is kept in
    /// `.history/`, so the chat's Undo can put it back.
    pub fn learn(&self, name: &str, description: &str, body: &str) -> Result<Learned, AppError> {
        let name = one_line(name);
        if name.is_empty() || body.trim().is_empty() {
            return Err(AppError::invalid_input(
                "A skill needs a name and instructions.",
            ));
        }
        let existing = self
            .list()?
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(&name));
        let (skill, previous) = match existing {
            Some(old) => {
                let previous = std::fs::read_to_string(&old.path).ok();
                let description = if description.trim().is_empty() {
                    old.description.clone()
                } else {
                    description.to_string()
                };
                let mut tags = old.tags.clone();
                if !tags.iter().any(|t| t == "learned") {
                    tags.push("learned".into());
                }
                let skill = self.update(&old.path, &old.name, &description, tags, body)?;
                (skill, previous)
            }
            None => (
                self.create(&name, description, vec!["learned".into()], body)?,
                None,
            ),
        };
        let created = previous.is_none();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let history = self.dir.join(".history");
        std::fs::create_dir_all(&history)
            .map_err(|e| AppError::storage(format!("Could not keep the skill's history: {e}")))?;
        let mut change_id = format!("{stamp}-{}", slugify(&skill.name));
        let mut n = 2;
        while history.join(format!("{change_id}.json")).exists() {
            change_id = format!("{stamp}-{}-{n}", slugify(&skill.name));
            n += 1;
        }
        let record = SkillUndo {
            path: skill.path.clone(),
            name: skill.name.clone(),
            previous,
        };
        let json = serde_json::to_string(&record).map_err(AppError::from)?;
        std::fs::write(history.join(format!("{change_id}.json")), json)
            .map_err(|e| AppError::storage(format!("Could not keep the skill's history: {e}")))?;
        // Keep the newest 100 changes.
        if let Ok(entries) = std::fs::read_dir(&history) {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            files.sort();
            let extra = files.len().saturating_sub(100);
            for old in files.into_iter().take(extra) {
                let _ = std::fs::remove_file(old);
            }
        }
        Ok(Learned {
            skill,
            change_id,
            created,
        })
    }

    /// Undo one learned change: put the old text back, or remove a skill
    /// that the change created. Returns the skill's name.
    pub fn undo(&self, change_id: &str) -> Result<String, AppError> {
        if change_id.is_empty()
            || !change_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(AppError::invalid_input("That change is not known."));
        }
        let file = self.dir.join(".history").join(format!("{change_id}.json"));
        let text = std::fs::read_to_string(&file).map_err(|_| {
            AppError::invalid_input("That change was already undone or is too old.")
        })?;
        let record: SkillUndo = serde_json::from_str(&text).map_err(AppError::from)?;
        match &record.previous {
            Some(previous) => {
                let path = self.guard_path(&record.path)?;
                std::fs::write(path, previous)
                    .map_err(|e| AppError::storage(format!("Could not restore the skill: {e}")))?;
            }
            None => {
                if let Ok(path) = self.guard_path(&record.path) {
                    std::fs::remove_file(path).map_err(|e| {
                        AppError::storage(format!("Could not remove the skill: {e}"))
                    })?;
                }
            }
        }
        let _ = std::fs::remove_file(file);
        Ok(record.name)
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
            source: front.get("source").cloned().filter(|s| !s.is_empty()),
            license: front.get("license").cloned().filter(|s| !s.is_empty()),
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
    if let Some(after) = rest.strip_prefix("---")
        && let Some(end) = after.find("\n---")
    {
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
    (front, raw.to_string())
}

/// Serialize a skill back to its markdown file format.
fn render_skill_file(skill: &Skill) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", skill.name));
    out.push_str(&format!("description: {}\n", skill.description));
    if !skill.tags.is_empty() {
        let tags: Vec<String> = skill.tags.iter().map(|t| one_line(t)).collect();
        out.push_str(&format!("tags: {}\n", tags.join(", ")));
    }
    if let Some(source) = &skill.source {
        out.push_str(&format!("source: {}\n", one_line(source)));
    }
    if let Some(license) = &skill.license {
        out.push_str(&format!("license: {}\n", one_line(license)));
    }
    out.push_str("---\n\n");
    out.push_str(skill.body.trim());
    out.push('\n');
    out
}

/// Frontmatter values are single lines. Collapse whitespace and newlines so
/// an imported description cannot end the header early.
pub(crate) fn one_line(value: &str) -> String {
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
    fn learned_skills_can_be_undone() {
        let dir = std::env::temp_dir().join(format!("la-learn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let skills = SkillsService::with_dir(dir.clone());

        let first = skills
            .learn(
                "Weekly report",
                "How the user wants reports",
                "Use 3 bullets.",
            )
            .unwrap();
        assert!(first.created);
        let second = skills
            .learn("weekly report", "", "Use 3 bullets and a summary line.")
            .unwrap();
        assert!(!second.created);
        let now = skills.list().unwrap();
        assert_eq!(now.len(), 1);
        assert!(now[0].body.contains("summary line"));
        assert!(now[0].tags.iter().any(|t| t == "learned"));

        assert_eq!(skills.undo(&second.change_id).unwrap(), "Weekly report");
        assert_eq!(skills.list().unwrap()[0].body.trim(), "Use 3 bullets.");
        skills.undo(&first.change_id).unwrap();
        assert!(skills.list().unwrap().is_empty());
        assert!(skills.undo(&first.change_id).is_err(), "only once");
        assert!(skills.undo("../../etc").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn imported_skills_keep_their_origin() {
        let dir = std::env::temp_dir().join(format!("localaide-skills-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let service = SkillsService::with_dir(dir.clone());
        let first = service
            .create_with_origin(
                "Code Review!",
                "Line one\nline two",
                vec!["imported".into()],
                "Review this.",
                Some("https://github.com/a/b".into()),
                Some("MIT".into()),
            )
            .unwrap();
        assert_eq!(first.description, "Line one line two");
        // A different name with the same slug gets its own file.
        let second = service
            .create("Code Review?", "", vec![], "Again.")
            .unwrap();
        assert_ne!(first.path, second.path);
        let listed = service.list().unwrap();
        assert_eq!(listed.len(), 2);
        let read = service.read(&first.path).unwrap();
        assert_eq!(read.source.as_deref(), Some("https://github.com/a/b"));
        assert_eq!(read.license.as_deref(), Some("MIT"));
        // Editing keeps the origin.
        let edited = service
            .update(&first.path, "Code Review!", "New", vec![], "Changed.")
            .unwrap();
        assert_eq!(edited.source.as_deref(), Some("https://github.com/a/b"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slug_is_filename_safe() {
        assert_eq!(slugify("Code Review!"), "code-review");
        assert_eq!(slugify("  --weird__name--  "), "weird-name");
    }
}
