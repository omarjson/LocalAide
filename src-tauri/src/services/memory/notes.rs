use crate::app::error::AppError;
use crate::domain::memory::MemoryNote;
use crate::services::library::LibraryService;
use crate::services::library::db::{DbNote, LibraryDb};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NOTE_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct NotesManager {
    db: Arc<LibraryDb>,
    library: Arc<LibraryService>,
    notes_dir: PathBuf,
}

impl NotesManager {
    pub fn new(db: Arc<LibraryDb>, library: Arc<LibraryService>, notes_dir: PathBuf) -> Self {
        Self {
            db,
            library,
            notes_dir,
        }
    }

    /// Note paths come back from the frontend, so only a `.md` file that sits
    /// directly in the notes folder is accepted. Any other path is refused
    /// before a file is read, changed or deleted.
    fn ensure_note_path(&self, path_str: &str) -> Result<(), AppError> {
        let path = Path::new(path_str);
        let in_notes_folder = path.parent() == Some(self.notes_dir.as_path());
        let is_markdown = path.extension().and_then(|e| e.to_str()) == Some("md");
        if in_notes_folder && is_markdown {
            Ok(())
        } else {
            Err(AppError::invalid_input(format!(
                "Not a memory note: {path_str}"
            )))
        }
    }

    /// Writes a new note file named `<stem>.md`. A note with that name is never
    /// replaced: the name gets `_2`, `_3` and so on until it is free. The file
    /// is created only if it does not exist yet, so two saves cannot race.
    fn write_new_note_file(&self, stem: &str, markdown: &str) -> Result<PathBuf, AppError> {
        use std::io::Write;

        for n in 1..=1000 {
            let name = if n == 1 {
                format!("{stem}.md")
            } else {
                format!("{stem}_{n}.md")
            };
            let path = self.notes_dir.join(name);
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(markdown.as_bytes()).map_err(|e| {
                        AppError::storage(format!(
                            "Failed to write memory note to {}: {e}",
                            path.display()
                        ))
                    })?;
                    return Ok(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(AppError::storage(format!(
                        "Failed to create memory note {}: {e}",
                        path.display()
                    )));
                }
            }
        }
        Err(AppError::storage(
            "Too many notes share this title. Use a different title.",
        ))
    }

    pub fn create_note(
        &self,
        title: &str,
        content: &str,
        tags: Option<Vec<String>>,
    ) -> Result<MemoryNote, AppError> {
        fs::create_dir_all(&self.notes_dir)
            .map_err(|e| AppError::storage(format!("Failed to create memory directory: {e}")))?;

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
                "note_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            )
        } else {
            avoid_reserved_name(safe_title)
        };

        let tags_vec = tags.unwrap_or_default();
        let tags_header = if !tags_vec.is_empty() {
            format!("<!-- tags: {} -->\n", tags_vec.join(", "))
        } else {
            String::new()
        };

        let markdown = format!("# {title}\n{tags_header}\n{content}\n");
        let path = self.write_new_note_file(&stem, &markdown)?;

        let meta = fs::metadata(&path)
            .map_err(|e| AppError::storage(format!("Failed to read note metadata: {e}")))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let now = (nanos / 1_000_000_000) as u64;
        let count = NOTE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let note_id = format!("note_{nanos}_{count}");
        let path_str = path.to_string_lossy().to_string();

        let db_note = DbNote {
            id: note_id.clone(),
            path: path_str.clone(),
            title: title.to_string(),
            content: content.to_string(),
            tags: tags_vec.clone(),
            size: meta.len(),
            modified,
            created_at: now,
            pinned: false,
        };

        self.db.insert_or_replace_note(&db_note)?;

        // Auto-index into unified library for hybrid search
        self.library.index_file(&path)?;

        Ok(MemoryNote {
            id: note_id,
            path: path_str,
            title: title.to_string(),
            content: Some(content.to_string()),
            tags: tags_vec,
            size: meta.len(),
            modified,
        })
    }

    pub fn update_note(
        &self,
        path_str: &str,
        content: &str,
        tags: Option<Vec<String>>,
    ) -> Result<(), AppError> {
        self.ensure_note_path(path_str)?;
        let p = Path::new(path_str);
        if !p.exists() {
            return Err(AppError::invalid_input(format!(
                "Memory note not found: {path_str}"
            )));
        }

        let existing = self.db.get_note_by_path(path_str)?;
        let title = existing
            .as_ref()
            .map(|n| n.title.clone())
            .unwrap_or_else(|| {
                p.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string())
            });

        let tags_vec = tags.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|n| n.tags.clone())
                .unwrap_or_default()
        });
        let tags_header = if !tags_vec.is_empty() {
            format!("<!-- tags: {} -->\n", tags_vec.join(", "))
        } else {
            String::new()
        };

        let markdown = format!("# {title}\n{tags_header}\n{content}\n");
        fs::write(p, &markdown)
            .map_err(|e| AppError::storage(format!("Failed to update note: {e}")))?;

        let meta = fs::metadata(p)
            .map_err(|e| AppError::storage(format!("Failed to read metadata: {e}")))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let db_note = DbNote {
            id: existing
                .as_ref()
                .map(|n| n.id.clone())
                .unwrap_or_else(|| format!("note_{now}")),
            path: path_str.to_string(),
            title,
            content: content.to_string(),
            tags: tags_vec,
            size: meta.len(),
            modified,
            created_at: existing.as_ref().map(|n| n.created_at).unwrap_or(now),
            pinned: existing.as_ref().map(|n| n.pinned).unwrap_or(false),
        };

        self.db.insert_or_replace_note(&db_note)?;
        let _ = self.library.index_file(p);

        Ok(())
    }

    pub fn read_note(&self, path_str: &str) -> Result<String, AppError> {
        self.ensure_note_path(path_str)?;
        let p = Path::new(path_str);
        if !p.exists() {
            return Err(AppError::invalid_input(format!(
                "Memory note not found: {path_str}"
            )));
        }
        fs::read_to_string(p)
            .map_err(|e| AppError::storage(format!("Failed to read note {path_str}: {e}")))
    }

    pub fn delete_note(&self, path_str: &str) -> Result<bool, AppError> {
        self.ensure_note_path(path_str)?;
        let p = Path::new(path_str);
        if p.exists() {
            let _ = fs::remove_file(p);
        }
        let deleted = self.db.delete_note_by_path(path_str)?;
        let _ = self.library.remove(path_str);
        Ok(deleted)
    }

    pub fn list_notes(&self) -> Result<Vec<MemoryNote>, AppError> {
        // Read synchronized records from SQLite
        let db_notes = self.db.list_notes()?;
        Ok(db_notes.into_iter().map(Self::to_note).collect())
    }

    /// Pins or unpins a note: pinned notes ride in every chat's context
    /// while recall is on.
    pub fn set_pinned(&self, path_str: &str, pinned: bool) -> Result<MemoryNote, AppError> {
        self.ensure_note_path(path_str)?;
        let updated = self.db.set_note_pinned(path_str, pinned)?;
        if !updated {
            return Err(AppError::invalid_input(format!(
                "Memory note not found: {path_str}"
            )));
        }
        match self.db.get_note_by_path(path_str)? {
            Some(n) => Ok(Self::to_note(n)),
            None => Err(AppError::invalid_input(format!(
                "Memory note not found: {path_str}"
            ))),
        }
    }

    /// The pinned notes, in the order the user pinned them.
    pub fn pinned_notes(&self) -> Result<Vec<DbNote>, AppError> {
        self.db.pinned_notes()
    }

    fn to_note(n: DbNote) -> MemoryNote {
        MemoryNote {
            id: n.id,
            path: n.path,
            title: n.title,
            content: Some(n.content),
            tags: n.tags,
            size: n.size,
            modified: n.modified,
            pinned: n.pinned,
        }
    }

    pub fn forget_all(&self) -> Result<usize, AppError> {
        let mut count = 0;
        // Delete disk files
        if self.notes_dir.exists()
            && let Ok(entries) = fs::read_dir(&self.notes_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let path_str = path.to_string_lossy().to_string();
                    let _ = fs::remove_file(&path);
                    let _ = self.library.remove(&path_str);
                    count += 1;
                }
            }
        }
        // Clear SQLite notes table
        let _ = self.db.clear_all_notes();
        Ok(count)
    }
}

