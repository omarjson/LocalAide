pub mod db;
pub mod embedding;
pub mod ingest;
pub mod rerank;
pub mod search;
pub mod watch;

use self::db::LibraryDb;
use self::embedding::EmbeddingEngine;
use self::ingest::FileIngester;
use self::rerank::{RERANK_CANDIDATES, RerankEngine, reorder};
use self::search::{HybridSearcher, SearchMode};
use self::watch::{LibraryUpdater, LibraryWatcher};
use crate::app::error::AppError;
use crate::domain::document::{IndexerStatsResponse, SearchResultDoc};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Deepest folder level that "Add a folder" goes into.
pub const MAX_FOLDER_DEPTH: usize = 32;
/// Most files one "Add a folder" looks at.
pub const MAX_FOLDER_FILES: usize = 20_000;

/// What one "Add a folder" did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FolderReport {
    /// Files now in the library, including unchanged files it already had.
    pub indexed: usize,
    /// Files it could not read, or of a type it does not read.
    pub skipped: usize,
    /// True when it stopped at `MAX_FOLDER_FILES` or `MAX_FOLDER_DEPTH`.
    pub limited: bool,
}

pub struct LibraryService {
    db: Arc<LibraryDb>,
    searcher: HybridSearcher,
    watcher: LibraryWatcher,
    /// The local embedding engine, when one has been wired in. Chunks are
    /// vectorized at ingest and queries embed before hybrid/semantic search.
    embedding: RwLock<Option<Arc<EmbeddingEngine>>>,
    /// The local reranker engine, when one has been wired in. When running,
    /// search re-orders the top candidates by query-document relevance.
    rerank: RwLock<Option<Arc<RerankEngine>>>,
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
            rerank: RwLock::new(None),
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

    /// Attach the reranker engine after construction.
    pub fn set_rerank_engine(&self, engine: Arc<RerankEngine>) {
        *self.rerank.write().expect("reranker slot lock poisoned") = Some(engine);
    }

    fn rerank_engine(&self) -> Option<Arc<RerankEngine>> {
        self.rerank
            .read()
            .expect("reranker slot lock poisoned")
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
        // A file with the size and change time it had when it was indexed is
        // not read again. This keeps re-adding a folder, and watcher events,
        // cheap.
        let stored = self.db.get_document_by_path(&path.to_string_lossy())?;
        if let Some(doc) = &stored
            && let Ok(meta) = std::fs::metadata(path)
            && meta.len() == doc.size
            && modified_secs(&meta) == doc.modified
        {
            return Ok(stored_result(doc));
        }

        let ingested = FileIngester::ingest_file(path)?;
        // Same bytes with a new change time: keep the stored chunks and
        // vectors, and remember the new time.
        if let Some(doc) = &stored
            && doc.content_hash == ingested.document.content_hash
        {
            self.db
                .touch_document(&doc.id, ingested.document.modified)?;
            return Ok(stored_result(doc));
        }

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
        let report = self.index_folder_report(folder, cancel_flag)?;
        Ok((report.indexed, report.skipped))
    }

    /// Adds the readable files in `folder` and its subfolders. Links to other
    /// places (symlinks, and junctions on Windows) are not followed, so the
    /// walk stays inside the folder the user chose and cannot loop. The walk
    /// stops at `MAX_FOLDER_DEPTH` levels and `MAX_FOLDER_FILES` files.
    pub fn index_folder_report(
        &self,
        folder: &Path,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<FolderReport, AppError> {
        if !folder.exists() || !folder.is_dir() {
            return Err(AppError::invalid_input(format!(
                "Directory does not exist: {}",
                folder.display()
            )));
        }

        let cancelled = || {
            cancel_flag
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
        };
        let mut report = FolderReport::default();
        let mut seen_files = 0usize;
        let mut stack = vec![(folder.to_path_buf(), 0usize)];

        'walk: while let Some((current_dir, depth)) = stack.pop() {
            if cancelled() {
                break;
            }

            let entries = match std::fs::read_dir(&current_dir) {
                Ok(entries) => entries,
                Err(_) => {
                    report.skipped += 1;
                    continue;
                }
            };

            for entry in entries.flatten() {
                if cancelled() {
                    break 'walk;
                }

                let p = entry.path();
                // Ignore rules apply inside the chosen folder only. Ancestors such as
                // `~/.notes` or `~/target/...` must not hide the whole folder.
                let inside = p.strip_prefix(folder).unwrap_or(p.as_path());
                if FileIngester::should_ignore_path(inside) {
                    continue;
                }

                // The entry's own type, without following a link.
                let Ok(file_type) = entry.file_type() else {
                    report.skipped += 1;
                    continue;
                };
                if file_type.is_symlink() {
                    continue;
                }

                if file_type.is_dir() {
                    if depth + 1 > MAX_FOLDER_DEPTH {
                        report.limited = true;
                        continue;
                    }
                    stack.push((p, depth + 1));
                } else if file_type.is_file() {
                    seen_files += 1;
                    if seen_files > MAX_FOLDER_FILES {
                        report.limited = true;
                        break 'walk;
                    }
                    if !FileIngester::is_supported_extension(&p) {
                        report.skipped += 1;
                        continue;
                    }

                    match self.index_file(&p) {
                        Ok(_) => report.indexed += 1,
                        Err(e) => {
                            log::debug!("Skipping file {}: {e}", p.display());
                            report.skipped += 1;
                        }
                    }
                }
            }
        }

        if report.limited {
            log::warn!(
                "Stopped adding {} at {MAX_FOLDER_FILES} files or {MAX_FOLDER_DEPTH} folder levels",
                folder.display()
            );
        }
        Ok(report)
    }

