//! Aggregated answer usage for the Usage screen.

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::usage::{self, UsageSummary};

/// Rolls the usage log up over the last `days` days (default 30).
#[tauri::command]
pub async fn usage_summary(days: Option<u32>) -> Result<UsageSummary, AppError> {
    let days = days.unwrap_or(30).clamp(1, 3650);
    run_blocking(move || usage::summary(days)).await
}
