//! Batch transcription with online providers: the whole recording goes in
//! one request with the user's key. Every call is in the online log.

use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::services::cloud_audit;

fn client() -> Result<reqwest::blocking::Client, AppError> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::internal(e.to_string()))
}

fn failed(provider: &str, status: u16, body: &str) -> AppError {
    let hint = match status {
        401 | 403 => " The key looks wrong or has no access to this model.",
        404 => " The model was not found; it may have been renamed.",
        429 => " Too many requests or no credit left.",
        _ => "",
    };
    let body: String = cloud_audit::redact(body, &[]).chars().take(300).collect();
    AppError::new(
        "VOICE_ONLINE_FAILED",
        format!("{provider} returned {status}.{hint} {}", body.trim()),
    )
}

/// Transcribe a 16 kHz mono WAV. Returns the text.
pub fn transcribe(
    provider: &str,
    model: &str,
    key: &str,
    wav: &[u8],
    seconds: f32,
) -> Result<String, AppError> {
    if key.trim().is_empty() {
        return Err(AppError::invalid_input(
            "Add your key for this provider in Settings → Voice first.",
        ));
    }
    let key = key.trim();
    let what = format!("Recorded audio, {seconds:.1} s");
    match provider {
        "groq" | "openai" => {
            let url = if provider == "groq" {
                "https://api.groq.com/openai/v1/audio/transcriptions"
            } else {
                "https://api.openai.com/v1/audio/transcriptions"
            };
            let label = if provider == "groq" { "Groq" } else { "OpenAI" };
            let audit = cloud_audit::start(label, url, model, "voice", &what, wav.len(), Some(key));
            let part = reqwest::blocking::multipart::Part::bytes(wav.to_vec())
                .file_name("speech.wav")
                .mime_str("audio/wav")
                .map_err(|e| AppError::internal(e.to_string()))?;
            let form = reqwest::blocking::multipart::Form::new()
                .text("model", model.to_string())
                .text("response_format", "json")
                .part("file", part);
            let resp = client()?.post(url).bearer_auth(key).multipart(form).send();
            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    audit.finish(false, "could not connect");
                    return Err(AppError::internal(format!("Could not reach {label}: {e}")));
                }
            };
            let status = resp.status().as_u16();
            let body = resp.text().unwrap_or_default();
            audit.finish(status == 200, &format!("HTTP {status}"));
            if status != 200 {
                return Err(failed(label, status, &body));
            }
            let v: Value =
                serde_json::from_str(&body).map_err(|e| AppError::internal(e.to_string()))?;
            Ok(v.get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string())
        }
        "deepgram" => {
            let url = format!(
                "https://api.deepgram.com/v1/listen?model={model}&smart_format=true&language=multi"
            );
            let audit = cloud_audit::start(
                "Deepgram",
                &url,
                model,
                "voice",
                &what,
                wav.len(),
                Some(key),
            );
            let resp = client()?
                .post(&url)
                .header("Authorization", format!("Token {key}"))
                .header("Content-Type", "audio/wav")
                .body(wav.to_vec())
                .send();
            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    audit.finish(false, "could not connect");
                    return Err(AppError::internal(format!("Could not reach Deepgram: {e}")));
                }
            };
            let status = resp.status().as_u16();
            let body = resp.text().unwrap_or_default();
            audit.finish(status == 200, &format!("HTTP {status}"));
            if status != 200 {
                return Err(failed("Deepgram", status, &body));
            }
            let v: Value =
                serde_json::from_str(&body).map_err(|e| AppError::internal(e.to_string()))?;
            Ok(v.pointer("/results/channels/0/alternatives/0/transcript")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string())
        }
        "gemini" => {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            );
            let audit = cloud_audit::start(
                "Google Gemini",
                &url,
                model,
                "voice",
                &what,
                wav.len(),
                Some(key),
            );
            let body = json!({
                "contents": [{"role": "user", "parts": [{"inline_data": {
                    "mime_type": "audio/wav",
                    "data": base64::engine::general_purpose::STANDARD.encode(wav),
                }}]}]
            });
            let resp = client()?
                .post(&url)
                .header("x-goog-api-key", key)
                .json(&body)
                .send();
            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    audit.finish(false, "could not connect");
                    return Err(AppError::internal(format!("Could not reach Google: {e}")));
                }
            };
            let status = resp.status().as_u16();
            let text = resp.text().unwrap_or_default();
            audit.finish(status == 200, &format!("HTTP {status}"));
            if status != 200 {
                return Err(failed("Google Gemini", status, &text));
            }
            let v: Value =
                serde_json::from_str(&text).map_err(|e| AppError::internal(e.to_string()))?;
            Ok(gemini_text(&v))
        }
        _ => Err(AppError::invalid_input(
            "That voice provider is not supported.",
        )),
    }
}

/// The transcript in a generateContent reply: every text part, joined.
pub fn gemini_text(v: &Value) -> String {
    v.pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_gemini_replies() {
        let v = json!({"candidates": [{"content": {"parts": [{"text": "Hello "}, {"text": "world."}]}}]});
        assert_eq!(gemini_text(&v), "Hello world.");
        assert_eq!(gemini_text(&json!({})), "");
    }

    #[test]
    fn a_missing_key_is_refused_before_any_call() {
        let err = transcribe("groq", "whisper-large-v3-turbo", " ", b"RIFF", 1.0).unwrap_err();
        assert!(err.message.contains("key"));
    }
}
