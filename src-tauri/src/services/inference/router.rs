//! Engine router: picks the local sidecar or the opt-in cloud engine.
//!
//! The rest of the app talks to one `InferenceEngine`. The router forwards to
//! the sidecar by default and to the remote engine only after the user enables
//! cloud mode in Settings. Loading a model always returns to the local engine.

use std::path::Path;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::time::Duration;

use super::openai_stream::{Endpoint, Flavor};
use super::remote::{RemoteEngine, RemoteTarget};
use super::sidecar::SidecarEngine;
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
    /// One client for tool-using chat, local and online. Redirects are not
    /// followed: a redirect would carry the conversation and the key to an
    /// address the user never entered.
    chat_client: reqwest::blocking::Client,
}

/// Which engine answers one message: the local model, or a model of an
/// online provider the user chose in the picker.
#[derive(Clone)]
pub enum ChatTarget {
    Local,
    Remote(RemoteTarget),
}

impl RouterEngine {
    pub fn new() -> Self {
        let chat_client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60 * 60))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            local: SidecarEngine::new(),
            remote: RemoteEngine::new(),
            mode: Mutex::new(EngineMode::Local),
            chat_client,
        }
    }

    pub fn chat_client(&self) -> &reqwest::blocking::Client {
        &self.chat_client
    }

    /// The local server, for its address and `/props`.
    pub fn local(&self) -> &SidecarEngine {
        &self.local
    }

    /// The default target: what the picker last chose.
    pub fn current_target(&self) -> ChatTarget {
        match (self.mode(), self.cloud_target()) {
            (EngineMode::Cloud, Some(t)) => ChatTarget::Remote(t),
            _ => ChatTarget::Local,
        }
    }

    /// Where a chat request for `target` goes.
    pub fn endpoint_for(&self, target: &ChatTarget) -> Result<Endpoint, AppError> {
        match target {
            ChatTarget::Local => {
                let ep = self.local.endpoint().ok_or_else(|| {
                    AppError::model_not_installed(
                        "No model is loaded. Pick a model in the model menu.",
                    )
                })?;
                Ok(Endpoint {
                    url: format!("http://127.0.0.1:{}/v1/chat/completions", ep.port),
                    bearer: Some(ep.api_key),
                    label: "The local model".to_string(),
                    flavor: Flavor::LlamaServer,
                    model: String::new(),
                    require_done: false,
                })
            }
            ChatTarget::Remote(t) => {
                crate::config::validate_base_url(&t.base_url).map_err(AppError::invalid_input)?;
                Ok(Endpoint {
                    url: format!("{}/chat/completions", t.base_url.trim_end_matches('/')),
                    bearer: Some(t.api_key.clone()).filter(|k| !k.trim().is_empty()),
                    label: t.label.clone(),
                    flavor: Flavor::for_base_url(&t.base_url),
                    model: t.model.clone(),
                    require_done: true,
                })
            }
        }
    }

    pub fn mode(&self) -> EngineMode {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Switch chat to the user's cloud provider.
    pub fn set_cloud_target(&self, target: RemoteTarget) {
        self.remote.configure(target);
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = EngineMode::Cloud;
        // The local model stays loaded on purpose: switching back is instant,
        // an in-flight answer is not killed mid-stream, and the Settings copy
        // promises a return to local inference. RAM is reclaimed when a
        // different model is loaded next.
    }

    /// Switch the router to cloud mode without configuring a target yet.
    /// Used when the UI toggle is on but no provider is selected: chat then
    /// reports "no model" instead of silently falling back to the local one.
    pub fn set_cloud_mode(&self) {
        self.remote.clear();
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = EngineMode::Cloud;
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
            EngineMode::Cloud => {
                self.remote
                    .generate(messages, max_tokens, temperature, cancel_flag, on_token)
            }
            EngineMode::Local => {
                self.local
                    .generate(messages, max_tokens, temperature, cancel_flag, on_token)
            }
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
