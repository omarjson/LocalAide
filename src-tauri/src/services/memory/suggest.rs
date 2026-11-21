//! Suggested notes: the model can offer to remember a fact without
//! interrupting the chat. Suggestions wait in a folder next to the app
//! data until the user accepts or dismisses them on the Memory screen.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::app::error::AppError;

/// One waiting suggestion, as the Memory screen shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    /// Plain file name inside the suggestions folder.
    pub file: String,
    pub title: String,
    pub content: String,
    pub modified: u64,
}

pub struct SuggestionsManager {
    dir: PathBuf,
}

impl SuggestionsManager {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Only a `.md` file name directly inside the suggestions folder is
    /// accepted — the same jail the memory notes use, since file names
    /// come back from the model and the frontend.
    fn resolve(&self, file: &str) -> Result<PathBuf, AppError> {
        let path = Path::new(file);
        let in_folder = path.parent() == Some(Path::new(""));
        let is_markdown = path.extension().and_then(|e| e.to_str()) == Some("md");
        if in_folder && is_markdown && !file.contains('/') && !file.contains('\\') {
            Ok(self.dir.join(file))
        } else {
            Err(AppError::invalid_input(format!(
                "Not a suggested note: {file}"
            )))
        }
    }

    /// Saves a suggestion, naming the file after the title like notes do.
    pub fn suggest(&self, title: &str, content: &str) -> Result<Suggestion, AppError> {
        fs::create_dir_all(&self.dir).map_err(|e| {
            AppError::storage(format!("Failed to create the suggestions folder: {e}"))
        })?;
        let safe_title: String = title
            .chars()
            .take(64)
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let stem = if safe_title.trim().is_empty() {
            format!(
                "suggestion_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            )
        } else {
            safe_title
        };
        let markdown = format!("# {title}\n\n{}\n", content.trim());
        for n in 1..=1000 {
            let name = if n == 1 {
                format!("{stem}.md")
            } else {
                format!("{stem}_{n}.md")
            };
            let path = self.dir.join(&name);
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    f.write_all(markdown.as_bytes())
                        .map_err(|e| AppError::storage(format!("Failed to write the suggestion: {e}")))?;
                    return Ok(Suggestion {
                        file: name,
                        title: title.trim().to_string(),
                        content: content.trim().to_string(),
                        modified: modified_secs(&path),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(AppError::storage(format!(
                        "Failed to create the suggestion: {e}"
                    )));
                }
            }
        }
        Err(AppError::storage(
            "Too many suggestions share this title. Use a different title.",
        ))
    }

    /// Every waiting suggestion, by file name.
    pub fn list(&self) -> Result<Vec<Suggestion>, AppError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries: Vec<_> = fs::read_dir(&self.dir)
            .map_err(|e| AppError::storage(format!("Failed to read the suggestions folder: {e}")))?
            .flatten()
            .collect();
        entries.sort_by_key(|e| e.file_name());
        let mut out = Vec::new();
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(_) => continue,
            };
            let file = entry.file_name().to_string_lossy().into_owned();
            out.push(parse_suggestion(&file, &raw, modified_secs(&path)));
        }
        Ok(out)
    }

    /// Reads one suggestion back, for accepting it.
    pub fn read(&self, file: &str) -> Result<Suggestion, AppError> {
        let path = self.resolve(file)?;
        let raw = fs::read_to_string(&path)
            .map_err(|e| AppError::storage(format!("Failed to read the suggestion: {e}")))?;
        Ok(parse_suggestion(file, &raw, 0))
    }

    pub fn dismiss(&self, file: &str) -> Result<bool, AppError> {
        let path = self.resolve(file)?;
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| AppError::storage(format!("Failed to dismiss the suggestion: {e}")))?;
            return Ok(true);
        }
        Ok(false)
    }
}

/// The file is `# Title\n\n<body>`; the body is everything after the
/// first line, trimmed.
fn parse_suggestion(file: &str, raw: &str, modified: u64) -> Suggestion {
    let mut lines = raw.lines();
    let title = lines
        .next()
        .and_then(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| file.trim_end_matches(".md").to_string());
    let content = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    Suggestion {
        file: file.to_string(),
        title,
        content,
        modified,
    }
}

fn modified_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(tag: &str) -> SuggestionsManager {
        let dir = std::env::temp_dir().join(format!(
            "localaide-suggest-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        SuggestionsManager::new(dir)
    }

    #[test]
    fn suggest_list_read_dismiss_roundtrip() {
        let m = manager("roundtrip");
        let made = m.suggest("User's stack", "Prefers Rust and Svelte.")?;
        let all = m.list()?;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].file, made.file);
        assert_eq!(all[0].title, "User's stack");
        assert_eq!(all[0].content, "Prefers Rust and Svelte.");

        let read = m.read(&made.file)?;
        assert_eq!(read.title, "User's stack");
        assert_eq!(read.content, "Prefers Rust and Svelte.");

        assert!(m.dismiss(&made.file)?);
        assert!(m.list()?.is_empty());
        let _ = fs::remove_dir_all(&m.dir);
        Ok::<(), AppError>(())
    }

    #[test]
    fn duplicate_titles_get_numbered_files() {
        let m = manager("dupes");
        let a = m.suggest("Same title", "first")?;
        let b = m.suggest("Same title", "second")?;
        assert_ne!(a.file, b.file);
        assert_eq!(m.list()?.len(), 2);
        let _ = fs::remove_dir_all(&m.dir);
    }

    #[test]
    fn file_names_from_outside_are_refused() {
        let m = manager("jail");
        assert!(m.read("../escape.md").is_err());
        assert!(m.read("sub/dir.md").is_err());
        assert!(m.read("not-markdown.txt").is_err());
        let _ = fs::remove_dir_all(&m.dir);
    }
}
