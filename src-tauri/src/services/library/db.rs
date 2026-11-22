use crate::app::error::AppError;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct LibraryStats {
    pub doc_count: u64,
    pub chunk_count: u64,
    pub total_bytes: u64,
    pub storage_path: String,
}

#[derive(Debug, Clone)]
pub struct DbDocument {
    pub id: String,
    pub path: String,
    pub title: String,
    pub content_hash: String,
    pub size: u64,
    pub modified: u64,
    pub indexed_at: u64,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct DbNote {
    pub id: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub size: u64,
    pub modified: u64,
    pub created_at: u64,
    /// Kept in every chat's context while recall is on.
    pub pinned: bool,
}

#[derive(Debug, Clone)]
pub struct DbToolAudit {
    pub id: i64,
    pub timestamp: i64,
    pub tool_name: String,
    pub action: String,
    pub path: String,
    pub approved: bool,
    pub status: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DbChunk {
    pub id: String,
    pub document_id: String,
    pub chunk_index: usize,
    pub text: String,
    pub token_count: usize,
    pub start_char: usize,
    pub end_char: usize,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct FtsHit {
    pub chunk_id: String,
    pub document_id: String,
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
    pub modified: u64,
    pub size: u64,
}

/// What a search result shows for one chunk.
#[derive(Debug, Clone)]
pub struct ChunkDetail {
    pub chunk_id: String,
    pub path: String,
    pub title: String,
    pub text: String,
    pub modified: u64,
    pub size: u64,
}

/// The newest schema version. See `migrate`.
const SCHEMA_VERSION: i64 = 3;

/// The activity log keeps this many of the newest rows.
const MAX_AUDIT_ROWS: i64 = 10_000;

pub struct LibraryDb {
    conn: Arc<Mutex<Connection>>,
    db_path_str: String,
}

impl LibraryDb {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let conn = Connection::open(path).map_err(|e| {
            AppError::storage(format!(
                "Failed to open SQLite database at {}: {e}",
                path.display()
            ))
        })?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path_str: path.to_string_lossy().to_string(),
        };
        db.configure_and_migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, AppError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::storage(format!("Failed to open in-memory SQLite: {e}")))?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path_str: ":memory:".to_string(),
        };
        db.configure_and_migrate()?;
        Ok(db)
    }

    fn configure_and_migrate(&self) -> Result<(), AppError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;

        // WAL mode, busy timeout, foreign keys
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| AppError::storage(format!("Failed to configure SQLite pragmas: {e}")))?;

        // Schema migrations
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                indexed_at INTEGER NOT NULL,
                kind TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                chunk_index INTEGER NOT NULL,
                text TEXT NOT NULL,
                token_count INTEGER NOT NULL,
                start_char INTEGER NOT NULL,
                end_char INTEGER NOT NULL,
                embedding BLOB
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_document_id ON chunks(document_id);

            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                path TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                tags TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_notes_modified ON notes(modified DESC);

            CREATE TABLE IF NOT EXISTS tool_audit (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                tool_name TEXT NOT NULL,
                action TEXT NOT NULL,
                path TEXT NOT NULL,
                approved INTEGER NOT NULL,
                status TEXT NOT NULL,
                details TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_tool_audit_ts ON tool_audit(timestamp DESC);

            CREATE TABLE IF NOT EXISTS cloud_audit (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                provider TEXT NOT NULL,
                host TEXT NOT NULL,
                model TEXT NOT NULL,
                kind TEXT NOT NULL,
                preview TEXT NOT NULL,
                bytes INTEGER NOT NULL,
                status TEXT NOT NULL,
                detail TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_cloud_audit_ts ON cloud_audit(timestamp DESC);

            CREATE TABLE IF NOT EXISTS library_folders (
                path TEXT PRIMARY KEY,
                added_at INTEGER NOT NULL,
                last_scan_at INTEGER NOT NULL,
                indexed INTEGER NOT NULL,
                skipped INTEGER NOT NULL,
                limited INTEGER NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                chunk_id UNINDEXED,
                document_id UNINDEXED,
                title,
                text,
                tokenize = 'porter unicode61'
            );",
        )
        .map_err(|e| AppError::storage(format!("Failed to apply SQLite schema migrations: {e}")))?;

        Self::migrate(&mut conn)?;

        // The activity log keeps the newest rows only.
        conn.execute(
            "DELETE FROM tool_audit WHERE id <= (
                 SELECT id FROM tool_audit ORDER BY id DESC LIMIT 1 OFFSET ?1
             )",
            params![MAX_AUDIT_ROWS],
        )
        .map_err(|e| AppError::storage(format!("Failed to trim the activity log: {e}")))?;
        conn.execute(
            "DELETE FROM cloud_audit WHERE id <= (
                 SELECT id FROM cloud_audit ORDER BY id DESC LIMIT 1 OFFSET ?1
             )",
            params![MAX_AUDIT_ROWS],
        )
        .map_err(|e| AppError::storage(format!("Failed to trim the online log: {e}")))?;

        Ok(())
    }

    /// Versioned schema changes, each applied once.
    ///
    /// Version 2 gives each chunk the rowid of its full-text row
    /// (`chunks.fts_rowid`). `chunks_fts.document_id` is not indexed, so
    /// deleting by it read the whole full-text table each time. Deletes now go
    /// by rowid. The full-text rows are rebuilt once with known rowids.
    fn migrate(conn: &mut Connection) -> Result<(), AppError> {
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|e| AppError::storage(format!("Failed to read the schema version: {e}")))?;
        if version >= SCHEMA_VERSION {
            return Ok(());
        }

        let tx = conn
            .transaction()
            .map_err(|e| AppError::storage(format!("Failed to start the migration: {e}")))?;
        if version < 3 {
            let has_pinned = tx
                .prepare("SELECT 1 FROM pragma_table_info('notes') WHERE name = 'pinned'")
                .and_then(|mut stmt| stmt.exists([]))
                .map_err(|e| {
                    AppError::storage(format!("Failed to inspect the notes table: {e}"))
                })?;
            if !has_pinned {
                tx.execute_batch("ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;")
                    .map_err(|e| AppError::storage(format!("Failed to add notes.pinned: {e}")))?;
            }
        }
        if version < 2 {
            let has_column = tx
                .prepare("SELECT 1 FROM pragma_table_info('chunks') WHERE name = 'fts_rowid'")
                .and_then(|mut stmt| stmt.exists([]))
                .map_err(|e| {
                    AppError::storage(format!("Failed to inspect the chunks table: {e}"))
                })?;
            if !has_column {
                tx.execute_batch("ALTER TABLE chunks ADD COLUMN fts_rowid INTEGER;")
                    .map_err(|e| {
                        AppError::storage(format!("Failed to add chunks.fts_rowid: {e}"))
                    })?;
            }
            tx.execute_batch(
                "DELETE FROM chunks_fts;
                 UPDATE chunks SET fts_rowid = rowid;
                 INSERT INTO chunks_fts (rowid, chunk_id, document_id, title, text)
                     SELECT c.fts_rowid, c.id, c.document_id, d.title, c.text
                     FROM chunks c JOIN documents d ON d.id = c.document_id;",
            )
            .map_err(|e| AppError::storage(format!("Failed to rebuild the search index: {e}")))?;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        tx.execute(
            "INSERT OR REPLACE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![SCHEMA_VERSION, now],
        )
        .map_err(|e| AppError::storage(format!("Failed to record the schema version: {e}")))?;
        tx.commit()
            .map_err(|e| AppError::storage(format!("Failed to commit the migration: {e}")))
    }

    /// Removes one document, its chunks and its full-text rows.
    fn delete_document_rows(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<(), AppError> {
        tx.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (
                 SELECT fts_rowid FROM chunks WHERE document_id = ?1 AND fts_rowid IS NOT NULL
             )",
            params![id],
        )
        .map_err(|e| AppError::storage(format!("Failed to delete search entries: {e}")))?;
        tx.execute("DELETE FROM documents WHERE id = ?1", params![id])
            .map_err(|e| AppError::storage(format!("Failed to delete document: {e}")))?;
        Ok(())
    }

    /// Writes the log into the database file and refreshes query statistics.
    /// Runs when the app closes.
    pub fn checkpoint(&self) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute_batch("PRAGMA optimize; PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| AppError::storage(format!("Failed to checkpoint the database: {e}")))
    }

    /// Rebuilds the database file without free pages. Returns the file size
    /// before and after, in bytes (0 for an in-memory database).
    pub fn compact(&self) -> Result<(u64, u64), AppError> {
        let size = || {
            std::fs::metadata(&self.db_path_str)
                .map(|m| m.len())
                .unwrap_or(0)
        };
        let before = size();
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute_batch(
            "PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .map_err(|e| AppError::storage(format!("Failed to compact the database: {e}")))?;
        drop(conn);
        Ok((before, size()))
    }

    pub fn insert_or_replace_document(
        &self,
        doc: &DbDocument,
        chunks: &[DbChunk],
    ) -> Result<(), AppError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let tx = conn
            .transaction()
            .map_err(|e| AppError::storage(format!("Failed to start transaction: {e}")))?;

        // Remove the old version of this document first.
        if let Some(existing_id) = tx
            .query_row(
                "SELECT id FROM documents WHERE path = ?1",
                params![doc.path],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| AppError::storage(format!("Failed to check existing document: {e}")))?
        {
            Self::delete_document_rows(&tx, &existing_id)?;
        }

        // Insert document
        tx.execute(
            "INSERT INTO documents (id, path, title, content_hash, size, modified, indexed_at, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                doc.id,
                doc.path,
                doc.title,
                doc.content_hash,
                doc.size as i64,
                doc.modified as i64,
                doc.indexed_at as i64,
                doc.kind,
            ],
        ).map_err(|e| AppError::storage(format!("Failed to insert document: {e}")))?;

        // Insert chunks & FTS entries. Each chunk keeps its full-text rowid, so
        // a later delete finds the row directly.
        for chunk in chunks {
            tx.execute(
                "INSERT INTO chunks_fts (chunk_id, document_id, title, text)
                 VALUES (?1, ?2, ?3, ?4)",
                params![chunk.id, chunk.document_id, doc.title, chunk.text,],
            )
            .map_err(|e| AppError::storage(format!("Failed to insert FTS entry: {e}")))?;
            let fts_rowid = tx.last_insert_rowid();

            let blob = chunk
                .embedding
                .as_ref()
                .map(|vec| Self::embedding_to_blob(vec));
            tx.execute(
                "INSERT INTO chunks (id, document_id, chunk_index, text, token_count, start_char, end_char, embedding, fts_rowid)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    chunk.id,
                    chunk.document_id,
                    chunk.chunk_index as i64,
                    chunk.text,
                    chunk.token_count as i64,
                    chunk.start_char as i64,
                    chunk.end_char as i64,
                    blob,
                    fts_rowid,
                ],
            ).map_err(|e| AppError::storage(format!("Failed to insert chunk: {e}")))?;
        }

        tx.commit().map_err(|e| {
            AppError::storage(format!("Failed to commit document transaction: {e}"))
        })?;

        Ok(())
    }

    pub fn delete_document_by_path(&self, path: &str) -> Result<bool, AppError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let tx = conn
            .transaction()
            .map_err(|e| AppError::storage(format!("Failed to start transaction: {e}")))?;

        let doc_id: Option<String> = tx
            .query_row(
                "SELECT id FROM documents WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::storage(format!("Failed to query document: {e}")))?;

        if let Some(id) = doc_id {
            Self::delete_document_rows(&tx, &id)?;
            tx.commit()
                .map_err(|e| AppError::storage(format!("Failed to commit delete: {e}")))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Removes every document inside `folder`. Used when a watched folder is
    /// deleted, since the watcher may report only the folder itself. Returns
    /// how many documents were removed.
    pub fn delete_documents_under(&self, folder: &str) -> Result<usize, AppError> {
        let trimmed = folder.trim_end_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Ok(0);
        }
        // Paths are stored as the system gave them, so try both separators.
        let prefixes = [format!("{trimmed}/"), format!("{trimmed}\\")];
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let tx = conn
            .transaction()
            .map_err(|e| AppError::storage(format!("Failed to start transaction: {e}")))?;
        let mut ids = Vec::new();
        {
            let mut stmt = tx
                .prepare("SELECT id FROM documents WHERE substr(path, 1, length(?1)) = ?1")
                .map_err(|e| AppError::storage(format!("Failed to prepare folder query: {e}")))?;
            for prefix in &prefixes {
                let rows = stmt
                    .query_map(params![prefix], |row| row.get::<_, String>(0))
                    .map_err(|e| AppError::storage(format!("Failed to query folder: {e}")))?;
                for row in rows {
                    ids.push(row.map_err(|e| AppError::storage(format!("Row read error: {e}")))?);
                }
            }
        }
        for id in &ids {
            Self::delete_document_rows(&tx, id)?;
        }
        tx.commit()
            .map_err(|e| AppError::storage(format!("Failed to commit delete: {e}")))?;
        Ok(ids.len())
    }

    /// Records a new change time for a document whose content did not change,
    /// so the next scan skips it without reading the file.
    pub fn touch_document(&self, id: &str, modified: u64) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute(
            "UPDATE documents SET modified = ?1 WHERE id = ?2",
            params![modified as i64, id],
        )
        .map_err(|e| AppError::storage(format!("Failed to update document: {e}")))?;
        Ok(())
    }

    pub fn get_document_by_path(&self, path: &str) -> Result<Option<DbDocument>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let res = conn
            .query_row(
                "SELECT id, path, title, content_hash, size, modified, indexed_at, kind
             FROM documents WHERE path = ?1",
                params![path],
                |row| {
                    Ok(DbDocument {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        title: row.get(2)?,
                        content_hash: row.get(3)?,
                        size: row.get::<_, i64>(4)? as u64,
                        modified: row.get::<_, i64>(5)? as u64,
                        indexed_at: row.get::<_, i64>(6)? as u64,
                        kind: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(|e| AppError::storage(format!("Failed to fetch document: {e}")))?;

        Ok(res)
    }

    /// Documents, newest first. `limit` None returns all of them.
    pub fn list_documents(
        &self,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<DbDocument>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, path, title, content_hash, size, modified, indexed_at, kind
             FROM documents ORDER BY indexed_at DESC, id LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare list statement: {e}")))?;

        // SQLite reads a negative limit as "no limit".
        let limit = limit.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![limit, offset], |row| {
                Ok(DbDocument {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    content_hash: row.get(3)?,
                    size: row.get::<_, i64>(4)? as u64,
                    modified: row.get::<_, i64>(5)? as u64,
                    indexed_at: row.get::<_, i64>(6)? as u64,
                    kind: row.get(7)?,
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to query documents: {e}")))?;

        let mut docs = Vec::new();
        for r in rows {
            docs.push(r.map_err(|e| AppError::storage(format!("Row read error: {e}")))?);
        }
        Ok(docs)
    }

    pub fn get_stats(&self) -> Result<LibraryStats, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let doc_count: i64 = conn
            .query_row("SELECT count(*) FROM documents", [], |r| r.get(0))
            .unwrap_or(0);
        let chunk_count: i64 = conn
            .query_row("SELECT count(*) FROM chunks", [], |r| r.get(0))
            .unwrap_or(0);
        let total_bytes: i64 = conn
            .query_row("SELECT coalesce(sum(size), 0) FROM documents", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);

        Ok(LibraryStats {
            doc_count: doc_count.max(0) as u64,
            chunk_count: chunk_count.max(0) as u64,
            total_bytes: total_bytes.max(0) as u64,
            storage_path: self.db_path_str.clone(),
        })
    }

    pub fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;

        // Escape FTS query or build tokenized query to prevent syntax crashes
        let sanitized = Self::sanitize_fts_query(query);
        if sanitized.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = conn
            .prepare(
                "SELECT 
                c.chunk_id, 
                c.document_id, 
                d.path, 
                d.title, 
                snippet(chunks_fts, 3, '<mark>', '</mark>', '...', 20) AS snip,
                bm25(chunks_fts) AS bm25_score,
                d.modified,
                d.size
             FROM chunks_fts c
             JOIN documents d ON d.id = c.document_id
             WHERE chunks_fts MATCH ?1
             ORDER BY bm25_score ASC
             LIMIT ?2",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare FTS search query: {e}")))?;

        let rows = stmt
            .query_map(params![sanitized, limit as i64], |row| {
                let raw_bm25: f64 = row.get(5)?;
                // SQLite FTS5 bm25 returns negative scores where more negative = higher relevance
                let norm_score = (-raw_bm25).max(0.0) as f32;
                Ok(FtsHit {
                    chunk_id: row.get(0)?,
                    document_id: row.get(1)?,
                    path: row.get(2)?,
                    title: row.get(3)?,
                    snippet: row.get(4)?,
                    score: norm_score,
                    modified: row.get::<_, i64>(6)? as u64,
                    size: row.get::<_, i64>(7)? as u64,
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to execute FTS query: {e}")))?;

        let mut hits = Vec::new();
        for r in rows {
            hits.push(r.map_err(|e| AppError::storage(format!("FTS row read error: {e}")))?);
        }
        Ok(hits)
    }

    /// Calls `visit` with each stored chunk vector. Only ids and vectors are
    /// read, so a search does not copy every chunk's text. A single scratch
    /// buffer is reused for every row instead of allocating a fresh `Vec<f32>`
    /// and `Vec<u8>` per embedding.
    pub fn visit_chunk_embeddings(
        &self,
        mut visit: impl FnMut(&str, &[f32]),
    ) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare("SELECT id, embedding FROM chunks WHERE embedding IS NOT NULL")
            .map_err(|e| {
                AppError::storage(format!("Failed to prepare embeddings statement: {e}"))
            })?;
        let mut rows = stmt
            .query([])
            .map_err(|e| AppError::storage(format!("Failed to query embeddings: {e}")))?;
        let mut scratch: Vec<f32> = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| AppError::storage(format!("Embedding row read error: {e}")))?
        {
            let id_value = row
                .get_ref(0)
                .map_err(|e| AppError::storage(format!("Embedding row read error: {e}")))?;
            let id = id_value
                .as_str()
                .map_err(|e| AppError::storage(format!("Embedding row read error: {e}")))?;
            let blob_value = row
                .get_ref(1)
                .map_err(|e| AppError::storage(format!("Embedding row read error: {e}")))?;
            let blob = blob_value
                .as_blob()
                .map_err(|e| AppError::storage(format!("Embedding row read error: {e}")))?;
            decode_embedding_into(blob, &mut scratch);
            visit(id, &scratch);
        }
        Ok(())
    }

    /// Path, title and text for the given chunks, in the same order. Chunks
    /// that no longer exist are left out.
    pub fn chunk_details(&self, chunk_ids: &[String]) -> Result<Vec<ChunkDetail>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT c.id, d.path, d.title, c.text, d.modified, d.size
                 FROM chunks c JOIN documents d ON d.id = c.document_id
                 WHERE c.id = ?1",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare chunk query: {e}")))?;
        let mut details = Vec::with_capacity(chunk_ids.len());
        for id in chunk_ids {
            let detail = stmt
                .query_row(params![id], |row| {
                    Ok(ChunkDetail {
                        chunk_id: row.get(0)?,
                        path: row.get(1)?,
                        title: row.get(2)?,
                        text: row.get(3)?,
                        modified: row.get::<_, i64>(4)? as u64,
                        size: row.get::<_, i64>(5)? as u64,
                    })
                })
                .optional()
                .map_err(|e| AppError::storage(format!("Failed to read chunk: {e}")))?;
            details.extend(detail);
        }
        Ok(details)
    }

    /// Chunks that have no stored vector yet, oldest documents first, so a
    /// backfill makes steady progress and can resume after a restart.
    pub fn chunks_missing_embeddings(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, String)>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.text
                 FROM chunks c
                 WHERE c.embedding IS NULL
                 ORDER BY c.document_id, c.chunk_index
                 LIMIT ?1",
            )
            .map_err(|e| {
                AppError::storage(format!("Failed to prepare missing-embeddings query: {e}"))
            })?;

        let rows = stmt
            .query_map([limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| AppError::storage(format!("Failed to query missing embeddings: {e}")))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| AppError::storage(format!("Missing row read error: {e}")))?);
        }
        Ok(out)
    }

    /// Store a vector for one chunk (a backfill step).
    pub fn update_chunk_embedding(&self, chunk_id: &str, vector: &[f32]) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let blob = Self::embedding_to_blob(vector);
        conn.execute(
            "UPDATE chunks SET embedding = ?1 WHERE id = ?2",
            rusqlite::params![blob, chunk_id],
        )
        .map_err(|e| AppError::storage(format!("Failed to store embedding: {e}")))?;
        Ok(())
    }

    /// (total chunks, chunks with stored vectors).
    pub fn embedding_coverage(&self) -> Result<(usize, usize), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
            .map_err(|e| AppError::storage(format!("Failed to count chunks: {e}")))?;
        let embedded: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .map_err(|e| AppError::storage(format!("Failed to count embeddings: {e}")))?;
        Ok((total as usize, embedded as usize))
    }

    fn sanitize_fts_query(query: &str) -> String {
        let clean: String = query
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect();

        let terms: Vec<String> = clean
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .map(|w| w.to_lowercase())
            .collect();

        if terms.is_empty() {
            return String::new();
        }

        terms.join(" OR ")
    }

    pub fn insert_or_replace_note(&self, note: &DbNote) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let tags_str = note.tags.join(",");
        conn.execute(
            "INSERT INTO notes (id, path, title, content, tags, size, modified, created_at, pinned)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                tags = excluded.tags,
                size = excluded.size,
                modified = excluded.modified",
            params![
                note.id,
                note.path,
                note.title,
                note.content,
                tags_str,
                note.size as i64,
                note.modified as i64,
                note.created_at as i64,
                note.pinned as i64,
            ],
        )
        .map_err(|e| AppError::storage(format!("Failed to insert/update note: {e}")))?;
        Ok(())
    }

    pub fn delete_note_by_path(&self, path: &str) -> Result<bool, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let count = conn
            .execute("DELETE FROM notes WHERE path = ?1", params![path])
            .map_err(|e| AppError::storage(format!("Failed to delete note: {e}")))?;
        Ok(count > 0)
    }

    pub fn get_note_by_path(&self, path: &str) -> Result<Option<DbNote>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let res = conn
            .query_row(
                "SELECT id, path, title, content, tags, size, modified, created_at, pinned
             FROM notes WHERE path = ?1",
                params![path],
                |row| {
                    let tags_str: String = row.get(4)?;
                    let tags = tags_str
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                    Ok(DbNote {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        title: row.get(2)?,
                        content: row.get(3)?,
                        tags,
                        size: row.get::<_, i64>(5)? as u64,
                        modified: row.get::<_, i64>(6)? as u64,
                        created_at: row.get::<_, i64>(7)? as u64,
                        pinned: row.get::<_, i64>(8)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|e| AppError::storage(format!("Failed to fetch note: {e}")))?;
        Ok(res)
    }

    pub fn list_notes(&self) -> Result<Vec<DbNote>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, path, title, content, tags, size, modified, created_at, pinned
             FROM notes ORDER BY pinned DESC, modified DESC",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare notes list query: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let tags_str: String = row.get(4)?;
                let tags = tags_str
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                Ok(DbNote {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    tags,
                    size: row.get::<_, i64>(5)? as u64,
                    modified: row.get::<_, i64>(6)? as u64,
                    created_at: row.get::<_, i64>(7)? as u64,
                    pinned: row.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to query notes: {e}")))?;

        let mut notes = Vec::new();
        for r in rows {
            notes.push(r.map_err(|e| AppError::storage(format!("Note row read error: {e}")))?);
        }
        Ok(notes)
    }

    /// Pins or unpins a note. Returns whether a row matched.
    pub fn set_note_pinned(&self, path: &str, pinned: bool) -> Result<bool, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let count = conn
            .execute(
                "UPDATE notes SET pinned = ?2 WHERE path = ?1",
                params![path, pinned as i64],
            )
            .map_err(|e| AppError::storage(format!("Failed to pin the note: {e}")))?;
        Ok(count > 0)
    }

    /// The pinned notes, oldest first, for the always-in-context block.
    pub fn pinned_notes(&self) -> Result<Vec<DbNote>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, path, title, content, tags, size, modified, created_at, pinned
             FROM notes WHERE pinned = 1 ORDER BY created_at ASC",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare pinned notes query: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                let tags_str: String = row.get(4)?;
                let tags = tags_str
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                Ok(DbNote {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    tags,
                    size: row.get::<_, i64>(5)? as u64,
                    modified: row.get::<_, i64>(6)? as u64,
                    created_at: row.get::<_, i64>(7)? as u64,
                    pinned: row.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to query pinned notes: {e}")))?;
        let mut notes = Vec::new();
        for r in rows {
            notes.push(
                r.map_err(|e| AppError::storage(format!("Pinned note row read error: {e}")))?,
            );
        }
        Ok(notes)
    }

    pub fn clear_all_notes(&self) -> Result<usize, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let count = conn
            .execute("DELETE FROM notes", [])
            .map_err(|e| AppError::storage(format!("Failed to clear notes: {e}")))?;
        Ok(count)
    }

    pub fn record_tool_audit(
        &self,
        tool_name: &str,
        action: &str,
        path: &str,
        approved: bool,
        status: &str,
        details: Option<&str>,
    ) -> Result<i64, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO tool_audit (timestamp, tool_name, action, path, approved, status, details)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                ts,
                tool_name,
                action,
                path,
                if approved { 1 } else { 0 },
                status,
                details
            ],
        )
        .map_err(|e| AppError::storage(format!("Failed to record tool audit: {e}")))?;
        Ok(conn.last_insert_rowid())
    }

    /// Sets the outcome of an audit row written before its action ran.
    pub fn finish_tool_audit(
        &self,
        id: i64,
        status: &str,
        details: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute(
            "UPDATE tool_audit SET status = ?1, details = ?2 WHERE id = ?3",
            params![status, details, id],
        )
        .map_err(|e| AppError::storage(format!("Failed to update tool audit: {e}")))?;
        Ok(())
    }

    pub fn list_tool_audits(&self, limit: usize) -> Result<Vec<DbToolAudit>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, tool_name, action, path, approved, status, details
             FROM tool_audit
             ORDER BY timestamp DESC, id DESC
             LIMIT ?1",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare tool audit query: {e}")))?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(DbToolAudit {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    tool_name: row.get(2)?,
                    action: row.get(3)?,
                    path: row.get(4)?,
                    approved: row.get::<_, i64>(5)? != 0,
                    status: row.get(6)?,
                    details: row.get(7)?,
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to query tool audits: {e}")))?;

        let mut audits = Vec::new();
        for r in rows {
            audits.push(r.map_err(|e| AppError::storage(format!("Audit row read error: {e}")))?);
        }
        Ok(audits)
    }

    pub fn clear_tool_audits(&self) -> Result<usize, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let count = conn
            .execute("DELETE FROM tool_audit", [])
            .map_err(|e| AppError::storage(format!("Failed to clear tool audits: {e}")))?;
        Ok(count)
    }

    pub fn embedding_to_blob(vec: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(vec.len() * 4);
        for val in vec {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        bytes
    }

    pub fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
        let mut out = Vec::with_capacity(blob.len() / 4);
        decode_embedding_into(blob, &mut out);
        out
    }
}

