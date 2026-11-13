//! Chat engine that streams from a user-configured OpenAI-compatible provider.
//!
//! This is the opt-in online path. It is only reachable when the user enables
//! cloud mode in Settings and configures a provider with their own API key.
//! Requests go to the provider's base URL over HTTPS (or to a localhost proxy
//! over HTTP); nothing is sent anywhere else.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{GenerationMetrics, InferenceToken, PromptMessage};
use crate::app::error::AppError;

/// Whole-request timeout for chat. Generation can run for minutes on long answers.
const CHAT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// Context assumed for the budget planner when no local model sets a limit.
pub const REMOTE_CONTEXT_SIZE: i32 = 8192;

/// The provider target the engine generates against. Built from the user's
/// config when cloud mode is switched on. Deliberately not `Debug`: it carries
/// the API key and must never be printable by a stray `{target:?}`.
#[derive(Clone, PartialEq)]
pub struct RemoteTarget {
    pub label: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

pub struct RemoteEngine {
    target: Mutex<Option<RemoteTarget>>,
    client: reqwest::blocking::Client,
}

impl RemoteEngine {
    pub fn new() -> Self {
        // No idle-read timeout exists on the blocking client — reqwest has
        // read_timeout only on the async builder. A provider that stalls
        // mid-stream therefore parks the generation thread until CHAT_TIMEOUT;
        // the cancel flag is still checked between SSE lines, so the UI
        // recovers immediately when the user presses Stop.
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(CHAT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            target: Mutex::new(None),
            client,
        }
    }

    /// Point the engine at a provider. Replaces any previous target.
    pub fn configure(&self, target: RemoteTarget) {
        let mut guard = self.target.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(target);
    }

    /// Forget the provider. Cloud mode then has no engine, like local mode
    /// with no model loaded.
    pub fn clear(&self) {
        let mut guard = self.target.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }

    /// The active target, if cloud mode is configured.
    pub fn target(&self) -> Option<RemoteTarget> {
        self.target
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// One generation against the provider's `/chat/completions` endpoint,
    /// streaming SSE exactly like the sidecar does.
    pub fn generate(
        &self,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        mut on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError> {
        let target = {
            let guard = self.target.lock().unwrap_or_else(|e| e.into_inner());
            guard
                .as_ref()
                .ok_or_else(|| {
                    AppError::invalid_input(
                        "No online provider is configured. Add one in Settings, or load a local model.",
                    )
                })?
                .clone()
        };

        let url = format!("{}/chat/completions", target.base_url.trim_end_matches('/'));
        let body = json!({
            "model": target.model,
            "messages": messages.iter().map(super::sidecar::wire_message).collect::<Vec<Value>>(),
            "max_tokens": max_tokens,
            "temperature": temperature,
            "stream": true
        });

        let start = Instant::now();
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&target.api_key)
            .json(&body)
            .send()
            .map_err(|e| {
                AppError::internal(format!(
                    "Could not reach {label}: {e}. Check the base URL and your connection.",
                    label = target.label
                ))
            })?;

        if !resp.status().is_success() {
            let code = resp.status();
            let text = resp.text().unwrap_or_default();
            let hint = if code.as_u16() == 401 || code.as_u16() == 403 {
                " The API key looks wrong or unauthorized."
            } else {
                ""
            };
            return Err(AppError::internal(format!(
                "{label} returned {code}:{hint} {text}",
                label = target.label
            )));
        }

        let mut total_tokens = 0usize;
        let mut ttft: Option<Duration> = None;
        let mut cancelled = false;
        let mut seen_done = false;

        for line in BufReader::new(resp).lines() {
            if cancel_flag.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let line = line.map_err(|e| AppError::internal(format!("stream read failed: {e}")))?;
            // The SSE spec allows the space after "data:" to be omitted.
            let data = match line.strip_prefix("data:") {
                Some(d) => d.strip_prefix(' ').unwrap_or(d),
                None => continue,
            };
            if data.trim() == "[DONE]" {
                seen_done = true;
                break;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            // Providers signal mid-stream failures as error events. One
            // truncated answer presented as complete is worse than an error.
            if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
                return Err(AppError::internal(format!(
                    "{label} streamed an error: {err}",
                    label = target.label
                )));
            }
            let Some(text) = v["choices"][0]["delta"]["content"].as_str() else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            if ttft.is_none() {
                ttft = Some(start.elapsed());
            }
            on_token(InferenceToken {
                token: text.to_string(),
                index: total_tokens,
            });
            total_tokens += 1;
        }

        // A clean close without [DONE] means the answer was cut short. Say so
        // instead of showing a silently truncated reply as finished.
        if !cancelled && !seen_done {
            return Err(AppError::internal(format!(
                "{label} closed the stream before completion (received {total_tokens} tokens).",
                label = target.label
            )));
        }

        let elapsed = start.elapsed().as_secs_f64();
        let tokens_per_sec = if elapsed > 0.0 {
            total_tokens as f64 / elapsed
        } else {
            0.0
        };

        Ok(GenerationMetrics {
            total_tokens,
            ttft,
            tokens_per_sec,
            cancelled,
        })
    }
}

impl Default for RemoteEngine {
    fn default() -> Self {
        Self::new()
    }
}
