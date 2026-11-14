use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::document::{
    IndexFolderResponse, IndexerStatsResponse, SearchResultDoc, WatchFolderResponse,
};
use crate::services::library::search::SearchMode;
use std::path::Path;
use tauri::State;

// Commands that index files, search, or start a sidecar are async and run their
// work through `with_app_state`, so the window keeps answering while they run.

#[tauri::command]
pub async fn index_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<SearchResultDoc, AppError> {
    with_app_state(&state, move |state| state.library.index_file(Path::new(&path))).await
}

#[tauri::command]
pub async fn index_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<IndexFolderResponse, AppError> {
    with_app_state(&state, move |state| {
        let (indexed, skipped) = state.library.index_folder(Path::new(&folder), None)?;
        Ok(IndexFolderResponse { indexed, skipped })
    })
    .await
}

#[tauri::command]
pub async fn search_index(
    state: State<'_, AppState>,
    query: String,
    mode: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SearchResultDoc>, AppError> {
    with_app_state(&state, move |state| {
        let search_mode = match mode.as_deref() {
            Some("semantic") => SearchMode::Semantic,
            Some("keyword") => SearchMode::Keyword,
            _ => SearchMode::Hybrid,
        };
        // Semantic and hybrid embed the query against the local engine when it
        // is running; with no engine the searcher degrades to keyword ranking.
        let query_vector = if search_mode == SearchMode::Keyword {
            None
        } else {
            state.embedding_engine.embed_query(&query).ok()
        };
        let limit_num = limit.unwrap_or(20);
        // The reranker second stage (when its engine is running) is applied
        // inside LibraryService::search, so the Search screen, memory search,
        // and chat RAG all get the same re-ranked results.
        state
            .library
            .search(&query, query_vector.as_deref(), search_mode, limit_num)
    })
    .await
}

/// Start the reranker engine on an installed reranker model. `model_id`
/// picks a specific one; None picks the smallest installed.
#[tauri::command]
pub async fn rerank_start(
    state: State<'_, AppState>,
    model_id: Option<String>,
) -> Result<crate::services::library::rerank::RerankState, AppError> {
    with_app_state(&state, move |state| {
        state.rerank_engine.start(model_id.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn rerank_stop(state: State<'_, AppState>) -> Result<(), AppError> {
    with_app_state(&state, |state| {
        state.rerank_engine.stop();
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn rerank_status(
    state: State<'_, AppState>,
) -> Result<crate::services::library::rerank::RerankState, AppError> {
    Ok(state.rerank_engine.state())
}

/// Start the embedding engine on an installed embedding model. `model_id`
/// picks a specific one; None picks the smallest installed.
#[tauri::command]
pub async fn embedding_start(
    state: State<'_, AppState>,
    model_id: Option<String>,
) -> Result<crate::services::library::embedding::EmbeddingState, AppError> {
    with_app_state(&state, move |state| {
        state.embedding_engine.start(model_id.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn embedding_stop(state: State<'_, AppState>) -> Result<(), AppError> {
    with_app_state(&state, |state| {
        state.embedding_engine.stop();
        Ok(())
    })
    .await
}

/// Fill in stored vectors for chunks indexed while the engine was off.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackfillResponse {
    pub updated: usize,
    pub total_chunks: usize,
    pub embedded_chunks: usize,
}

#[tauri::command]
pub async fn embedding_backfill(state: State<'_, AppState>) -> Result<BackfillResponse, AppError> {
    with_app_state(&state, |state| {
        let updated = state.library.backfill_embeddings(64)?;
        let (total, embedded) = state.library.embedding_coverage()?;
        Ok(BackfillResponse {
            updated,
            total_chunks: total,
            embedded_chunks: embedded,
        })
    })
    .await
}

#[tauri::command]
pub async fn list_index(state: State<'_, AppState>) -> Result<Vec<SearchResultDoc>, AppError> {
    with_app_state(&state, |state| state.library.list()).await
}

#[tauri::command]
pub async fn indexer_stats(state: State<'_, AppState>) -> Result<IndexerStatsResponse, AppError> {
    with_app_state(&state, |state| state.library.stats()).await
}

#[tauri::command]
pub async fn index_remove(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    with_app_state(&state, move |state| {
        state.library.remove(&path)?;
        Ok(())
    })
    .await
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

#[tauri::command]
pub fn index_watch_stop(state: State<'_, AppState>) -> Result<(), AppError> {
    state.library.stop_watch();
    Ok(())
}
