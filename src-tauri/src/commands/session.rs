//! Tauri commands for local session search, reindexing and multi-run.

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::multi_run::{self, RunAnswer, RunTarget};
use crate::services::session_index::{self, SessionHit};

/// Search saved chat threads. The index is created on demand if absent (it
/// will be empty until `session_reindex` runs).
#[tauri::command]
pub async fn session_search(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SessionHit>, AppError> {
    run_blocking(move || {
        session_index::search(
            &session_index::default_db_path(),
            &query,
            limit.unwrap_or(20),
        )
    })
    .await
}

/// Rebuild the thread index from the conversations folder. Returns the
/// number of threads indexed.
#[tauri::command]
pub async fn session_reindex() -> Result<usize, AppError> {
    run_blocking(move || {
        session_index::index_all(
            &session_index::default_db_path(),
            &crate::app::paths::conversations_dir(),
        )
    })
    .await
}

/// Send one prompt to several named OpenAI-compatible endpoints at once.
/// Lives here because there is no dedicated command file in this stream;
/// all connection details come from `targets`, never from config.
#[tauri::command]
pub async fn multi_run_chat(
    targets: Vec<RunTarget>,
    prompt: String,
) -> Result<Vec<RunAnswer>, AppError> {
    run_blocking(move || multi_run::run(targets, &prompt)).await
}
