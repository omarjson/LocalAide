pub mod notes;
pub mod rag;

use self::notes::NotesManager;
use self::rag::{RagEngine, RagRecallResult};
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
}

impl MemoryService {
    pub fn new(db: Arc<LibraryDb>, library: Arc<LibraryService>, notes_dir: PathBuf) -> Self {
        let notes = NotesManager::new(db, library.clone(), notes_dir);
        let rag = RagEngine::new(library.clone());
        Self {
            notes,
            rag,
            library,
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
        self.library.search(query, query_vector, SearchMode::Hybrid, 20)
    }
}
