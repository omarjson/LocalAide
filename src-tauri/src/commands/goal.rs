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

/// Pause a goal: progress stopped, but it is not done. A paused goal
/// takes no steps until it is resumed.
#[tauri::command]
pub async fn goal_pause(id: String) -> Result<Goal, AppError> {
    run_blocking(move || goal::pause(&id)).await
}

/// Mark a goal blocked: it waits on something outside the run.
#[tauri::command]
pub async fn goal_block(id: String) -> Result<Goal, AppError> {
    run_blocking(move || goal::block(&id)).await
}

/// Resume a paused or blocked goal.
#[tauri::command]
pub async fn goal_resume(id: String) -> Result<Goal, AppError> {
    run_blocking(move || goal::resume(&id)).await
}
