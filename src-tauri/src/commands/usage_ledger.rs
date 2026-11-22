//! Usage ledger commands: record one model call and list the ledger.

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::usage_ledger::{self, UsageLedgerEntry};

/// Record one call. `rate_per_million_usd` is a flat caller-supplied rate;
/// when absent the documented flat default is used. No per-model pricing is
/// looked up or invented.
#[tauri::command]
pub async fn usage_record(
    provider: String,
    model: String,
    input_tokens: i64,
    output_tokens: i64,
    thread_id: String,
    rate_per_million_usd: Option<f64>,
) -> Result<UsageLedgerEntry, AppError> {
    run_blocking(move || {
        usage_ledger::add(
            &provider,
            &model,
            input_tokens,
            output_tokens,
            &thread_id,
            rate_per_million_usd,
        )
    })
    .await
}

/// List recorded calls, newest first. `thread_id` filters one chat when set.
#[tauri::command]
pub async fn usage_list(
    thread_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<UsageLedgerEntry>, AppError> {
    run_blocking(move || {
        usage_ledger::list(thread_id.as_deref(), limit.unwrap_or(200))
    })
    .await
}
