//! Goal commands. All state lives in the local goals database; there is no
//! model call on this path.

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::goal::{self, Goal};

/// Start a goal with a step budget.
#[tauri::command]
pub async fn goal_start(
    thread_id: String,
    objective: String,
    budget: i64,
) -> Result<Goal, AppError> {
    run_blocking(move || goal::start(&thread_id, &objective, budget)).await
}

/// Read one goal's current state. `None` when the id is unknown.
#[tauri::command]
pub async fn goal_status(id: String) -> Result<Option<Goal>, AppError> {
    run_blocking(move || goal::status(&id)).await
}

/// Record a step's progress text and consume one unit of budget.
#[tauri::command]
pub async fn goal_step(id: String, progress: String) -> Result<Goal, AppError> {
    run_blocking(move || goal::step(&id, &progress)).await
}

/// Mark a goal completed.
#[tauri::command]
pub async fn goal_complete(id: String) -> Result<Goal, AppError> {
    run_blocking(move || goal::complete(&id)).await
}
