pub mod db;
pub mod embedding;
pub mod ingest;
pub mod rerank;
pub mod search;
pub mod watch;

use self::db::LibraryDb;
use self::embedding::EmbeddingEngine;
use self::ingest::FileIngester;
use self::search::{HybridSearcher, SearchMode};
use self::watch::{LibraryUpdater, LibraryWatcher};
use crate::app::error::AppError;
use crate::domain::document::{IndexerStatsResponse, SearchResultDoc};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct LibraryService {
    db: Arc<LibraryDb>,
    searcher: HybridSearcher,
    watcher: LibraryWatcher,
    /// The local embedding engine, when one has been wired in. Chunks are
    /// vectorized at ingest and queries embed before hybrid/semantic search.
    embedding: RwLock<Option<Arc<EmbeddingEngine>>>,
}

impl LibraryService {
    pub fn new(db: Arc<LibraryDb>) -> Self {
        let searcher = HybridSearcher::new(db.clone());
        let watcher = LibraryWatcher::new();
        Self {
            db,
            searcher,
            watcher,
            embedding: RwLock::new(None),
        }
    }

    /// Attach the embedding engine after construction (AppState builds the
    /// engine and the library side by side).
    pub fn set_embedding_engine(&self, engine: Arc<EmbeddingEngine>) {
        *self
            .embedding
            .write()
            .expect("embedding slot lock poisoned") = Some(engine);
    }

    fn embedding_engine(&self) -> Option<Arc<EmbeddingEngine>> {
        self.embedding
            .read()
            .expect("embedding slot lock poisoned")
            .clone()
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
        let mut chunks = ingested.chunks;
        // Vectorize the chunks when the embedding engine is running; the
        // insert stays keyword-only otherwise, and a backfill can fill
        // these in later.
        if let Some(engine) = self.embedding_engine()
            && engine.is_running()
        {
            let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
            if let Ok(vectors) = engine.embed(&texts) {
                for (chunk, vector) in chunks.iter_mut().zip(vectors) {
                    chunk.embedding = Some(vector);
                }
            }
        }
        self.db
            .insert_or_replace_document(&ingested.document, &chunks)?;

        let snippet = if let Some(first_chunk) = chunks.first() {
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
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::Relaxed)
            {
                break;
            }

            let entries = match std::fs::read_dir(&current_dir) {
                Ok(entries) => entries,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };

            for entry in entries.flatten() {
                if let Some(ref flag) = cancel_flag
                    && flag.load(Ordering::Relaxed)
                {
                    break;
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

    /// Search the library. `query_vector` comes from the embedding engine
    /// for semantic and hybrid modes; keyword mode ignores it, and hybrid
    /// degrades to keyword ranking when no vector is available.
    pub fn search(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
        mode: SearchMode,
        limit: usize,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        self.searcher.search(query, query_vector, mode, limit)
    }

    /// Fill in stored vectors for chunks that were indexed without the
    /// embedding engine running. Returns how many chunks were embedded.
    pub fn backfill_embeddings(&self, batch_size: usize) -> Result<usize, AppError> {
        let engine = match self.embedding_engine() {
            Some(engine) if engine.is_running() => engine,
            _ => {
                return Err(AppError::new(
                    "EMBEDDING_NOT_READY",
                    "Start the embedding engine before backfilling",
                ));
            }
        };

        let mut updated = 0;
        loop {
            let missing = self.db.chunks_missing_embeddings(batch_size)?;
            if missing.is_empty() {
                break;
            }
            let texts: Vec<String> = missing.iter().map(|(_, text)| text.clone()).collect();
            let vectors = engine.embed(&texts)?;
            for ((chunk_id, _), vector) in missing.iter().zip(vectors) {
                self.db.update_chunk_embedding(chunk_id, &vector)?;
                updated += 1;
            }
            if missing.len() < batch_size {
                break;
            }
        }
        Ok(updated)
    }

    /// (total chunks, chunks with stored vectors).
    pub fn embedding_coverage(&self) -> Result<(usize, usize), AppError> {
        self.db.embedding_coverage()
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
