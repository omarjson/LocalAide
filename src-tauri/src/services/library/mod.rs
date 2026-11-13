pub mod db;
pub mod ingest;
pub mod search;
pub mod watch;

use self::db::{DbDocument, LibraryDb};
use self::ingest::{FileIngester, IngestedFile};
use self::search::{HybridSearcher, SearchMode};
use self::watch::{LibraryUpdater, LibraryWatcher};
use crate::app::error::AppError;
use crate::domain::document::{IndexerStatsResponse, SearchResultDoc};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct LibraryService {
    db: Arc<LibraryDb>,
    searcher: HybridSearcher,
    watcher: LibraryWatcher,
}

impl LibraryService {
    pub fn new(db: Arc<LibraryDb>) -> Self {
        let searcher = HybridSearcher::new(db.clone());
        let watcher = LibraryWatcher::new();
        Self {
            db,
            searcher,
            watcher,
        }
    }

    pub fn open_default(storage_dir: &Path) -> Result<Self, AppError> {
        let db_path = storage_dir.join("library.db");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::storage(format!("Failed to create library directory: {e}"))
            })?;
        }
        let db = Arc::new(LibraryDb::open(&db_path)?);
        Ok(Self::new(db))
    }

    pub fn open_in_memory() -> Result<Self, AppError> {
        let db = Arc::new(LibraryDb::open_in_memory()?);
        Ok(Self::new(db))
    }

    pub fn db(&self) -> Arc<LibraryDb> {
        self.db.clone()
    }

    pub fn index_file(&self, path: &Path) -> Result<SearchResultDoc, AppError> {
        let ingested = FileIngester::ingest_file(path)?;
        self.db
            .insert_or_replace_document(&ingested.document, &ingested.chunks)?;

        let snippet = if let Some(first_chunk) = ingested.chunks.first() {
            if first_chunk.text.chars().count() > 200 {
                format!(
                    "{}...",
                    first_chunk.text.chars().take(200).collect::<String>()
                )
            } else {
                first_chunk.text.clone()
            }
        } else {
            String::new()
        };

        Ok(SearchResultDoc {
            path: ingested.document.path,
            title: ingested.document.title,
            snippet,
            score: 1.0,
            modified: ingested.document.modified,
            size: ingested.document.size,
        })
    }

    pub fn index_folder(
        &self,
        folder: &Path,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<(usize, usize), AppError> {
        if !folder.exists() || !folder.is_dir() {
            return Err(AppError::invalid_input(format!(
                "Directory does not exist: {}",
                folder.display()
            )));
        }

        let mut indexed = 0;
        let mut skipped = 0;
        let mut stack = vec![folder.to_path_buf()];

        while let Some(current_dir) = stack.pop() {
            if let Some(ref flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
            }

            let entries = match std::fs::read_dir(&current_dir) {
                Ok(entries) => entries,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };

            for entry in entries.flatten() {
                if let Some(ref flag) = cancel_flag {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                }

                let p = entry.path();
                // Ignore rules apply inside the chosen folder only. Ancestors such as
                // `~/.notes` or `~/target/...` must not hide the whole folder.
                let inside = p.strip_prefix(folder).unwrap_or(p.as_path());
                if FileIngester::should_ignore_path(inside) {
                    continue;
                }

                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() {
                    if !FileIngester::is_supported_extension(&p) {
                        skipped += 1;
                        continue;
                    }

                    match self.index_file(&p) {
                        Ok(_) => indexed += 1,
                        Err(e) => {
                            log::debug!("Skipping file {}: {e}", p.display());
                            skipped += 1;
                        }
                    }
                }
            }
        }

        Ok((indexed, skipped))
    }

    pub fn search(
        &self,
        query: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        self.searcher.search(query, None, mode, limit)
    }

    pub fn list(&self) -> Result<Vec<SearchResultDoc>, AppError> {
        let docs = self.db.list_documents()?;
        let results = docs
            .into_iter()
            .map(|d| SearchResultDoc {
                path: d.path,
                title: d.title,
                snippet: format!("Type: {}, Size: {} bytes", d.kind, d.size),
                score: 1.0,
                modified: d.modified,
                size: d.size,
            })
            .collect();
        Ok(results)
    }

    pub fn remove(&self, path: &str) -> Result<bool, AppError> {
        self.db.delete_document_by_path(path)
    }

    pub fn stats(&self) -> Result<IndexerStatsResponse, AppError> {
        let s = self.db.get_stats()?;
        Ok(IndexerStatsResponse {
            doc_count: s.doc_count,
            total_bytes: s.total_bytes,
            storage_path: s.storage_path,
        })
    }

    pub fn start_watch(self: &Arc<Self>, folder: &Path) -> Result<(), AppError> {
        self.watcher.start_watch(folder, self.clone())
    }

    pub fn stop_watch(&self) {
        self.watcher.stop_watch();
    }
}

impl LibraryUpdater for LibraryService {
    fn update_file(&self, path: &Path) -> Result<(), AppError> {
        self.index_file(path).map(|_| ())
    }

    fn remove_file(&self, path: &str) -> Result<bool, AppError> {
        self.remove(path)
    }
}
