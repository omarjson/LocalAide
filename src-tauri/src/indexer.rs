use serde::{Deserialize, Serialize};
use sled::{Batch, Db};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::document::Value as DocValue;
use tantivy::schema::{Field, STORED, STRING, Schema, TEXT};
use tantivy::{Index, IndexReader, ReloadPolicy, doc};

use crate::index_dir;

/// Hard ceiling on how much file content we pull into memory when indexing.
/// Prevents a single multi-GB file from exhausting RAM during a folder scan.
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;

/// Content stored in the index is truncated to this length (keeps sled small).
const MAX_STORED_CHARS: usize = 8_000;

/// A single indexed document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDoc {
    pub path: String,
    pub title: String,
    pub content: String,
    pub kind: DocKind,
    pub size: u64,
    pub modified: u64, // epoch seconds
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Text,
    Markdown,
    Image,
    Other,
}

impl DocKind {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") => DocKind::Markdown,
            Some("txt") | Some("text") => DocKind::Text,
            Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("gif") | Some("svg") => {
                DocKind::Image
            }
            _ => DocKind::Other,
        }
    }

    /// File extensions we know how to extract text (or embeddings) from.
    fn is_indexable(path: &Path) -> bool {
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") | Some("markdown") | Some("txt") | Some("text") => true,
            Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("gif") => true,
            _ => false,
        }
    }
}

/// Search result with score and which strategy produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub doc: IndexedDoc,
    pub score: f32,
    pub source: SearchSource,
    /// True when the doc was curated (memory note) rather than auto-indexed.
    #[serde(default)]
    pub curated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSource {
    Semantic,
    Keyword,
    Hybrid,
}

/// Query mode for search.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Semantic,
    Keyword,
    Hybrid,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// Progress event emitted while indexing a folder.
#[derive(Debug, Clone, Serialize)]
pub struct IndexProgress {
    pub path: String,
    pub indexed: u64,
    pub skipped: u64,
}

/// Deterministic text embedding (integration point: replace with candle + EmbeddingGemma).
///
/// Produces a unit "bag-of-words" vector so cosine similarity is a dot product.
/// Stable across runs for the same text, which is what incremental indexing needs.
pub fn embed_text(text: &str) -> Vec<f32> {
    let engine = crate::embeddings::EmbeddingEngine::shared();
    engine.embed_text(text)
}

/// Cosine similarity between two vectors of equal dimension.
///
/// Returns 0.0 for mismatched dimensions instead of silently truncating —
/// a silent truncate would return meaningless scores.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0f32;
    for i in 0..a.len() {
        sum += a[i] * b[i];
    }
    // Assumes both are already unit vectors (the engine guarantees this); fall
    // back to a true cosine when a zero vector slips through.
    if sum.abs() > 1.0 {
        let ma: f32 = a.iter().map(|v| v * v).sum();
        let mb: f32 = b.iter().map(|v| v * v).sum();
        if ma > 0.0 && mb > 0.0 {
            return sum / (ma.sqrt() * mb.sqrt());
        }
        return 0.0;
    }
    sum
}