/// Windows reserves CON, PRN, AUX, NUL, COM1 to COM9 and LPT1 to LPT9 as file
/// names, whatever the extension, so a note with such a title gets a suffix.
fn avoid_reserved_name(stem: String) -> String {
    let upper = stem.to_ascii_uppercase();
    let numbered_port = upper.len() == 4
        && (upper.starts_with("COM") || upper.starts_with("LPT"))
        && matches!(upper.as_bytes()[3], b'1'..=b'9');
    if numbered_port || matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        format!("{stem}_note")
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_with_the_same_title_are_both_kept() {
        let root = std::env::temp_dir().join(format!("la-notes-same-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let notes_dir = root.join("memory");
        let library = Arc::new(LibraryService::open_in_memory().unwrap());
        let manager = NotesManager::new(library.db(), library.clone(), notes_dir.clone());

        let first = manager.create_note("Groceries", "milk", None).unwrap();
        let second = manager.create_note("Groceries", "bread", None).unwrap();
        assert_ne!(first.path, second.path);
        assert!(fs::read_to_string(&first.path).unwrap().contains("milk"));
        assert!(fs::read_to_string(&second.path).unwrap().contains("bread"));

        let reserved = manager.create_note("CON", "a note", None).unwrap();
        assert!(reserved.path.ends_with("CON_note.md"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reserved_windows_names_get_a_suffix() {
        for name in ["con", "NUL", "Com1", "lpt9"] {
            assert_eq!(
                avoid_reserved_name(name.to_string()),
                format!("{name}_note")
            );
        }
        for name in ["COM0", "COM10", "CONSOLE", "Notes"] {
            assert_eq!(avoid_reserved_name(name.to_string()), name);
        }
    }

    #[test]
    fn note_paths_must_be_markdown_files_in_the_notes_folder() {
        let root = std::env::temp_dir().join(format!("la-notes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let notes_dir = root.join("memory");
        fs::create_dir_all(&notes_dir).unwrap();
        let library = Arc::new(LibraryService::open_in_memory().unwrap());
        let manager = NotesManager::new(library.db(), library.clone(), notes_dir.clone());

        let note = notes_dir.join("plan.md");
        fs::write(&note, "# Plan").unwrap();
        let outside = root.join("secret.md");
        fs::write(&outside, "keep").unwrap();
        let escape = notes_dir.join("..").join("secret.md");

        assert!(manager.read_note(&note.to_string_lossy()).is_ok());
        assert!(manager.read_note(&outside.to_string_lossy()).is_err());
        assert!(manager.read_note(&escape.to_string_lossy()).is_err());
        assert!(
            manager
                .update_note(&outside.to_string_lossy(), "changed", None)
                .is_err()
        );
        assert!(manager.delete_note(&outside.to_string_lossy()).is_err());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep");

        let _ = fs::remove_dir_all(&root);
    }
}
