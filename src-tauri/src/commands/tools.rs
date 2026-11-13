use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::tool::{ToolAuditEntry, ToolExecutionRequest, ToolExecutionResponse};
use tauri::State;

#[tauri::command]
pub fn tool_execute(
    state: State<'_, AppState>,
    req: ToolExecutionRequest,
) -> Result<ToolExecutionResponse, AppError> {
    state.tools.execute(req)
}

#[tauri::command]
pub fn tool_audits_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ToolAuditEntry>, AppError> {
    state.tools.list_audits(limit.unwrap_or(100))
}

#[tauri::command]
pub fn tool_audits_clear(state: State<'_, AppState>) -> Result<(), AppError> {
    state.tools.clear_audits()
}
