//! Link to an OpenCode server the user runs (`opencode serve`, by default
//! on http://127.0.0.1:4096). LocalAide can check that it answers and hand
//! a task over to a new OpenCode session, so a coding job started in chat
//! continues in OpenCode (or a front end for it, such as OpenChamber).
//!
//! Every call is an explicit user action. The address must be https, or
//! plain http on this computer, like other servers in LocalAide. When the
//! server has a password (`OPENCODE_SERVER_PASSWORD`), it is sent with HTTP
//! basic auth as user `opencode`, and only to that address.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::error::AppError;

pub const DEFAULT_URL: &str = "http://127.0.0.1:4096";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
/// Hand-over text is a chat message or a summary of one, not a document.
const MAX_TASK_CHARS: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenCodeHealth {
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenCodeHandoff {
    pub session_id: String,
    pub title: String,
}

pub struct OpenCodeClient {
    client: reqwest::Client,
    base: String,
    password: Option<String>,
}

impl OpenCodeClient {
    pub fn new(base: &str, password: Option<&str>) -> Result<Self, AppError> {
        let base = base.trim().trim_end_matches('/').to_string();
        crate::config::validate_base_url(&base).map_err(AppError::invalid_input)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(Self {
            client,
            base,
            password: password
                .map(str::to_string)
                .filter(|p| !p.trim().is_empty()),
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .header("Accept", "application/json")
            .header("User-Agent", "LocalAide-opencode");
        if let Some(password) = &self.password {
            request = request.basic_auth("opencode", Some(password));
        }
        request
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<String, AppError> {
        let response = request.send().await.map_err(|e| {
            AppError::new(
                "OPENCODE_UNREACHABLE",
                "OpenCode is not answering at that address. Start it with \"opencode serve\" and try again.",
            )
            .with_detail(e.to_string())
            .retryable()
        })?;
        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(AppError::new(
                "OPENCODE_AUTH",
                "OpenCode asked for a password. Enter the one set in OPENCODE_SERVER_PASSWORD.",
            ));
        }
        if !status.is_success() {
            return Err(AppError::new(
                "OPENCODE_ERROR",
                format!("OpenCode answered with an error ({status})."),
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::internal(e.to_string()))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::invalid_input("OpenCode sent too much data."));
        }
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    /// `GET /global/health`.
    pub async fn health(&self) -> Result<OpenCodeHealth, AppError> {
        let body = self
            .send(self.request(reqwest::Method::GET, "/global/health"))
            .await?;
        parse_health(&body)
    }

    /// Create a session and send it the task without waiting for the
    /// answer (`POST /session`, then `POST /session/:id/prompt_async`).
    pub async fn hand_off(&self, title: &str, task: &str) -> Result<OpenCodeHandoff, AppError> {
        let task = task.trim();
        if task.is_empty() {
            return Err(AppError::invalid_input(
                "There is nothing to send to OpenCode.",
            ));
        }
        if task.chars().count() > MAX_TASK_CHARS {
            return Err(AppError::invalid_input(
                "That is too long to send to OpenCode.",
            ));
        }
        let title = session_title(title, task);
        let body = self
            .send(
                self.request(reqwest::Method::POST, "/session")
                    .json(&json!({ "title": title })),
            )
            .await?;
        let session_id = parse_session_id(&body)?;
        self.send(
            self.request(
                reqwest::Method::POST,
                &format!("/session/{session_id}/prompt_async"),
            )
            .json(&json!({ "parts": [{ "type": "text", "text": task }] })),
        )
        .await?;
        Ok(OpenCodeHandoff { session_id, title })
    }
}

pub fn parse_health(body: &str) -> Result<OpenCodeHealth, AppError> {
    let json: Value = serde_json::from_str(body).map_err(|_| {
        AppError::new(
            "OPENCODE_ERROR",
            "That address answered, but not like OpenCode.",
        )
    })?;
    if json.get("healthy").and_then(Value::as_bool) == Some(false) {
        return Err(AppError::new(
            "OPENCODE_ERROR",
            "OpenCode says it is not healthy.",
        ));
    }
    Ok(OpenCodeHealth {
        version: json
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// Session ids are used in a request path, so only plain tokens pass.
pub fn parse_session_id(body: &str) -> Result<String, AppError> {
    let json: Value = serde_json::from_str(body)
        .map_err(|_| AppError::new("OPENCODE_ERROR", "OpenCode did not create a session."))?;
    let id = json
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::new(
            "OPENCODE_ERROR",
            "OpenCode sent a session id LocalAide does not accept.",
        ));
    }
    Ok(id)
}

/// The title the user gave, or the first line of the task, shortened.
fn session_title(title: &str, task: &str) -> String {
    let source = if title.trim().is_empty() {
        task.lines().next().unwrap_or("Task from LocalAide")
    } else {
        title
    };
    let flat: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = flat.chars().take(60).collect();
    if cut.chars().count() < flat.chars().count() {
        format!("{}…", cut.trim_end())
    } else {
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_and_session_answers_parse() {
        assert_eq!(
            parse_health(r#"{"healthy":true,"version":"1.4.0"}"#)
                .unwrap()
                .version
                .as_deref(),
            Some("1.4.0")
        );
        assert!(parse_health("<html>").is_err());
        assert!(parse_health(r#"{"healthy":false}"#).is_err());
        assert_eq!(
            parse_session_id(r#"{"id":"ses_abc-1"}"#).unwrap(),
            "ses_abc-1"
        );
        assert!(parse_session_id(r#"{"id":"../etc"}"#).is_err());
        assert!(parse_session_id(r#"{}"#).is_err());
    }

    #[test]
    fn titles_come_from_the_task_when_empty() {
        assert_eq!(
            session_title("", "Fix the login bug\nDetails"),
            "Fix the login bug"
        );
        assert_eq!(session_title("  My   task ", "x"), "My task");
        assert!(session_title("", &"a".repeat(100)).ends_with('…'));
    }

    #[test]
    fn addresses_follow_the_local_rules() {
        assert!(OpenCodeClient::new("http://127.0.0.1:4096/", None).is_ok());
        assert!(OpenCodeClient::new("http://192.168.1.4:4096", None).is_err());
        assert!(OpenCodeClient::new("https://code.example.com", Some("")).is_ok());
    }

    #[tokio::test]
    async fn hand_off_creates_a_session_and_sends_the_task() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for body in [r#"{"id":"ses_1"}"#, ""] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                let status = if body.is_empty() {
                    "204 No Content"
                } else {
                    "200 OK"
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            }
        });
        let client = OpenCodeClient::new(&format!("http://{addr}"), Some("pw")).unwrap();
        let handoff = client
            .hand_off("", "Write tests for the parser")
            .await
            .unwrap();
        assert_eq!(handoff.session_id, "ses_1");
        assert_eq!(handoff.title, "Write tests for the parser");
        let requests = seen.lock().unwrap();
        assert!(requests[0].starts_with("POST /session "));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: basic")
        );
        assert!(requests[1].starts_with("POST /session/ses_1/prompt_async "));
    }
}
