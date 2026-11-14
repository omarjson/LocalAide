use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::document::SearchResultDoc;
use crate::domain::memory::{ForgetAllResponse, MemoryCreateResponse, MemoryNote};
use tauri::State;

// Note writes re-index the note and searches may embed the query, so the
// commands that do that work run through `with_app_state`.

#[tauri::command]
pub async fn memory_create(
    state: State<'_, AppState>,
    title: String,
    content: String,
    tags: Option<Vec<String>>,
) -> Result<MemoryCreateResponse, AppError> {
    with_app_state(&state, move |state| {
        let note = state.memory.create_note(&title, &content, tags)?;
        Ok(MemoryCreateResponse { path: note.path })
    })
    .await
}

#[tauri::command]
pub async fn memory_update(
    state: State<'_, AppState>,
    path: String,
    content: String,
    tags: Option<Vec<String>>,
) -> Result<(), AppError> {
    with_app_state(&state, move |state| {
        state.memory.update_note(&path, &content, tags)
    })
    .await
}

#[tauri::command]
pub fn memory_read(state: State<'_, AppState>, path: String) -> Result<String, AppError> {
    state.memory.read_note(&path)
}

#[tauri::command]
pub async fn memory_delete(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    with_app_state(&state, move |state| {
        state.memory.delete_note(&path)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn memory_list(state: State<'_, AppState>) -> Result<Vec<MemoryNote>, AppError> {
    with_app_state(&state, |state| state.memory.list_notes()).await
}

#[tauri::command]
pub async fn memory_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchResultDoc>, AppError> {
    with_app_state(&state, move |state| {
        let query_vector = state.embedding_engine.embed_query(&query).ok();
        state.memory.search(&query, query_vector.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn memory_forget_all(state: State<'_, AppState>) -> Result<ForgetAllResponse, AppError> {
    with_app_state(&state, |state| {
        let deleted_count = state.memory.forget_all()?;
        Ok(ForgetAllResponse { deleted_count })
    })
    .await
}
