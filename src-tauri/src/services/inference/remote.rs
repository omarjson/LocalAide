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

/// Injected source of a replacement key after a 401/429. The router supplies
/// one backed by its credential pool; tests can inject a pure closure. It is
/// called with the HTTP status and returns the next key, or `None` when no
/// key remains. No I/O is performed by the engine itself in `rotated_key`.
pub type KeySupplier = Arc<dyn Fn(u16) -> Option<String> + Send + Sync>;

pub struct RemoteEngine {
    target: Mutex<Option<RemoteTarget>>,
    client: reqwest::blocking::Client,
    key_supplier: Mutex<Option<KeySupplier>>,
}

impl RemoteEngine {
    pub fn new() -> Self {
        // No idle-read timeout exists on the blocking client — reqwest has
        // read_timeout only on the async builder. A provider that stalls
        // mid-stream therefore parks the generation thread until CHAT_TIMEOUT;
        // the cancel flag is still checked between SSE lines, so the UI
        // recovers immediately when the user presses Stop.
        // Redirects are not followed: a redirect would send the conversation
        // and the API key to an address the user never entered.
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(CHAT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            target: Mutex::new(None),
            client,
            key_supplier: Mutex::new(None),
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

    /// Install the callback that supplies a replacement key on 401/429.
    /// The router installs one backed by its credential pool; callers that
    /// do not want rotation can leave it unset. Safe to replace at any time.
    pub fn set_key_supplier(&self, supplier: KeySupplier) {
        *self.key_supplier.lock().unwrap_or_else(|e| e.into_inner()) = Some(supplier);
    }

    /// One generation against the provider's `/chat/completions` endpoint,
    /// streaming SSE exactly like the sidecar does. A 401/429 is retried once
    /// with the next key from the injected supplier; if none is available the
    /// original, hint-bearing error is returned unchanged.
    pub fn generate(
        &self,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        mut on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError> {
        let mut target = {
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

        match self.send_once(
            &target,
            messages,
            max_tokens,
            temperature,
            cancel_flag.clone(),
            &mut *on_token,
        ) {
            Ok(metrics) => Ok(metrics),
            Err((status, error)) if status == 401 || status == 429 => {
                let supplier = self
                    .key_supplier
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let replacement = supplier
                    .as_ref()
                    .and_then(|s| rotated_key(&target.api_key, status, s.as_ref()));
                match replacement {
                    // Retry once with the next key. A second failure is
                    // reported with its own error hint, never streamed as an
                    // answer.
                    Some(key) => {
                        target.api_key = key;
                        self.send_once(
                            &target,
                            messages,
                            max_tokens,
                            temperature,
                            cancel_flag,
                            &mut *on_token,
                        )
                        .map_err(|(_, error)| error)
                    }
                    // No key remains (or no supplier was installed): preserve
                    // the existing 401/429 error and its plain-language hint.
                    None => Err(error),
                }
            }
            Err((_, error)) => Err(error),
        }
    }

    /// A single HTTP attempt. The `u16` in the error is the provider's HTTP
    /// status when the request was declined (401/429 and other non-2xx), or 0
    /// for transport, stream and cancellation failures. It lets `generate`
    /// decide about key rotation without parsing error text.
    fn send_once<F>(
        &self,
        target: &RemoteTarget,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        on_token: &mut F,
    ) -> Result<GenerationMetrics, (u16, AppError)>
    where
        F: FnMut(InferenceToken) + ?Sized,
    {
        let url = format!("{}/chat/completions", target.base_url.trim_end_matches('/'));
        let body = json!({
            "model": target.model,
            "messages": messages.iter().map(super::sidecar::wire_message).collect::<Vec<Value>>(),
            "max_tokens": max_tokens,
            "temperature": temperature,
            "stream": true
        });

        let start = Instant::now();
        let audit = crate::services::cloud_audit::start(
            &target.label,
            &url,
            &target.model,
            "chat",
            &crate::services::cloud_audit::last_user_text(&body),
            body.to_string().len(),
            Some(&target.api_key),
        );
        let mut request = self.client.post(&url).json(&body);
        // Keyless providers (Pollinations, local routers) get no
        // Authorization header at all, rather than an empty bearer token.
        if !target.api_key.trim().is_empty() {
            request = request.bearer_auth(&target.api_key);
        }
        let resp = match request.send() {
            Ok(r) => {
                let status = r.status();
                audit.finish(status.is_success(), &format!("HTTP {}", status.as_u16()));
                r
            }
            Err(e) => {
                audit.finish(false, "could not connect");
                return Err((
                    0,
                    AppError::internal(format!(
                        "Couldn't reach {} — check your connection and try again.",
                        target.label
                    ))
                    .with_detail(e.to_string()),
                ));
            }
        };

        if !resp.status().is_success() {
            let code = resp.status();
            let text = resp.text().unwrap_or_default();
            // Plain words first, same helper as the streaming path, so the
            // chat shows the same sentence whichever route an online turn
            // took. The status number itself is not shown.
            return Err((
                code.as_u16(),
                AppError::internal(super::openai_stream::http_error_message(
                    &target.label,
                    code.as_u16(),
                    &text,
                    !target.api_key.trim().is_empty(),
                )),
            ));
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
            let line =
                line.map_err(|e| (0, AppError::internal(format!("stream read failed: {e}"))))?;
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
                return Err((
                    0,
                    AppError::internal(format!(
                        "{label} streamed an error: {err}",
                        label = target.label
                    )),
                ));
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
            return Err((
                0,
                AppError::internal(format!(
                    "{label} closed the stream before completion (received {total_tokens} tokens).",
                    label = target.label
                )),
            ));
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

/// Pure rotation decision, shared by the engine and its tests. Returns the
/// supplier's key only for a retryable status when that key actually differs
/// from the one that just failed; never performs I/O itself.
pub fn rotated_key<S>(current: &str, status: u16, supplier: &S) -> Option<String>
where
    S: Fn(u16) -> Option<String> + ?Sized,
{
    if status != 401 && status != 429 {
        return None;
    }
    supplier(status).filter(|next| next != current && !next.trim().is_empty())
}
