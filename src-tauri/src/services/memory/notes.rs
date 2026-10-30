use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use crate::app::error::AppError;
use crate::domain::memory::MemoryNote;
use crate::services::library::db::{DbNote, LibraryDb};
use crate::services::library::LibraryService;

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
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();

        let filename = if safe_title.trim().is_empty() {
            format!("note_{}.md", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs())
        } else {
            format!("{safe_title}.md")
        };

        let path = self.notes_dir.join(&filename);
        let tags_vec = tags.unwrap_or_default();
        let tags_header = if !tags_vec.is_empty() {
            format!("<!-- tags: {} -->\n", tags_vec.join(", "))
        } else {
            String::new()
        };

        let markdown = format!("# {title}\n{tags_header}\n{content}\n");
        fs::write(&path, &markdown)
            .map_err(|e| AppError::storage(format!("Failed to write memory note to {}: {e}", path.display())))?;

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
        let p = Path::new(path_str);
        if !p.exists() {
            return Err(AppError::invalid_input(format!("Memory note not found: {path_str}")));
        }

        let existing = self.db.get_note_by_path(path_str)?;
        let title = existing
            .as_ref()
            .map(|n| n.title.clone())
            .unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".to_string()));

        let tags_vec = tags.unwrap_or_else(|| existing.as_ref().map(|n| n.tags.clone()).unwrap_or_default());
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
            id: existing.as_ref().map(|n| n.id.clone()).unwrap_or_else(|| format!("note_{now}")),
            path: path_str.to_string(),
            title,
            content: content.to_string(),
            tags: tags_vec,
            size: meta.len(),
            modified,
            created_at: existing.as_ref().map(|n| n.created_at).unwrap_or(now),
        };

        self.db.insert_or_replace_note(&db_note)?;
        let _ = self.library.index_file(p);

        Ok(())
    }

    pub fn read_note(&self, path_str: &str) -> Result<String, AppError> {
        let p = Path::new(path_str);
        if !p.exists() {
            return Err(AppError::invalid_input(format!("Memory note not found: {path_str}")));
        }
        fs::read_to_string(p).map_err(|e| AppError::storage(format!("Failed to read note {path_str}: {e}")))
    }

    pub fn delete_note(&self, path_str: &str) -> Result<bool, AppError> {
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
        let results = db_notes
            .into_iter()
            .map(|n| MemoryNote {
                id: n.id,
                path: n.path,
                title: n.title,
                content: Some(n.content),
                tags: n.tags,
                size: n.size,
                modified: n.modified,
            })
            .collect();
        Ok(results)
    }

    pub fn forget_all(&self) -> Result<usize, AppError> {
        let mut count = 0;
        // Delete disk files
        if self.notes_dir.exists() {
            if let Ok(entries) = fs::read_dir(&self.notes_dir) {
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
        }
        // Clear SQLite notes table
        let _ = self.db.clear_all_notes();
        Ok(count)
    }
}
