use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::document::{
    IndexFolderResponse, IndexerStatsResponse, SearchIndexRequest, SearchResultDoc,
    WatchFolderResponse,
};
use crate::services::library::search::SearchMode;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub fn index_file(state: State<'_, AppState>, path: String) -> Result<SearchResultDoc, AppError> {
    state.library.index_file(Path::new(&path))
}

#[tauri::command]
pub fn index_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<IndexFolderResponse, AppError> {
    let (indexed, skipped) = state.library.index_folder(Path::new(&folder), None)?;
    Ok(IndexFolderResponse { indexed, skipped })
}

#[tauri::command]
pub fn search_index(
    state: State<'_, AppState>,
    query: String,
    mode: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchResultDoc>, AppError> {
    let search_mode = match mode.as_deref() {
        Some("semantic") => SearchMode::Semantic,
        Some("keyword") => SearchMode::Keyword,
        _ => SearchMode::Hybrid,
    };
    let limit_num = limit.unwrap_or(20);
    state.library.search(&query, search_mode, limit_num)
}

#[tauri::command]
pub fn list_index(state: State<'_, AppState>) -> Result<Vec<SearchResultDoc>, AppError> {
    state.library.list()
}

#[tauri::command]
pub fn indexer_stats(state: State<'_, AppState>) -> Result<IndexerStatsResponse, AppError> {
    state.library.stats()
}

#[tauri::command]
pub fn index_remove(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    state.library.remove(&path)?;
    Ok(())
}

#[tauri::command]
pub fn index_watch_start(
    state: State<'_, AppState>,
    folder: String,
) -> Result<WatchFolderResponse, AppError> {
    state.library.start_watch(Path::new(&folder))?;
    Ok(WatchFolderResponse {
        folder,
        status: "watching".to_string(),
    })
}
