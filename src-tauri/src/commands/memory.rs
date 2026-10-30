use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::document::SearchResultDoc;
use crate::domain::memory::{ForgetAllResponse, MemoryCreateResponse, MemoryNote};
use tauri::State;

#[tauri::command]
pub fn memory_create(
    state: State<'_, AppState>,
    title: String,
    content: String,
    tags: Option<Vec<String>>,
) -> Result<MemoryCreateResponse, AppError> {
    let note = state.memory.create_note(&title, &content, tags)?;
    Ok(MemoryCreateResponse { path: note.path })
}

#[tauri::command]
pub fn memory_update(
    state: State<'_, AppState>,
    path: String,
    content: String,
    tags: Option<Vec<String>>,
) -> Result<(), AppError> {
    state.memory.update_note(&path, &content, tags)
}

#[tauri::command]
pub fn memory_read(state: State<'_, AppState>, path: String) -> Result<String, AppError> {
    state.memory.read_note(&path)
}

#[tauri::command]
pub fn memory_delete(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    state.memory.delete_note(&path)?;
    Ok(())
}

#[tauri::command]
pub fn memory_list(state: State<'_, AppState>) -> Result<Vec<MemoryNote>, AppError> {
    state.memory.list_notes()
}

#[tauri::command]
pub fn memory_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchResultDoc>, AppError> {
    state.memory.search(&query)
}

#[tauri::command]
pub fn memory_forget_all(state: State<'_, AppState>) -> Result<ForgetAllResponse, AppError> {
    let deleted_count = state.memory.forget_all()?;
    Ok(ForgetAllResponse { deleted_count })
}
