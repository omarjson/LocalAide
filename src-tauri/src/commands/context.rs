//! Tauri commands for workspace instruction files and `@path` references.

use std::path::PathBuf;

use crate::app::blocking::run_blocking;
use crate::app::error::AppError;
use crate::services::context_files::{
    self, ContextFilesResult, DEFAULT_MAX_BYTES as FILES_MAX_BYTES, HARD_MAX_BYTES,
};
use crate::services::context_refs::{self, DEFAULT_MAX_BYTES as REFS_MAX_BYTES, RefResult};

/// Find AGENTS.md / CLAUDE.md / .hermes.md / .cursorrules in the workspace,
/// the library's indexed folders and any extra `roots` supplied by the
/// window, then concatenate them within a byte budget.
#[tauri::command]
pub async fn context_files_list(
    roots: Option<Vec<String>>,
    max_bytes: Option<usize>,
) -> Result<ContextFilesResult, AppError> {
    run_blocking(move || {
        let mut paths = vec![crate::app::paths::workspace_dir()];
        paths.extend(context_files::indexed_roots(
            &crate::app::paths::index_dir().join("library.db"),
        ));
        if let Some(extra) = roots {
            paths.extend(extra.into_iter().map(PathBuf::from));
        }
        let cap = max_bytes
            .unwrap_or(FILES_MAX_BYTES)
            .clamp(1024, HARD_MAX_BYTES);
        context_files::collect(&paths, cap)
    })
    .await
}

/// Resolve `@relative/path[:start-end]` to a bounded slice of a file inside
/// the workspace or the supplied roots.
#[tauri::command]
pub async fn context_refs_resolve(
    reference: String,
    roots: Option<Vec<String>>,
    max_bytes: Option<usize>,
) -> Result<RefResult, AppError> {
    run_blocking(move || {
        let mut paths = vec![crate::app::paths::workspace_dir()];
        if let Some(extra) = roots {
            paths.extend(extra.into_iter().map(PathBuf::from));
        }
        let cap = max_bytes
            .unwrap_or(REFS_MAX_BYTES)
            .clamp(1024, HARD_MAX_BYTES);
        context_refs::resolve(&reference, &paths, cap)
    })
    .await
}