/// Read at most `MAX_INDEX_BYTES` of a file as a lossy UTF-8 string.
///
/// Large files are truncated rather than loaded whole, so a 10 GB text file
/// costs 2 MB of RAM instead of OOMing the process.
fn read_text(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > MAX_INDEX_BYTES {
        let mut f = fs::File::open(path).ok()?;
        let mut buf = vec![0u8; MAX_INDEX_BYTES as usize];
        let mut read = 0usize;
        while read < buf.len() {
            let n = f.read(&mut buf[read..]).ok()?;
            if n == 0 {
                break;
            }
            read += n;
        }
        buf.truncate(read);
        return Some(String::from_utf8_lossy(&buf).into_owned());
    }
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Truncate stored content on a char boundary so we never split a UTF-8 codepoint.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// In-memory + persisted index state.
struct IndexInner {
    sled: Db,
    tantivy: Index,
    schema: Schema,
    path_field: Field,
    body_field: Field,
    title_field: Field,
    writer: Mutex<tantivy::IndexWriter>,
    reader: IndexReader,
    doc_count: std::sync::atomic::AtomicU64,
}

impl IndexInner {
    fn open(index_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(index_dir).map_err(|e| e.to_string())?;
        let sled_path = index_dir.join("store.sled");
        let sled = sled::open(&sled_path).map_err(|e| e.to_string())?;

        let mut schema_builder = Schema::builder();
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let body_field = schema_builder.add_text_field("body", TEXT);
        let path_field = schema_builder.add_text_field("path", STRING | STORED);
        let schema = schema_builder.build();

        let dir = tantivy::directory::MmapDirectory::open(index_dir)
            .map_err(|e| format!("tantivy mmap dir: {e}"))?;
        let tantivy = Index::open_or_create(dir, schema.clone())
            .map_err(|e| format!("tantivy open_or_create: {e}"))?;
        let writer = tantivy
            .writer(50_000_000)
            .map_err(|e| format!("tantivy writer: {e}"))?;
        // Manual reload: we call `reload()` right after every commit, which
        // guarantees a search immediately after a write observes the new doc.
        let reader = tantivy
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| format!("tantivy reader: {e}"))?;

        // Recover the doc count instead of counting on every `stats()` call.
        let count = sled.len() as u64;

        Ok(Self {
            sled,
            tantivy,
            schema,
            path_field,
            body_field,
            title_field,
            writer: Mutex::new(writer),
            reader,
            doc_count: std::sync::atomic::AtomicU64::new(count),
        })
    }

    /// Add documents in a batch: one tantivy commit for the whole group.
    ///
    /// The previous implementation committed (which fsyncs a segment) after
    /// every single document, making a 10k-file folder crawl glacial.
    fn add_docs(&self, docs: Vec<IndexedDoc>) -> Result<(), String> {
        if docs.is_empty() {
            return Ok(());
        }
        {
            let mut writer = self
                .writer
                .lock()
                .map_err(|e| format!("writer mutex poisoned: {e}"))?;
            for doc in &docs {
                writer
                    .add_document(doc!(
                        self.title_field => doc.title.clone(),
                        self.body_field => doc.content.clone(),
                        self.path_field => doc.path.clone(),
                    ))
                    .map_err(|e| format!("tantivy add: {e}"))?;
            }
            writer
                .commit()
                .map_err(|e| format!("tantivy commit: {e}"))?;
            drop(writer);
            // Make the new segment visible to search immediately.
            let _ = self.reader.reload();
        }

        let mut batch = Batch::default();
        for doc in &docs {
            let value = serde_json::to_string(doc).map_err(|e| e.to_string())?;
            batch.insert(doc.path.as_bytes(), value.as_bytes());
        }
        self.sled
            .apply_batch(batch)
            .map_err(|e| format!("sled batch: {e}"))?;
        self.doc_count
            .fetch_add(docs.len() as u64, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Add a single document (used by the file watcher and memory writes).
    fn add_doc(&self, doc: IndexedDoc) -> Result<(), String> {
        self.add_docs(vec![doc])
    }

    fn get_doc(&self, path: &str) -> Option<IndexedDoc> {
        let raw = self.sled.get(path.as_bytes()).ok().flatten()?;
        serde_json::from_slice(&raw).ok()
    }

    fn remove_doc(&self, path: &str) -> Result<(), String> {
        let existed = self
            .sled
            .remove(path.as_bytes())
            .map_err(|e| e.to_string())?
            .is_some();
        if existed {
            self.doc_count
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
        {
            let mut writer = self
                .writer
                .lock()
                .map_err(|e| format!("writer mutex poisoned: {e}"))?;
            let term = tantivy::Term::from_field_text(self.path_field, path);
            writer.delete_term(term);
            writer
                .commit()
                .map_err(|e| format!("tantivy delete: {e}"))?;
            drop(writer);
            let _ = self.reader.reload();
        }
        Ok(())
    }

    /// BM25 keyword search over the tantivy index.
    ///
    /// The old implementation deserialized every document and did a substring
    /// match. This uses the inverted index, so cost is proportional to the
    /// number of matching documents rather than the corpus size.
    fn keyword_search(&self, query: &str, limit: usize) -> Result<Vec<(IndexedDoc, f32)>, String> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        // Escape query-grammar characters so ordinary text is treated as a
        // phrase instead of raising a parse error.
        let escaped = query
            .chars()
            .flat_map(|c| match c {
                '+' | '-' | '<' | '>' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '"'
                | '~' | '*' | '?' | ':' | '\\' | '/' => vec!['\\', c],
                _ => vec![c],
            })
            .collect::<String>();

        let query_parser =
            QueryParser::for_index(&self.tantivy, vec![self.title_field, self.body_field]);
        let query = match query_parser.parse_query(&escaped) {
            Ok(q) => q,
            Err(_) => {
                // Fall back to the raw text (a bare term never fails to parse).
                let token = query
                    .trim()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if token.is_empty() {
                    return Ok(vec![]);
                }
                query_parser
                    .parse_query(&token)
                    .map_err(|e| format!("query parse: {e}"))?
            }
        };

        let searcher = self.reader.searcher();
        let collector = TopDocs::with_limit(limit).order_by_score();
        let hits = searcher
            .search(query.as_ref(), &collector)
            .map_err(|e| format!("tantivy search: {e}"))?;

        let mut out = Vec::with_capacity(hits.len());
        for (score, doc_address) in hits {
            let retrieved: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| format!("tantivy doc: {e}"))?;
            let path = retrieved
                .get_first(self.path_field)
                .and_then(|v| {
                    DocValue::as_value(&v)
                        .as_leaf()
                        .and_then(|leaf| leaf.as_str())
                })
                .unwrap_or_default()
                .to_string();
            if path.is_empty() {
                continue;
            }
            if let Some(stored) = self.get_doc(&path) {
                out.push((stored, score));
            }
        }
        Ok(out)
    }

    fn list_docs(&self) -> Vec<IndexedDoc> {
        let mut out =
            Vec::with_capacity(self.doc_count.load(std::sync::atomic::Ordering::Relaxed) as usize);
        for result in self.sled.scan_prefix(b"") {
            if let Ok((_key, value)) = result {
                if let Ok(doc) = serde_json::from_slice::<IndexedDoc>(&value) {
                    out.push(doc);
                }
            }
        }
        out
    }

    /// Only path + score-relevant fields for semantic ranking.
    ///
    /// Avoids cloning full titles and contents when we just need embeddings.
    fn list_doc_paths_and_embeddings(&self) -> Vec<(String, Vec<f32>)> {
        let mut out = Vec::new();
        for result in self.sled.scan_prefix(b"") {
            if let Ok((key, value)) = result {
                if let Ok(doc) = serde_json::from_slice::<IndexedDoc>(&value) {
                    out.push((String::from_utf8_lossy(&key).into_owned(), doc.embedding));
                }
            }
        }
        out
    }

    fn count(&self) -> u64 {
        self.doc_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Public indexer handle shared across Tauri commands.
pub struct Indexer {
    inner: Arc<IndexInner>,
}

impl Indexer {
    pub fn open() -> Result<Self, String> {
        let dir = index_dir();
        Self::open_at(&dir)
    }

    pub fn open_at(dir: &Path) -> Result<Self, String> {
        let inner = Arc::new(IndexInner::open(dir)?);
        Ok(Self { inner })
    }

    /// Index a single file. Returns the doc.
    pub fn index_file(&self, path: &str) -> Result<IndexedDoc, String> {
        let p = Path::new(path);
        if !p.exists() || !p.is_file() {
            return Err(format!("File not found or not a regular file: {path}"));
        }
        let mut docs = self.index_files(&[path.to_string()])?;
        if docs.is_empty() {
            return Err(format!(
                "File could not be indexed (unsupported format or empty): {path}"
            ));
        }
        Ok(docs.remove(0))
    }

    /// Index several files in one commit (used for folder crawls and the watcher).
    pub fn index_files(&self, paths: &[String]) -> Result<Vec<IndexedDoc>, String> {
        let engine = crate::embeddings::EmbeddingEngine::shared();
        let dim = engine.dim();
        let mut docs = Vec::with_capacity(paths.len());

        for path in paths {
            let p = Path::new(path);
            if !p.exists() || !p.is_file() {
                log::warn!("skip missing file: {path}");
                continue;
            }
            let metadata = fs::metadata(p).map_err(|e| e.to_string())?;
            let size = metadata.len();
            let modified = metadata
                .modified()
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                })
                .unwrap_or(0);
            let title = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string();
            let kind = DocKind::from_path(p);

            let (content, embedding) = if matches!(kind, DocKind::Image) {
                let content = format!("[image] {}", title);
                (content, engine.embed_image(path))
            } else {
                let raw = read_text(p).unwrap_or_default();
                if raw.is_empty() {
                    (raw, vec![0.0f32; dim])
                } else {
                    let emb = engine.embed_text(&raw);
                    (raw, emb)
                }
            };

            let doc = IndexedDoc {
                path: path.to_string(),
                title,
                content: truncate_chars(&content, MAX_STORED_CHARS),
                kind,
                size,
                modified,
                embedding,
            };
            docs.push(doc);
        }

        self.inner.add_docs(docs.clone())?;
        Ok(docs)
    }

    /// Recursively index every applicable file under a folder.
    ///
    /// Files already indexed with the same mtime and size are skipped, so a
    /// rescan after touching one file costs one document, not the whole tree.
    pub fn index_folder(&self, folder: &str) -> Result<(u64, u64), String> {
        let root = Path::new(folder);
        if !root.exists() || !root.is_dir() {
            return Err(format!("not a folder: {folder}"));
        }

        // Collect first, then batch-commit. Keeps memory bounded for huge trees.
        const CHUNK: usize = 500;
        let mut indexed = 0u64;
        let mut skipped = 0u64;
        let mut batch: Vec<String> = Vec::with_capacity(CHUNK);

        let mut dirs: Vec<PathBuf> = vec![root.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("skip dir {}: {e}", dir.display());
                    skipped += 1;
                    continue;
                }
            };
            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => {
                        skipped += 1;
                        continue;
                    }
                };
                let path = entry.path();
                let file_type = match entry.file_type() {
                    Ok(t) => t,
                    Err(_) => {
                        skipped += 1;
                        continue;
                    }
                };
                if file_type.is_dir() {
                    dirs.push(path);
                } else if file_type.is_file() && DocKind::is_indexable(&path) {
                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => {
                            skipped += 1;
                            continue;
                        }
                    };
                    let size = metadata.len();
                    let modified = metadata
                        .modified()
                        .map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                        })
                        .unwrap_or(0);
                    let path_str = path.to_string_lossy().to_string();

                    if let Some(existing) = self.inner.get_doc(&path_str) {
                        if existing.size == size && existing.modified == modified {
                            skipped += 1;
                            continue;
                        }
                    }
                    batch.push(path_str);
                    if batch.len() >= CHUNK {
                        match self.inner.add_docs(self.load_docs(&batch)) {
                            Ok(()) => indexed += batch.len() as u64,
                            Err(e) => {
                                log::warn!("index chunk failed: {e}");
                                skipped += batch.len() as u64;
                            }
                        }
                        batch.clear();
                    }
                }
            }
        }
        if !batch.is_empty() {
            match self.inner.add_docs(self.load_docs(&batch)) {
                Ok(()) => indexed += batch.len() as u64,
                Err(e) => {
                    log::warn!("index chunk failed: {e}");
                    skipped += batch.len() as u64;
                }
            }
        }

        Ok((indexed, skipped))
    }

    fn load_docs(&self, paths: &[String]) -> Vec<IndexedDoc> {
        let engine = crate::embeddings::EmbeddingEngine::shared();
        let dim = engine.dim();
        let mut docs = Vec::with_capacity(paths.len());
        for path in paths {
            let p = Path::new(path);
            let title = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string();
            let kind = DocKind::from_path(p);
            let (content, embedding) = if matches!(kind, DocKind::Image) {
                (format!("[image] {title}"), engine.embed_image(path))
            } else {
                let raw = read_text(p).unwrap_or_default();
                if raw.is_empty() {
                    (raw, vec![0.0f32; dim])
                } else {
                    let emb = engine.embed_text(&raw);
                    (raw, emb)
                }
            };
            let (size, modified) = fs::metadata(p)
                .map(|m| {
                    (
                        m.len(),
                        m.modified()
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                            })
                            .unwrap_or(0),
                    )
                })
                .unwrap_or((0, 0));
            docs.push(IndexedDoc {
                path: path.to_string(),
                title,
                content: truncate_chars(&content, MAX_STORED_CHARS),
                kind,
                size,
                modified,
                embedding,
            });
        }
        docs
    }

    pub fn remove(&self, path: &str) -> Result<(), String> {
        self.inner.remove_doc(path)
    }

    pub fn search(
        &self,
        query: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<Vec<SearchResult>, String> {
        if limit == 0 {
            return Ok(vec![]);
        }

        let mut results: HashMap<String, SearchResult> = HashMap::new();
        let query_embedding = embed_text(query);

        // Semantic: cosine against stored embeddings.
        // We keep paths + embeddings only, then hydrate full docs once at the end.
        if matches!(mode, SearchMode::Semantic | SearchMode::Hybrid) && !query_embedding.is_empty()
        {
            let semantic_threshold = 0.05f32;
            let mut best: HashMap<String, f32> = HashMap::new();
            for (path, embedding) in self.inner.list_doc_paths_and_embeddings() {
                let score = cosine_similarity(&query_embedding, &embedding);
                if score > semantic_threshold {
                    best.insert(path, score);
                }
            }
            // Hydrate the survivors in one pass.
            for (path, score) in best {
                if let Some(doc) = self.inner.get_doc(&path) {
                    results.insert(
                        doc.path.clone(),
                        SearchResult {
                            doc,
                            score,
                            source: SearchSource::Semantic,
                            curated: false,
                        },
                    );
                }
            }
        }

        // Keyword: BM25 over the tantivy inverted index.
        if matches!(mode, SearchMode::Keyword | SearchMode::Hybrid) {
            for (doc, score) in self.inner.keyword_search(query, limit.max(20))? {
                let norm_score = if score > 0.0 {
                    0.5 + score.min(3.0) / 6.0
                } else {
                    0.0
                };
                match results.get_mut(&doc.path) {
                    Some(existing) => {
                        // Reciprocal-rank-ish blend so neither strand dominates.
                        existing.score = existing.score * 0.5 + norm_score * 0.5;
                        existing.source = SearchSource::Hybrid;
                    }
                    None => {
                        results.insert(
                            doc.path.clone(),
                            SearchResult {
                                doc,
                                score: norm_score,
                                source: SearchSource::Keyword,
                                curated: false,
                            },
                        );
                    }
                }
            }
        }

        let mut out: Vec<SearchResult> = results.into_values().collect();
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        Ok(out)
    }

    pub fn list(&self) -> Vec<IndexedDoc> {
        self.inner.list_docs()
    }

    pub fn count(&self) -> u64 {
        self.inner.count()
    }

    pub fn stats(&self) -> IndexerStats {
        let total_bytes: usize = self
            .inner
            .sled
            .iter()
            .filter_map(|r| r.ok())
            .map(|(_, v)| v.len())
            .sum();
        IndexerStats {
            doc_count: self.inner.count(),
            total_bytes: total_bytes as u64,
            storage_path: index_dir().to_string_lossy().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexerStats {
    pub doc_count: u64,
    pub total_bytes: u64,
    pub storage_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consecutive_index_opens() {
        let temp_dir = std::env::temp_dir().join(format!(
            "localaide_test_idx_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&temp_dir);

        // First launch: initialize index
        {
            let indexer = Indexer::open_at(&temp_dir).expect("First indexer open should succeed");
            let file_path = temp_dir.join("test.txt");
            fs::write(&file_path, "Hello LocalAide consecutive launch test").unwrap();
            let doc = indexer
                .index_file(file_path.to_str().unwrap())
                .expect("Indexing should succeed");
            assert_eq!(doc.title, "test.txt");
            assert_eq!(indexer.count(), 1);
        }

        // Second launch: must open existing index without panicking (Index::open_or_create)
        {
            let indexer = Indexer::open_at(&temp_dir)
                .expect("Second indexer open must succeed on existing directory");
            assert_eq!(indexer.count(), 1);
        }

        // Third launch: must open existing index cleanly
        {
            let indexer = Indexer::open_at(&temp_dir).expect("Third indexer open must succeed");
            assert_eq!(indexer.count(), 1);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_index_file_missing_returns_err_instead_of_panic() {
        let temp_dir = std::env::temp_dir().join(format!(
            "localaide_test_missing_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&temp_dir);
        let indexer = Indexer::open_at(&temp_dir).expect("Indexer should initialize");

        let missing_path = temp_dir.join("does_not_exist_file.txt");
        let result = indexer.index_file(missing_path.to_str().unwrap());
        assert!(result.is_err(), "Missing file should return Err, not panic");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
