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
use crate::config::{AppConfig, CredentialPool};

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
    /// API keys for the active remote provider, with a rotation cursor. An
    /// Arc so the remote engine's rotation hook can share it without keeping
    /// a reference to the router (and without any ownership cycle).
    credentials: Arc<Mutex<Option<CredentialPool>>>,
    /// Provider ids in failover order, from config.
    provider_order: Mutex<Vec<String>>,
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
        // The rotation hook shares the pool with the router through an Arc:
        // it captures no `&self`, so there is no ownership cycle. A 401/429
        // from a real RemoteEngine request now advances the cursor and the
        // engine retries once with the next key.
        let credentials = Arc::new(Mutex::new(None::<CredentialPool>));
        let remote = RemoteEngine::new();
        let pool = credentials.clone();
        remote.set_key_supplier(Arc::new(move |status| {
            let mut guard = pool.lock().unwrap_or_else(|e| e.into_inner());
            match guard.as_mut() {
                Some(keys) => {
                    if keys.on_auth_failure(status) {
                        keys.current().map(str::to_string)
                    } else {
                        None
                    }
                }
                None => None,
            }
        }));
        Self {
            local: SidecarEngine::new(),
            remote,
            mode: Mutex::new(EngineMode::Local),
            chat_client,
            credentials,
            provider_order: Mutex::new(Vec::new()),
        }
    }

    pub fn chat_client(&self) -> &reqwest::blocking::Client {
        &self.chat_client
    }

    /// Apply the routing settings from config. `provider_id` selects whose
    /// key pool to use; pass None to clear the pool. Safe to call at any time.
    pub fn apply_routing(&self, cfg: &AppConfig, provider_id: Option<&str>) {
        *self
            .provider_order
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = cfg.provider_order();
        let pool = provider_id
            .map(|id| cfg.credentials_for(id))
            .filter(|pool| !pool.is_empty());
        *self.credentials.lock().unwrap_or_else(|e| e.into_inner()) = pool;
    }

    /// Provider ids in failover order.
    pub fn provider_order(&self) -> Vec<String> {
        self.provider_order
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The key to send with the next request, when a pool is configured.
    pub fn current_key(&self) -> Option<String> {
        self.credentials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|pool| pool.current().map(str::to_string))
    }

    /// Move to the next key after a 401 or 429. Returns true when rotated.
    pub fn rotate_key(&self, status: u16) -> bool {
        self.credentials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
            .is_some_and(|pool| pool.on_auth_failure(status))
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
        let key = self.current_key();
        self.endpoint_for_with_key(target, key.as_deref())
    }

    /// Same as `endpoint_for`, but with an explicit key override. Used by the
    /// rotation hook so a 401/429 can retry with the next key in the pool.
    pub fn endpoint_for_with_key(
        &self,
        target: &ChatTarget,
        key_override: Option<&str>,
    ) -> Result<Endpoint, AppError> {
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
                let key = key_override
                    .filter(|k| !k.trim().is_empty())
                    .unwrap_or(&t.api_key);
                Ok(Endpoint {
                    url: format!("{}/chat/completions", t.base_url.trim_end_matches('/')),
                    bearer: Some(key.to_string()).filter(|k| !k.trim().is_empty()),
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
        *self.credentials.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
        *self.credentials.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = EngineMode::Cloud;
    }

    /// Switch back to the local engine. A model must be loaded before chat works.
    pub fn set_local(&self) {
        *self.credentials.lock().unwrap_or_else(|e| e.into_inner()) = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn routing_pool_rotates_keys() {
        let engine = RouterEngine::new();
        let mut cfg = AppConfig::default();
        cfg.routing.provider_priority = vec!["p".into()];
        cfg.routing.key_pools = BTreeMap::from([("p".into(), vec!["a".into(), "b".into()])]);
        engine.apply_routing(&cfg, Some("p"));
        assert_eq!(engine.provider_order(), vec!["p"]);
        assert_eq!(engine.current_key().as_deref(), Some("a"));
        assert!(engine.rotate_key(401));
        assert_eq!(engine.current_key().as_deref(), Some("b"));
        assert!(!engine.rotate_key(200));
        assert_eq!(engine.current_key().as_deref(), Some("b"));
        assert!(engine.rotate_key(429));
        assert_eq!(engine.current_key().as_deref(), Some("a"));
    }
}
