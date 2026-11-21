//! Aggregated answer usage for the Usage screen.

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::usage::{self, UsageSummary};

/// Rolls the usage log up. `days` of 1 is today (the current UTC day), 0 is
/// all time, anything else the rolling last `days` days (default 30). When
/// `folder` is set, only the answers of that project are counted.
#[tauri::command]
pub async fn usage_summary(
    days: Option<u32>,
    folder: Option<String>,
) -> Result<UsageSummary, AppError> {
    let days = days.unwrap_or(30).clamp(0, 36500);
    run_blocking(move || usage::summary(days, folder.as_deref())).await
}
