use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Error)]
#[error("[{code}] {message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
            retryable: false,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn model_not_installed(id: &str) -> Self {
        Self::new("MODEL_NOT_INSTALLED", format!("Model '{id}' is not installed."))
    }

    pub fn model_load_failed(id: &str, reason: &str) -> Self {
        Self::new("MODEL_LOAD_FAILED", format!("Failed to load model '{id}': {reason}"))
            .with_detail(reason)
    }

    pub fn checksum_mismatch(expected: &str, actual: &str) -> Self {
        Self::new("CHECKSUM_MISMATCH", "Cryptographic integrity verification failed.")
            .with_detail(format!("Expected {expected}, got {actual}"))
    }

    pub fn sandbox_denied(path: &str, reason: &str) -> Self {
        Self::new("SANDBOX_DENIED", format!("Filesystem access denied for '{path}'"))
            .with_detail(reason)
    }

    pub fn db_busy(reason: &str) -> Self {
        Self::new("DB_BUSY", "Database is currently busy.")
            .with_detail(reason)
            .retryable()
    }

    pub fn cancelled() -> Self {
        Self::new("CANCELLED", "The operation was cancelled.")
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::new("INVALID_INPUT", msg)
    }

    pub fn storage(msg: impl Into<String>) -> Self {
        Self::new("STORAGE_ERROR", msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new("INTERNAL", msg)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::internal(err.to_string())
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        Self::internal(msg)
    }
}
