//! Keeps slow work off the UI thread.
//!
//! Plain `fn` commands run on the main thread, so a slow one freezes the
//! window. Slow commands are `async` and hand their work to a worker thread
//! through these helpers, so the window keeps answering while they run.

use crate::app::error::AppError;
use crate::app::state::AppState;
use tauri::State;

/// Runs blocking work on a worker thread and waits for its result without
/// holding the UI thread or the async runtime.
pub async fn run_blocking<T, F>(work: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    match rx.await {
        Ok(result) => result,
        Err(_) => Err(AppError::internal(
            "A background task stopped unexpectedly.",
        )),
    }
}

/// Runs `work` on a worker thread with its own copy of the app state. The copy
/// holds only shared handles, so making it is cheap.
pub async fn with_app_state<T, F>(state: &State<'_, AppState>, work: F) -> Result<T, AppError>
where
    F: FnOnce(AppState) -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    let shared: &AppState = state;
    let owned = shared.clone();
    run_blocking(move || work(owned)).await
}

#[cfg(test)]
mod tests {
    use super::run_blocking;
    use crate::app::error::AppError;

    #[test]
    fn run_blocking_returns_the_work_result() {
        let value = futures::executor::block_on(run_blocking(|| Ok(21 * 2)));
        assert_eq!(value.unwrap(), 42);

        let failed = futures::executor::block_on(run_blocking(|| {
            Err::<i32, AppError>(AppError::invalid_input("nope"))
        }));
        assert!(failed.is_err());
    }
}
