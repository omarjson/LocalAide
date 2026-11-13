pub mod context;
pub mod prompt;
pub mod sidecar;

use crate::app::error::AppError;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct InferenceToken {
    pub token: String,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct GenerationMetrics {
    pub total_tokens: usize,
    pub ttft: Option<Duration>,
    pub tokens_per_sec: f64,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
pub struct EngineStatus {
    pub loaded: bool,
    pub active_model: Option<String>,
    pub context_size: i32,
}

/// One chat turn. `images` holds `data:image/...;base64,` URLs and is empty for text-only turns.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
    pub images: Vec<String>,
}

pub trait InferenceEngine: Send + Sync {
    fn load(&self, model_path: &Path, context_size: i32) -> Result<String, AppError>;
    fn unload(&self) -> Result<(), AppError>;
    fn generate(
        &self,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError>;
    fn status(&self) -> EngineStatus;
}
