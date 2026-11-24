//! Usage ledger: one row per model call, in `app_dir()/index/usage.db`.
//!
//! This is separate from the aggregated JSONL usage log. It keeps a small,
//! queryable record of tokens and an estimated cost so a thread can show what
//! it spent. No pricing table is built in: the caller supplies a flat rate in
//! USD per 1M tokens, and the documented default is only a fallback.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

/// File name inside `app_dir()/index`.
pub const USAGE_DB: &str = "usage.db";

/// Documented fallback rate: USD per 1M tokens, applied to input + output
/// alike. It is a flat estimate, not a claim about any provider's real price.
/// Callers that know a rate should pass it in.
pub const DEFAULT_FLAT_RATE_PER_MILLION_USD: f64 = 1.0;

/// Where the ledger lives by default.
pub fn default_db_path() -> PathBuf {
    crate::app::paths::index_dir().join(USAGE_DB)
}

/// One recorded model call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageLedgerEntry {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub estimated_cost_usd: f64,
    /// Unix timestamp, seconds since the epoch (UTC).
    pub at: i64,
    pub thread_id: String,
}

/// Estimate cost from a flat USD-per-1M-tokens rate supplied by the caller.
/// Negative token counts and rates are clamped to zero. No per-model pricing
/// is invented here.
pub fn estimate_cost_usd(
    input_tokens: i64,
    output_tokens: i64,
    usd_per_million_tokens: f64,
) -> f64 {
    let tokens = input_tokens.max(0) as f64 + output_tokens.max(0) as f64;
    (tokens / 1_000_000.0) * usd_per_million_tokens.max(0.0)
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn open(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::storage(format!("Could not create the index folder: {e}")))?;
    }
    let conn = Connection::open(path)
        .map_err(|e| AppError::storage(format!("Could not open the usage ledger: {e}")))?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|e| AppError::storage(format!("Could not configure the usage ledger: {e}")))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

/// Idempotent schema.
fn ensure_schema(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS usage_ledger (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             provider TEXT NOT NULL,
             model TEXT NOT NULL,
             input_tokens INTEGER NOT NULL,
             output_tokens INTEGER NOT NULL,
             estimated_cost_usd REAL NOT NULL,
             at INTEGER NOT NULL,
             thread_id TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS usage_ledger_thread ON usage_ledger(thread_id, at);",
    )
    .map_err(|e| AppError::storage(format!("Could not create the usage ledger schema: {e}")))
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<UsageLedgerEntry> {
    Ok(UsageLedgerEntry {
        id: row.get(0)?,
        provider: row.get(1)?,
        model: row.get(2)?,
        input_tokens: row.get(3)?,
        output_tokens: row.get(4)?,
        estimated_cost_usd: row.get(5)?,
        at: row.get(6)?,
        thread_id: row.get(7)?,
    })
}

/// Add one row. `rate_per_million_usd` is the caller's flat rate; `None` uses
/// the documented default. Returns the stored row, including its timestamp.
#[allow(clippy::too_many_arguments)]
pub fn add_at(
    path: &Path,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    thread_id: &str,
    rate_per_million_usd: Option<f64>,
) -> Result<UsageLedgerEntry, AppError> {
    let rate = rate_per_million_usd.unwrap_or(DEFAULT_FLAT_RATE_PER_MILLION_USD);
    let cost = estimate_cost_usd(input_tokens, output_tokens, rate);
    let at = now_secs();
    let conn = open(path)?;
    conn.execute(
        "INSERT INTO usage_ledger(provider, model, input_tokens, output_tokens,
                                  estimated_cost_usd, at, thread_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            provider,
            model,
            input_tokens,
            output_tokens,
            cost,
            at,
            thread_id
        ],
    )
    .map_err(|e| AppError::storage(format!("Could not record usage: {e}")))?;
    Ok(UsageLedgerEntry {
        id: conn.last_insert_rowid(),
        provider: provider.to_string(),
        model: model.to_string(),
        input_tokens,
        output_tokens,
        estimated_cost_usd: cost,
        at,
        thread_id: thread_id.to_string(),
    })
}

