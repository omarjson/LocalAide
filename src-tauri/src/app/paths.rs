use crate::app::error::AppError;
use std::path::{Path, PathBuf};

pub fn app_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".aiassistant")
}

pub fn models_dir() -> PathBuf {
    app_dir().join("models")
}

pub fn index_dir() -> PathBuf {
    app_dir().join("index")
}

pub fn memory_dir() -> PathBuf {
    app_dir().join("memory")
}

pub fn workspace_dir() -> PathBuf {
    app_dir().join("workspace")
}

pub fn conversations_dir() -> PathBuf {
    app_dir().join("conversations")
}

pub fn config_path() -> PathBuf {
    app_dir().join("config.json")
}

/// Validate that a path is securely contained within one of the allowed roots
/// without symlink escapes or directory traversal.
pub fn validate_sandboxed_path(
    target: &Path,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, AppError> {
    let canonical = target.canonicalize().map_err(|e| {
        AppError::sandbox_denied(
            &target.to_string_lossy(),
            &format!("Failed to canonicalize path: {e}"),
        )
    })?;

    for root in allowed_roots {
        if let Ok(canonical_root) = root.canonicalize() {
            if canonical.starts_with(&canonical_root) {
                return Ok(canonical);
            }
        }
    }

    Err(AppError::sandbox_denied(
        &canonical.to_string_lossy(),
        "Target path is outside allowed sandbox directories.",
    ))
}
