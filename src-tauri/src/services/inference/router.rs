//! Engine router: picks the local sidecar or the opt-in cloud engine.
//!
//! The rest of the app talks to one `InferenceEngine`. The router forwards to
//! the sidecar by default and to the remote engine only after the user enables
//! cloud mode in Settings. Loading a model always returns to the local engine.

use std::path::Path;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

use super::sidecar::SidecarEngine;
use super::remote::{RemoteEngine, RemoteTarget};
use super::{EngineStatus, GenerationMetrics, InferenceEngine, InferenceToken, PromptMessage};
use crate::app::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Local,
    Cloud,
}

pub struct RouterEngine {
    local: SidecarEngine,
    remote: RemoteEngine,
    mode: Mutex<EngineMode>,
}

impl RouterEngine {
    pub fn new() -> Self {
        Self {
            local: SidecarEngine::new(),
            remote: RemoteEngine::new(),
            mode: Mutex::new(EngineMode::Local),
        }
    }

    pub fn mode(&self) -> EngineMode {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Switch chat to the user's cloud provider.
    pub fn set_cloud_target(&self, target: RemoteTarget) {
        self.remote.configure(target);
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = EngineMode::Cloud;
        // The sidecar holds RAM for the loaded model. Cloud mode does not need it.
        let _ = self.local.unload();
    }

    /// Switch back to the local engine. A model must be loaded before chat works.
    pub fn set_local(&self) {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = EngineMode::Local;
        self.remote.clear();
    }

    /// The cloud target, when cloud mode is active.
    pub fn cloud_target(&self) -> Option<RemoteTarget> {
        self.remote.target()
    }
}

impl Default for RouterEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InferenceEngine for RouterEngine {
    fn load(&self, model_path: &Path, context_size: i32) -> Result<String, AppError> {
        // Loading a model is an explicit local choice, whatever the last mode was.
        self.set_local();
        self.local.load(model_path, context_size)
    }

    fn unload(&self) -> Result<(), AppError> {
        self.local.unload()
    }

    fn generate(
        &self,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError> {
        match self.mode() {
            EngineMode::Cloud => self
                .remote
                .generate(messages, max_tokens, temperature, cancel_flag, on_token),
            EngineMode::Local => self
                .local
                .generate(messages, max_tokens, temperature, cancel_flag, on_token),
        }
    }

    fn status(&self) -> EngineStatus {
        match self.mode() {
            EngineMode::Cloud => match self.cloud_target() {
                Some(t) => EngineStatus {
                    loaded: true,
                    active_model: Some(format!("{} ({})", t.model, t.label)),
                    context_size: super::remote::REMOTE_CONTEXT_SIZE,
                },
                None => EngineStatus {
                    loaded: false,
                    active_model: None,
                    context_size: super::remote::REMOTE_CONTEXT_SIZE,
                },
            },
            EngineMode::Local => self.local.status(),
        }
    }
}
