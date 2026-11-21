pub mod notes;
pub mod rag;
pub mod suggest;

use self::notes::NotesManager;
use self::rag::{RagEngine, RagRecallResult};
use self::suggest::{Suggestion, SuggestionsManager};
use crate::app::error::AppError;
use crate::domain::document::SearchResultDoc;
use crate::domain::memory::MemoryNote;
use crate::services::library::LibraryService;
use crate::services::library::db::LibraryDb;
use crate::services::library::search::SearchMode;
use std::path::PathBuf;
use std::sync::Arc;

pub struct MemoryService {
    notes: NotesManager,
    rag: RagEngine,
    library: Arc<LibraryService>,
    suggestions: SuggestionsManager,
}

impl MemoryService {
    pub fn new(
        db: Arc<LibraryDb>,
        library: Arc<LibraryService>,
        notes_dir: PathBuf,
        suggestions_dir: PathBuf,
    ) -> Self {
        let notes = NotesManager::new(db, library.clone(), notes_dir);
        let rag = RagEngine::new(library.clone());
        Self {
            notes,
            rag,
            library,
            suggestions: SuggestionsManager::new(suggestions_dir),
        }
    }

    pub fn create_note(
        &self,
        title: &str,
        content: &str,
        tags: Option<Vec<String>>,
    ) -> Result<MemoryNote, AppError> {
        self.notes.create_note(title, content, tags)
    }

    pub fn update_note(
        &self,
        path: &str,
        content: &str,
        tags: Option<Vec<String>>,
    ) -> Result<(), AppError> {
        self.notes.update_note(path, content, tags)
    }

    pub fn read_note(&self, path: &str) -> Result<String, AppError> {
        self.notes.read_note(path)
    }

    pub fn delete_note(&self, path: &str) -> Result<bool, AppError> {
        self.notes.delete_note(path)
    }

    pub fn list_notes(&self) -> Result<Vec<MemoryNote>, AppError> {
        self.notes.list_notes()
    }

    pub fn forget_all(&self) -> Result<usize, AppError> {
        self.notes.forget_all()
    }

    /// Pins or unpins a note; pinned notes ride in every chat with recall on.
    pub fn set_pinned(&self, path: &str, pinned: bool) -> Result<MemoryNote, AppError> {
        self.notes.set_pinned(path, pinned)
    }

    /// The block that rides in every chat's context: the notes the user
    /// pinned, oldest first. Empty when nothing is pinned.
    pub fn pinned_context(&self) -> Result<String, AppError> {
        let notes = self.notes.pinned_notes()?;
        if notes.is_empty() {
            return Ok(String::new());
        }
        let mut lines = vec![
            "Notes the user pinned - always in context; use them when relevant and say you remembered them.".to_string(),
        ];
        for n in notes {
            lines.push(format!("- {}: {}", n.title, n.content.trim()));
        }
        Ok(lines.join("
"))
    }

    /// The model offered to remember a fact: it waits in the review queue.
    pub fn suggest_note(&self, title: &str, content: &str) -> Result<Suggestion, AppError> {
        self.suggestions.suggest(title, content)
    }

    pub fn pending_suggestions(&self) -> Result<Vec<Suggestion>, AppError> {
        self.suggestions.list()
    }

    /// Accepts a suggestion: it becomes a real note and leaves the queue.
    pub fn accept_suggestion(&self, file: &str, tags: Vec<String>) -> Result<MemoryNote, AppError> {
        let suggestion = self.suggestions.read(file)?;
        let tags_opt = if tags.is_empty() { None } else { Some(tags) };
        let note = self.notes.create_note(&suggestion.title, &suggestion.content, tags_opt)?;
        let _ = self.suggestions.dismiss(file);
        Ok(note)
    }

    pub fn dismiss_suggestion(&self, file: &str) -> Result<bool, AppError> {
        self.suggestions.dismiss(file)
    }

    pub fn recall(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
        max_tokens: Option<usize>,
    ) -> Result<RagRecallResult, AppError> {
        self.rag.recall(query, query_vector, max_tokens)
    }

    pub fn search(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        self.library
            .search(query, query_vector, SearchMode::Hybrid, 20)
    }
}
