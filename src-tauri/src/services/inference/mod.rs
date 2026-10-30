pub mod context;
pub mod llama;
pub mod prompt;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use crate::app::error::AppError;

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

pub trait InferenceEngine: Send + Sync {
    fn load(&self, model_path: &Path, context_size: i32) -> Result<String, AppError>;
    fn unload(&self) -> Result<(), AppError>;
    fn generate(
        &self,
        prompt: &str,
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError>;
    fn status(&self) -> EngineStatus;
}
