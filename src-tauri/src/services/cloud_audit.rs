//! What left this computer: one row per outbound call to an online service
//! (chat, transcription, web search, page reads, model lists).
//!
//! Each row keeps when, which provider and host, which model, what kind of
//! content went out and a short preview of it. Previews are redacted: the
//! key used for the call, bearer tokens and common key shapes are replaced
//! before anything is stored, so the log never holds a secret.
//!
//! Calls to this computer (127.0.0.1, localhost) are not logged.

use std::sync::{Arc, OnceLock};

use crate::services::library::db::LibraryDb;

static DB: OnceLock<Arc<LibraryDb>> = OnceLock::new();

/// Longest preview kept, in characters.
pub const PREVIEW_CHARS: usize = 160;

/// Called once at startup.
pub fn install(db: Arc<LibraryDb>) {
    let _ = DB.set(db);
}

/// The host of a URL, or the URL itself when it does not parse.
pub fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.chars().take(80).collect())
}

/// True for addresses on this computer; those calls never leave it.
pub fn is_local(url: &str) -> bool {
    match url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
    {
        Some(h) => {
            h == "localhost"
                || h == "127.0.0.1"
                || h == "::1"
                || h == "[::1]"
                || h.ends_with(".localhost")
        }
        None => false,
    }
}

fn looks_like_key(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
    let prefixes = [
        "sk-",
        "sk_",
        "gsk_",
        "xai-",
        "AIza",
        "pk-",
        "rk-",
        "ghp_",
        "gho_",
        "github_pat_",
        "hf_",
        "dg_",
        "sess-",
        "Bearer",
    ];
    if w.len() >= 16 && prefixes.iter().any(|p| w.starts_with(p)) {
        return true;
    }
    // Long runs of key-like characters with both letters and digits.
    w.len() >= 32
        && w.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && w.chars().any(|c| c.is_ascii_digit())
        && w.chars().any(|c| c.is_ascii_alphabetic())
}

/// Remove secrets from text: every given secret, then anything shaped like
/// a key or bearer token.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for s in secrets.iter().map(|s| s.trim()).filter(|s| s.len() >= 6) {
        out = out.replace(s, "[key removed]");
    }
    out.split_inclusive(char::is_whitespace)
        .map(|piece| {
            let word = piece.trim_end();
            if looks_like_key(word) {
                format!("[key removed]{}", &piece[word.len()..])
            } else {
                piece.to_string()
            }
        })
        .collect()
}

/// A short, redacted, single-line preview.
pub fn preview(text: &str, secrets: &[&str]) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = redact(&flat, secrets);
    let mut cut: String = clean.chars().take(PREVIEW_CHARS).collect();
    if cut.chars().count() < clean.chars().count() {
        cut.push('…');
    }
    cut
}

/// An open log row; `finish` records how the call ended.
pub struct Call(Option<i64>);

impl Call {
    pub fn finish(self, ok: bool, detail: &str) {
        if let (Some(id), Some(db)) = (self.0, DB.get()) {
            let detail: String = redact(detail, &[]).chars().take(200).collect();
            let _ = db.finish_cloud_call(id, if ok { "ok" } else { "failed" }, Some(&detail));
        }
    }
}

/// Record that a call is about to leave this computer. `content` is what
/// goes out (it is reduced to a redacted preview); `secret` is the key used,
/// so it can be removed from the preview too.
pub fn start(
    provider: &str,
    url: &str,
    model: &str,
    kind: &str,
    content: &str,
    bytes: usize,
    secret: Option<&str>,
) -> Call {
    if is_local(url) {
        return Call(None);
    }
    let Some(db) = DB.get() else {
        return Call(None);
    };
    let secrets: Vec<&str> = secret.into_iter().collect();
    let shown = preview(content, &secrets);
    match db.record_cloud_call(provider, &host_of(url), model, kind, &shown, bytes, "sent") {
        Ok(id) => Call(Some(id)),
        Err(e) => {
            log::warn!("Could not record an online call: {e}");
            Call(None)
        }
    }
}

/// The newest user text in an OpenAI-style request body, for the preview.
pub fn last_user_text(body: &serde_json::Value) -> String {
    let Some(messages) = body.get("messages").and_then(|m| m.as_array()) else {
        return String::new();
    };
    let Some(msg) = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
    else {
        return String::new();
    };
    match msg.get("content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_never_reach_the_preview() {
        let key = "sk-or-v1-0123456789abcdef0123456789abcdef";
        let p = preview(&format!("my key is {key} please"), &[key]);
        assert!(!p.contains("0123456789abcdef"), "{p}");
        assert!(p.contains("[key removed]"));
        let p = preview("Authorization: Bearer gsk_ABCDEFGHIJKLMNOP1234567890", &[]);
        assert!(!p.contains("gsk_ABCDEF"), "{p}");
        let p = preview("AIzaSyA1234567890abcdefghijklmnopqrstu is google", &[]);
        assert!(!p.contains("AIzaSy"), "{p}");
        let p = preview("plain words stay as they are", &[]);
        assert_eq!(p, "plain words stay as they are");
    }

    #[test]
    fn previews_are_short_and_flat() {
        let long = "word ".repeat(100);
        let p = preview(&long, &[]);
        assert!(p.chars().count() <= PREVIEW_CHARS + 1);
        assert!(p.ends_with('…'));
        assert_eq!(preview("a\n\nb\tc", &[]), "a b c");
    }

    #[test]
    fn local_calls_are_not_logged() {
        assert!(is_local("http://127.0.0.1:8080/v1/chat/completions"));
        assert!(is_local("http://localhost:11434/v1"));
        assert!(!is_local("https://openrouter.ai/api/v1"));
        assert_eq!(
            host_of("https://api.groq.com/openai/v1/audio"),
            "api.groq.com"
        );
    }

    #[test]
    fn finds_the_last_user_message() {
        let body = serde_json::json!({"messages": [
            {"role": "system", "content": "s"},
            {"role": "user", "content": "first"},
            {"role": "assistant", "content": "a"},
            {"role": "user", "content": [{"type": "text", "text": "second"}, {"type": "image_url"}]}
        ]});
        assert_eq!(last_user_text(&body), "second");
    }
}