    /// Search the library. `query_vector` comes from the embedding engine
    /// for semantic and hybrid modes; keyword mode ignores it, and hybrid
    /// degrades to keyword ranking when no vector is available. When the
    /// reranker engine is running, the top fused candidates are re-ordered
    /// by query-document relevance; a rerank failure keeps the fused order.
    pub fn search(
        &self,
        query: &str,
        query_vector: Option<&[f32]>,
        mode: SearchMode,
        limit: usize,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        let mut results = self.searcher.search(query, query_vector, mode, limit)?;

        let engine = match self.rerank_engine() {
            Some(engine) if engine.is_running() && results.len() > 1 => engine,
            _ => return Ok(results),
        };

        // Snippets are cloned so the fused order survives a rerank failure
        // unchanged.
        let candidates: Vec<SearchResultDoc> =
            results.iter().take(RERANK_CANDIDATES).cloned().collect();
        let texts: Vec<String> = candidates.iter().map(|d| d.snippet.clone()).collect();
        match engine.rerank(query, &texts) {
            Ok(hits) => {
                results = reorder(&candidates, &hits);
                results.truncate(limit);
            }
            Err(e) => log::warn!("Rerank failed, keeping fused ranking: {e}"),
        }
        Ok(results)
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
        self.list_page(None, 0)
    }

    /// One page of the library, newest first. `limit` None returns all.
    pub fn list_page(
        &self,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<SearchResultDoc>, AppError> {
        let docs = self.db.list_documents(limit, offset)?;
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

    /// Writes pending database changes to the file. Runs when the app closes.
    pub fn checkpoint(&self) -> Result<(), AppError> {
        self.db.checkpoint()
    }

    /// Shrinks the database file. Returns its size before and after, in bytes.
    pub fn compact(&self) -> Result<(u64, u64), AppError> {
        self.db.compact()
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

    /// A removed path may be a file or a whole folder. The watcher can report
    /// only the folder, so its documents go with it.
    fn remove_file(&self, path: &str) -> Result<bool, AppError> {
        let file = self.remove(path)?;
        let inside = self.db.delete_documents_under(path)?;
        Ok(file || inside > 0)
    }
}

/// A file's change time in whole seconds, as stored for each document.
fn modified_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The search-result shape for a document that is already stored.
fn stored_result(doc: &db::DbDocument) -> SearchResultDoc {
    SearchResultDoc {
        path: doc.path.clone(),
        title: doc.title.clone(),
        snippet: String::new(),
        score: 1.0,
        modified: doc.modified,
        size: doc.size,
    }
}

#[cfg(test)]
mod tests {
    use super::LibraryService;
    use super::search::SearchMode;
    use super::watch::LibraryUpdater;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("la-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn hits(library: &LibraryService, words: &str) -> usize {
        library
            .search(words, None, SearchMode::Keyword, 10)
            .unwrap()
            .len()
    }

    #[test]
    fn replacing_and_removing_a_document_updates_search() {
        let dir = temp_dir("replace");
        let file = dir.join("plan.md");
        let library = LibraryService::open_in_memory().unwrap();

        std::fs::write(&file, "the old harbour plan").unwrap();
        library.index_file(&file).unwrap();
        assert_eq!(hits(&library, "harbour"), 1);

        // New content: the old words leave the index, the new ones arrive.
        std::fs::write(&file, "a brand new lighthouse idea, longer than before").unwrap();
        library.index_file(&file).unwrap();
        assert_eq!(hits(&library, "harbour"), 0);
        assert_eq!(hits(&library, "lighthouse"), 1);

        assert!(library.remove(&file.to_string_lossy()).unwrap());
        assert_eq!(hits(&library, "lighthouse"), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unchanged_file_is_not_read_again() {
        let dir = temp_dir("unchanged");
        let file = dir.join("note.txt");
        std::fs::write(&file, "notes about the garden").unwrap();
        let library = LibraryService::open_in_memory().unwrap();

        let first = library.index_file(&file).unwrap();
        assert!(!first.snippet.is_empty(), "a fresh index returns a snippet");
        let second = library.index_file(&file).unwrap();
        assert!(second.snippet.is_empty(), "an unchanged file is skipped");
        assert_eq!(hits(&library, "garden"), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn removing_a_folder_removes_its_documents() {
        let dir = temp_dir("folder");
        let sub = dir.join("trip");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.md"), "packing list for the mountains").unwrap();
        std::fs::write(dir.join("b.md"), "mountains are outside the folder").unwrap();
        let library = LibraryService::open_in_memory().unwrap();
        let report = library.index_folder_report(&dir, None).unwrap();
        assert_eq!(report.indexed, 2);
        assert!(!report.limited);

        assert!(library.remove_file(&sub.to_string_lossy()).unwrap());
        assert_eq!(hits(&library, "packing"), 0);
        assert_eq!(hits(&library, "outside"), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
