//! Local full-text search over saved chat threads.
//!
//! Threads Live on Disk As `~/.aiassistant/conversations/<id>.json`. This
//! module keeps a small SQLite FTS5 index beside the library database so the
//! window can search old conversations without reading every file. Nothing
//! here talks to a network service, and reindexing is a full, atomic rebuild.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

/// Name of the index file inside `app_dir()/index`.
pub const INDEX_FILE: &str = "session_index.db";

/// One hit returned by [`search`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionHit {
    pub thread_id: String,
    pub title: String,
    /// A short match-centered excerpt with `[` and `]` around the term.
    pub snippet: String,
    /// Path of the JSON file the hit came from.
    pub path: String,
}

/// Where the index lives by default.
pub fn default_db_path() -> PathBuf {
    crate::app::paths::index_dir().join(INDEX_FILE)
}

fn open(db_path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::storage(format!("Could not create the index folder: {e}")))?;
    }
    Connection::open(db_path)
        .map_err(|e| AppError::storage(format!("Could not open the session index: {e}")))
}

fn ensure_schema(conn: &Connection) -> Result<(), AppError> {
    // The bundled SQLite is compiled with SQLITE_ENABLE_FTS5 (the library DB
    // already uses fts5), so raw FTS5 SQL works without rusqlite's `fts5`
    // feature gate. `path` and `updated_at` are stored but not tokenized.
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
            thread_id UNINDEXED,
            title,
            body,
            path UNINDEXED,
            updated_at UNINDEXED,
            tokenize = 'unicode61'
        );",
    )
    .map_err(|e| AppError::storage(format!("Could not create the session index schema: {e}")))
}

/// Recursively collect the searchable text out of a JSON value. Stored chat
/// messages use `content`, but older/other exports use `text` or `message`,
/// and content may be a list of parts; accepting all of them costs nothing.
fn text_of(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(text_of)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for key in ["text", "content", "message"] {
                if let Some(v) = map.get(key) {
                    parts.push(text_of(v));
                }
            }
            parts
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => String::new(),
    }
}

fn thread_fields(path: &Path, value: &serde_json::Value) -> (String, String, String, i64) {
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Untitled".to_string());
    let body = value.get("messages").map(text_of).unwrap_or_default();
    let updated = value
        .get("updated_at")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(i64::MAX as u64) as i64;
    (id, title, body, updated)
}

/// Rebuild the whole index from the JSON files in `conversations`.
///
/// Returns the number of threads indexed. Unreadable or malformed files are
/// skipped with a warning rather than failing the whole reindex.
pub fn index_all(db_path: &Path, conversations: &Path) -> Result<usize, AppError> {
    let mut conn = open(db_path)?;
    ensure_schema(&conn)?;

    let mut files: Vec<PathBuf> = Vec::new();
    if conversations.is_dir() {
        let entries = fs::read_dir(conversations).map_err(|e| {
            AppError::storage(format!("Could not read the conversations folder: {e}"))
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }
    files.sort();

    let tx = conn
        .transaction()
        .map_err(|e| AppError::storage(format!("Could not start a reindex transaction: {e}")))?;
    tx.execute("DELETE FROM session_fts", params![])
        .map_err(|e| AppError::storage(format!("Could not clear the session index: {e}")))?;
    let mut insert = tx
        .prepare(
            "INSERT INTO session_fts(thread_id, title, body, path, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(|e| AppError::storage(format!("Could not prepare the index insert: {e}")))?;

    let mut count = 0usize;
    for path in files {
        let data = match fs::read_to_string(&path) {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Skipping unreadable conversation {}: {e}", path.display());
                continue;
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(e) => {
                log::warn!("Skipping malformed conversation {}: {e}", path.display());
                continue;
            }
        };
        let (id, title, body, updated) = thread_fields(&path, &value);
        if let Err(e) = insert.execute(params![
            id,
            title,
            body,
            path.to_string_lossy().into_owned(),
            updated
        ]) {
            log::warn!("Skipping conversation {}: {e}", path.display());
            continue;
        }
        count += 1;
    }
    drop(insert);
    tx.commit()
        .map_err(|e| AppError::storage(format!("Could not commit the reindex: {e}")))?;
    Ok(count)
}

/// Turn arbitrary user text into a safe FTS5 MATCH expression: only
/// alphanumeric/underscore tokens, each quoted, joined as an implicit AND.
/// This avoids FTS5 syntax errors from characters like `"`, `*` or `:`.
fn match_expression(query: &str) -> String {
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Search the index. An empty/all-punctuation query returns no hits.
/// `limit` is clamped to a sane range so a bad caller cannot ask for millions.
pub fn search(db_path: &Path, query: &str, limit: usize) -> Result<Vec<SessionHit>, AppError> {
    let expr = match_expression(query);
    if expr.is_empty() {
        return Ok(Vec::new());
    }
    let conn = open(db_path)?;
    ensure_schema(&conn)?;
    let limit = limit.clamp(1, 200) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT thread_id, title, path, snippet(session_fts, 2, '[', ']', ' ... ', 16)
             FROM session_fts
             WHERE session_fts MATCH ?1
             ORDER BY rank, updated_at DESC
             LIMIT ?2",
        )
        .map_err(|e| AppError::storage(format!("Could not prepare the session search: {e}")))?;
    let rows = stmt
        .query_map(params![expr, limit], |row| {
            Ok(SessionHit {
                thread_id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                snippet: row.get(3)?,
            })
        })
        .map_err(|e| AppError::storage(format!("Session search failed: {e}")))?;
    let mut hits = Vec::new();
    for row in rows {
        hits.push(row.map_err(|e| AppError::storage(format!("Session search failed: {e}")))?);
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_expression_quotes_tokens() {
        assert_eq!(match_expression("rust sqlite"), "\"rust\" \"sqlite\"");
        assert_eq!(match_expression("a \" b"), "\"a\" \"b\"");
        assert!(match_expression("!!!").is_empty());
    }

    #[test]
    fn index_and_search_round_trip() {
        let dir = std::env::temp_dir().join(format!("la-session-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let conversations = dir.join("conversations");
        fs::create_dir_all(&conversations).unwrap();
        fs::write(
            conversations.join("t_1.json"),
            r#"{"id":"t_1","title":"Rust evening","updated_at":10,"messages":[{"role":"user","content":"sqlite full text search"}]}"#,
        )
        .unwrap();
        let db = dir.join("sessions.db");
        assert_eq!(index_all(&db, &conversations).unwrap(), 1);
        let hits = search(&db, "sqlite", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].thread_id, "t_1");
        assert_eq!(hits[0].title, "Rust evening");
        assert!(hits[0].snippet.contains("sqlite"));
        assert!(search(&db, "postgres", 10).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
