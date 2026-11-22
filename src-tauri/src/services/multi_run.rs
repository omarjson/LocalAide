//! Run one prompt against several OpenAI-compatible endpoints at once.
//!
//! This module is deliberately config-free: every target (URL, model and
//! optional key) is supplied by the caller, so nothing here reads user
//! settings or silently contacts a provider they did not name.

use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app::error::AppError;

const MAX_TARGETS: usize = 16;
const MAX_PROMPT_BYTES: usize = 256 * 1024;
const TIMEOUT_SECS: u64 = 60;

/// One provider to query. `endpoint` is a base URL such as
/// `https://api.example.com/v1` or a full `/chat/completions` URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunTarget {
    pub label: String,
    pub endpoint: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// The answer (or error) from one target, tagged with its label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunAnswer {
    pub label: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn completion_url(endpoint: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn short_error(text: &str) -> String {
    text.chars().take(200).collect()
}

/// Perform one HTTP call. Errors are returned as data so a single offline
/// provider does not lose the answers from all the others.
fn call_one(target: RunTarget, prompt: String) -> RunAnswer {
    let result = (|| -> Result<String, String> {
        let url = completion_url(&target.endpoint);
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("Endpoint must be an http(s) URL.".to_string());
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Could not build the HTTP client: {e}"))?;

        let mut body = serde_json::json!({
            "model": target.model,
            "messages": [{ "role": "user", "content": prompt }],
            "stream": false,
        });
        if let Some(tokens) = target.max_tokens {
            body["max_tokens"] = serde_json::json!(tokens);
        }

        let mut request = client.post(&url).json(&body);
        if let Some(key) = target.api_key.as_deref() {
            if !key.is_empty() {
                request = request.bearer_auth(key);
            }
        }
        let response = request.send().map_err(|e| format!("Request failed: {e}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().unwrap_or_default();
            return Err(format!("HTTP {status}: {}", short_error(&detail)));
        }
        let value: serde_json::Value = response
            .json()
            .map_err(|e| format!("Invalid response: {e}"))?;
        let answer = value
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|choice| {
                choice
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .or_else(|| choice.get("text"))
            })
            .and_then(|content| content.as_str())
            .unwrap_or("")
            .to_string();
        Ok(answer)
    })();

    match result {
        Ok(text) => RunAnswer {
            label: target.label,
            text,
            error: None,
        },
        Err(error) => RunAnswer {
            label: target.label,
            text: String::new(),
            error: Some(error),
        },
    }
}

/// Send the same prompt to every target concurrently. Answers keep the order
/// of `targets`, regardless of which endpoint replies first.
pub fn run(targets: Vec<RunTarget>, prompt: &str) -> Result<Vec<RunAnswer>, AppError> {
    if targets.is_empty() {
        return Err(AppError::invalid_input("At least one target is required."));
    }
    if targets.len() > MAX_TARGETS {
        return Err(AppError::invalid_input(format!(
            "At most {MAX_TARGETS} targets can run at once."
        )));
    }
    if prompt.trim().is_empty() {
        return Err(AppError::invalid_input("The prompt must not be empty."));
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(AppError::invalid_input("The prompt is too large."));
    }

    let mut handles = Vec::with_capacity(targets.len());
    for target in targets {
        let owned_prompt = prompt.to_string();
        handles.push(thread::spawn(move || call_one(target, owned_prompt)));
    }

    let mut answers = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.join() {
            Ok(answer) => answers.push(answer),
            Err(_) => answers.push(RunAnswer {
                label: String::new(),
                text: String::new(),
                error: Some("A target worker stopped unexpectedly.".to_string()),
            }),
        }
    }
    Ok(answers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_url_normalises_base_urls() {
        assert_eq!(
            completion_url("https://api.example.com/v1/"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            completion_url("https://api.example.com/v1/chat/completions"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn run_rejects_empty_input() {
        assert!(run(Vec::new(), "hi").is_err());
        let targets = vec![RunTarget {
            label: "a".into(),
            endpoint: "https://example.invalid/v1".into(),
            model: "m".into(),
            api_key: None,
            max_tokens: None,
        }];
        assert!(run(targets, "   ").is_err());
    }
}
