//! Tauri command for the local setup doctor (report only, no repairs).

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::doctor::{self, DoctorReport};

/// Run every local check and return a serializable report. The command
/// itself never fails; a broken subsystem is reported in `checks`.
#[tauri::command]
pub async fn doctor_run() -> Result<DoctorReport, AppError> {
    run_blocking(|| Ok(doctor::run())).await
}