/// Decode a little-endian f32 embedding blob into `out`, reusing its capacity
/// across calls. Trailing bytes that do not form a whole f32 are ignored,
/// matching `blob_to_embedding`.
pub fn decode_embedding_into(blob: &[u8], out: &mut Vec<f32>) {
    out.clear();
    out.reserve(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
}

/// One call that left this computer (Settings → Online providers → What left
/// this computer).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CloudCallRow {
    pub id: i64,
    pub timestamp: i64,
    pub provider: String,
    pub host: String,
    pub model: String,
    pub kind: String,
    pub preview: String,
    pub bytes: i64,
    pub status: String,
    pub detail: Option<String>,
}

/// A folder the user added to the library.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FolderRow {
    pub path: String,
    pub added_at: i64,
    pub last_scan_at: i64,
    pub indexed: i64,
    pub skipped: i64,
    pub limited: bool,
    /// Documents now in the library from this folder.
    pub documents: i64,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl LibraryDb {
    #[allow(clippy::too_many_arguments)]
    pub fn record_cloud_call(
        &self,
        provider: &str,
        host: &str,
        model: &str,
        kind: &str,
        preview: &str,
        bytes: usize,
        status: &str,
    ) -> Result<i64, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute(
            "INSERT INTO cloud_audit (timestamp, provider, host, model, kind, preview, bytes, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![now_secs(), provider, host, model, kind, preview, bytes as i64, status],
        )
        .map_err(|e| AppError::storage(format!("Failed to record an online call: {e}")))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn finish_cloud_call(
        &self,
        id: i64,
        status: &str,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute(
            "UPDATE cloud_audit SET status = ?2, detail = ?3 WHERE id = ?1",
            params![id, status, detail],
        )
        .map_err(|e| AppError::storage(format!("Failed to update an online call: {e}")))?;
        Ok(())
    }

    pub fn list_cloud_calls(&self, limit: usize) -> Result<Vec<CloudCallRow>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, provider, host, model, kind, preview, bytes, status, detail
                 FROM cloud_audit ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| AppError::storage(e.to_string()))?;
        let rows = stmt
            .query_map(params![limit as i64], |r| {
                Ok(CloudCallRow {
                    id: r.get(0)?,
                    timestamp: r.get(1)?,
                    provider: r.get(2)?,
                    host: r.get(3)?,
                    model: r.get(4)?,
                    kind: r.get(5)?,
                    preview: r.get(6)?,
                    bytes: r.get(7)?,
                    status: r.get(8)?,
                    detail: r.get(9)?,
                })
            })
            .map_err(|e| AppError::storage(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::storage(e.to_string()))
    }

    pub fn clear_cloud_calls(&self) -> Result<usize, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute("DELETE FROM cloud_audit", [])
            .map_err(|e| AppError::storage(format!("Failed to clear the online log: {e}")))
    }

    /// Remember a folder scan. The first scan sets when it was added.
    pub fn save_folder_scan(
        &self,
        path: &str,
        indexed: usize,
        skipped: usize,
        limited: bool,
    ) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let now = now_secs();
        conn.execute(
            "INSERT INTO library_folders (path, added_at, last_scan_at, indexed, skipped, limited)
             VALUES (?1, ?2, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET last_scan_at = ?2, indexed = ?3, skipped = ?4, limited = ?5",
            params![path, now, indexed as i64, skipped as i64, limited as i64],
        )
        .map_err(|e| AppError::storage(format!("Failed to save the folder: {e}")))?;
        Ok(())
    }

    pub fn list_folders(&self) -> Result<Vec<FolderRow>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT path, added_at, last_scan_at, indexed, skipped, limited FROM library_folders ORDER BY added_at",
            )
            .map_err(|e| AppError::storage(e.to_string()))?;
        let mut rows: Vec<FolderRow> = stmt
            .query_map([], |r| {
                Ok(FolderRow {
                    path: r.get(0)?,
                    added_at: r.get(1)?,
                    last_scan_at: r.get(2)?,
                    indexed: r.get(3)?,
                    skipped: r.get(4)?,
                    limited: r.get::<_, i64>(5)? != 0,
                    documents: 0,
                })
            })
            .map_err(|e| AppError::storage(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::storage(e.to_string()))?;
        for row in &mut rows {
            let sep = if row.path.contains('\\') { '\\' } else { '/' };
            let prefix = format!("{}{sep}%", row.path.trim_end_matches(['/', '\\']));
            row.documents = conn
                .query_row(
                    "SELECT COUNT(*) FROM documents WHERE path LIKE ?1 ESCAPE '^'",
                    params![prefix.replace('^', "^^").replace('_', "^_")],
                    |r| r.get(0),
                )
                .unwrap_or(0);
        }
        Ok(rows)
    }

    pub fn remove_folder(&self, path: &str) -> Result<bool, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let n = conn
            .execute("DELETE FROM library_folders WHERE path = ?1", params![path])
            .map_err(|e| AppError::storage(e.to_string()))?;
        Ok(n > 0)
    }

    /// Forget every stored vector, for example after a switch to an
    /// embedding model with other vectors.
    pub fn clear_embeddings(&self) -> Result<usize, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        conn.execute(
            "UPDATE chunks SET embedding = NULL WHERE embedding IS NOT NULL",
            [],
        )
        .map_err(|e| AppError::storage(e.to_string()))
    }
}

#[cfg(test)]
mod audit_and_folder_tests {
    use super::LibraryDb;

    #[test]
    fn cloud_calls_and_folders_round_trip() {
        let db = LibraryDb::open_in_memory().unwrap();
        let id = db
            .record_cloud_call(
                "OpenRouter",
                "openrouter.ai",
                "m",
                "chat",
                "hello",
                5,
                "sent",
            )
            .unwrap();
        db.finish_cloud_call(id, "ok", Some("200")).unwrap();
        let calls = db.list_cloud_calls(10).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].status, "ok");
        assert_eq!(db.clear_cloud_calls().unwrap(), 1);

        db.save_folder_scan("/home/u/Documents", 3, 1, false)
            .unwrap();
        db.save_folder_scan("/home/u/Documents", 4, 0, false)
            .unwrap();
        let folders = db.list_folders().unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].indexed, 4);
        assert!(db.remove_folder("/home/u/Documents").unwrap());
    }
}