/// Add one row in the default database.
#[allow(clippy::too_many_arguments)]
pub fn add(
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    thread_id: &str,
    rate_per_million_usd: Option<f64>,
) -> Result<UsageLedgerEntry, AppError> {
    add_at(
        &default_db_path(),
        provider,
        model,
        input_tokens,
        output_tokens,
        thread_id,
        rate_per_million_usd,
    )
}

/// List rows, optionally only one thread's, newest first.
pub fn list_at(
    path: &Path,
    thread_id: Option<&str>,
    limit: usize,
) -> Result<Vec<UsageLedgerEntry>, AppError> {
    let conn = open(path)?;
    let limit = limit.clamp(1, 1000) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT id, provider, model, input_tokens, output_tokens,
                    estimated_cost_usd, at, thread_id
             FROM usage_ledger
             WHERE ?1 IS NULL OR thread_id = ?1
             ORDER BY at DESC, id DESC
             LIMIT ?2",
        )
        .map_err(|e| AppError::storage(format!("Could not prepare the usage list: {e}")))?;
    let rows = stmt
        .query_map(params![thread_id, limit], row_to_entry)
        .map_err(|e| AppError::storage(format!("Could not list usage: {e}")))?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| AppError::storage(format!("Could not list usage: {e}")))?);
    }
    Ok(entries)
}

/// List rows in the default database.
pub fn list(thread_id: Option<&str>, limit: usize) -> Result<Vec<UsageLedgerEntry>, AppError> {
    list_at(&default_db_path(), thread_id, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_uses_the_callers_flat_rate() {
        // 1M total tokens at $2 / 1M -> $2, regardless of the split.
        assert!((estimate_cost_usd(600_000, 400_000, 2.0) - 2.0).abs() < 1e-9);
        assert!((estimate_cost_usd(1_000_000, 0, 1.0) - 1.0).abs() < 1e-9);
        assert!((estimate_cost_usd(0, 500_000, 4.0) - 2.0).abs() < 1e-9);
        assert_eq!(estimate_cost_usd(0, 0, 3.0), 0.0);
    }

    #[test]
    fn cost_never_invents_a_price_and_clamps_bad_inputs() {
        assert_eq!(estimate_cost_usd(1_000, 1_000, 0.0), 0.0);
        assert_eq!(
            estimate_cost_usd(-5, 100, 1.0),
            estimate_cost_usd(0, 100, 1.0)
        );
        assert_eq!(
            estimate_cost_usd(100, -5, 1.0),
            estimate_cost_usd(100, 0, 1.0)
        );
        assert_eq!(estimate_cost_usd(100, 100, -1.0), 0.0);
    }

    #[test]
    fn entries_round_trip_and_filter_by_thread() {
        let dir = std::env::temp_dir().join(format!("localaide-usage-db-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let db = dir.join("usage.db");

        let first = add_at(&db, "local", "qwen3-4b", 1000, 500, "t1", Some(2.0)).unwrap();
        let expected = estimate_cost_usd(1000, 500, 2.0);
        assert!((first.estimated_cost_usd - expected).abs() < 1e-12);
        assert!(first.at > 0);

        // Reopening applies the same schema again without error, and the
        // fallback rate is the documented flat default.
        let second = add_at(&db, "cloud", "gpt-x", 100, 100, "t2", None).unwrap();
        assert!(
            (second.estimated_cost_usd
                - estimate_cost_usd(100, 100, DEFAULT_FLAT_RATE_PER_MILLION_USD))
            .abs()
                < 1e-12
        );

        let t1 = list_at(&db, Some("t1"), 50).unwrap();
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0].model, "qwen3-4b");
        assert_eq!(t1[0].input_tokens, 1000);
        let all = list_at(&db, None, 50).unwrap();
        assert_eq!(all.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
