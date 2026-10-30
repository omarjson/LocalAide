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

#[derive(Debug, Clone)]
pub struct ChunkEmbeddingRecord {
    pub chunk_id: String,
    pub document_id: String,
    pub path: String,
    pub title: String,
    pub text: String,
    pub modified: u64,
    pub size: u64,
    pub embedding: Vec<f32>,
}

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
        let conn = self
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

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                chunk_id UNINDEXED,
                document_id UNINDEXED,
                title,
                text,
                tokenize = 'porter unicode61'
            );",
        )
        .map_err(|e| AppError::storage(format!("Failed to apply SQLite schema migrations: {e}")))?;

        Ok(())
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

        // Remove old document chunks from fts table before replacing
        if let Some(existing_id) = tx
            .query_row(
                "SELECT id FROM documents WHERE path = ?1",
                params![doc.path],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| AppError::storage(format!("Failed to check existing document: {e}")))?
        {
            tx.execute(
                "DELETE FROM chunks_fts WHERE document_id = ?1",
                params![existing_id],
            )
            .map_err(|e| AppError::storage(format!("Failed to clear old FTS entries: {e}")))?;
            tx.execute("DELETE FROM documents WHERE id = ?1", params![existing_id])
                .map_err(|e| AppError::storage(format!("Failed to clear old document: {e}")))?;
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

        // Insert chunks & FTS entries
        for chunk in chunks {
            let blob = chunk
                .embedding
                .as_ref()
                .map(|vec| Self::embedding_to_blob(vec));
            tx.execute(
                "INSERT INTO chunks (id, document_id, chunk_index, text, token_count, start_char, end_char, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    chunk.id,
                    chunk.document_id,
                    chunk.chunk_index as i64,
                    chunk.text,
                    chunk.token_count as i64,
                    chunk.start_char as i64,
                    chunk.end_char as i64,
                    blob,
                ],
            ).map_err(|e| AppError::storage(format!("Failed to insert chunk: {e}")))?;

            tx.execute(
                "INSERT INTO chunks_fts (chunk_id, document_id, title, text)
                 VALUES (?1, ?2, ?3, ?4)",
                params![chunk.id, chunk.document_id, doc.title, chunk.text,],
            )
            .map_err(|e| AppError::storage(format!("Failed to insert FTS entry: {e}")))?;
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
            tx.execute("DELETE FROM chunks_fts WHERE document_id = ?1", params![id])
                .map_err(|e| AppError::storage(format!("Failed to delete FTS records: {e}")))?;
            tx.execute("DELETE FROM documents WHERE id = ?1", params![id])
                .map_err(|e| AppError::storage(format!("Failed to delete document: {e}")))?;
            tx.commit()
                .map_err(|e| AppError::storage(format!("Failed to commit delete: {e}")))?;
            Ok(true)
        } else {
            Ok(false)
        }
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

    pub fn list_documents(&self) -> Result<Vec<DbDocument>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, path, title, content_hash, size, modified, indexed_at, kind
             FROM documents ORDER BY indexed_at DESC",
            )
            .map_err(|e| AppError::storage(format!("Failed to prepare list statement: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
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

    pub fn all_chunk_embeddings(&self) -> Result<Vec<ChunkEmbeddingRecord>, AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AppError::internal("DB lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.document_id, d.path, d.title, c.text, d.modified, d.size, c.embedding
             FROM chunks c
             JOIN documents d ON d.id = c.document_id
             WHERE c.embedding IS NOT NULL",
        ).map_err(|e| AppError::storage(format!("Failed to prepare embeddings statement: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let blob: Vec<u8> = row.get(7)?;
                let embedding = Self::blob_to_embedding(&blob);
                Ok(ChunkEmbeddingRecord {
                    chunk_id: row.get(0)?,
                    document_id: row.get(1)?,
                    path: row.get(2)?,
                    title: row.get(3)?,
                    text: row.get(4)?,
                    modified: row.get::<_, i64>(5)? as u64,
                    size: row.get::<_, i64>(6)? as u64,
                    embedding,
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to query embeddings: {e}")))?;

        let mut records = Vec::new();
        for r in rows {
            records
                .push(r.map_err(|e| AppError::storage(format!("Embedding row read error: {e}")))?);
        }
        Ok(records)
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
            "INSERT INTO notes (id, path, title, content, tags, size, modified, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
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
                "SELECT id, path, title, content, tags, size, modified, created_at
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
                "SELECT id, path, title, content, tags, size, modified, created_at
             FROM notes ORDER BY modified DESC",
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
                })
            })
            .map_err(|e| AppError::storage(format!("Failed to query notes: {e}")))?;

        let mut notes = Vec::new();
        for r in rows {
            notes.push(r.map_err(|e| AppError::storage(format!("Note row read error: {e}")))?);
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
        blob.chunks_exact(4)
            .map(|chunk| {
                let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                f32::from_le_bytes(arr)
            })
            .collect()
    }
}
