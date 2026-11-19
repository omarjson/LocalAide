//! The two ways LocalAide talks to an MCP server: a local program over its
//! standard input and output, or a remote address over Streamable HTTP.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::app::lifecycle;

/// Lines kept from a local server's error output, for error messages.
const STDERR_LINES: usize = 20;
/// An HTTP answer larger than this is refused.
const MAX_HTTP_BYTES: usize = 8 * 1024 * 1024;

pub trait Transport: Send {
    /// Send one message. With `expect_id`, wait for the answer with that id.
    fn send(
        &mut self,
        message: &Value,
        expect_id: Option<u64>,
        timeout: Duration,
    ) -> Result<Option<Value>, AppError>;
    fn set_protocol_version(&mut self, _version: &str) {}
    fn close(&mut self);
}

fn disconnected(detail: impl Into<String>) -> AppError {
    AppError::new("MCP_DISCONNECTED", "The MCP server stopped answering.").with_detail(detail)
}

/// The answer to a request the server sends us. LocalAide offers no client
/// features, so only `ping` gets a real answer.
fn reply_to_server_request(request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    request.get("method")?;
    if request.get("method").and_then(Value::as_str) == Some("ping") {
        return Some(json!({"jsonrpc": "2.0", "id": id, "result": {}}));
    }
    Some(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32601, "message": "LocalAide does not support this request."}
    }))
}

fn matches_id(message: &Value, id: u64) -> bool {
    message.get("method").is_none() && message.get("id").and_then(Value::as_u64) == Some(id)
}

pub struct StdioTransport {
    name: String,
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

/// Folders where command-line tools usually live. Apps opened from the
/// Dock or Start menu do not get the shell's PATH, so `npx` and `uvx` would
/// otherwise not be found.
#[cfg(not(windows))]
fn extended_path() -> Option<std::ffi::OsString> {
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let home = dirs_next::home_dir();
    let mut extra: Vec<std::path::PathBuf> = vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/usr/bin".into(),
        "/bin".into(),
    ];
    if let Some(home) = &home {
        for rel in [
            ".local/bin",
            ".cargo/bin",
            ".bun/bin",
            ".volta/bin",
            ".deno/bin",
        ] {
            extra.push(home.join(rel));
        }
        // The newest Node installed with nvm.
        if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            let mut versions: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path().join("bin")))
                .collect();
            versions.sort();
            if let Some(newest) = versions.pop() {
                extra.push(newest);
            }
        }
    }
    for dir in extra {
        if dir.is_dir() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).ok()
}

impl StdioTransport {
    pub fn spawn(
        name: &str,
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<Self, AppError> {
        let command = command.trim();
        // On Windows, npx, uvx and similar are batch files that only the
        // command interpreter can start.
        #[cfg(windows)]
        let mut cmd = {
            let lower = command.to_ascii_lowercase();
            if lower.ends_with(".exe") || lower.contains('\\') || lower.contains('/') {
                let mut c = Command::new(command);
                c.args(args);
                c
            } else {
                let mut c = Command::new("cmd");
                c.arg("/C").arg(command).args(args);
                c
            }
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new(command);
            c.args(args);
            if let Some(path) = extended_path() {
                c.env("PATH", path);
            }
            c
        };
        cmd.envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = cwd.map(str::trim).filter(|d| !d.is_empty()) {
            cmd.current_dir(dir);
        }
        lifecycle::hide_console_window(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| {
            AppError::new(
                "MCP_START_FAILED",
                format!(
                    "{name} could not start: {e}. Check that \"{command}\" is installed on this computer."
                ),
            )
        })?;
        lifecycle::tie_child_to_app(&child);

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::internal("The MCP server has no input."))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::internal("The MCP server has no output."))?;
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stderr) = child.stderr.take() {
            let tail = stderr_tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    let mut tail = tail.lock().unwrap_or_else(|e| e.into_inner());
                    if tail.len() == STDERR_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            });
        }
        Ok(Self {
            name: name.to_string(),
            child,
            stdin,
            lines,
            stderr_tail,
        })
    }

    fn stderr_text(&self) -> String {
        let tail = self.stderr_tail.lock().unwrap_or_else(|e| e.into_inner());
        tail.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    fn write_line(&mut self, message: &Value) -> Result<(), AppError> {
        let mut line = message.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
            .map_err(|e| disconnected(format!("{e}\n{}", self.stderr_text())))
    }
}

impl Transport for StdioTransport {
    fn send(
        &mut self,
        message: &Value,
        expect_id: Option<u64>,
        timeout: Duration,
    ) -> Result<Option<Value>, AppError> {
        self.write_line(message)?;
        let Some(id) = expect_id else {
            return Ok(None);
        };
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(AppError::new(
                    "MCP_TIMEOUT",
                    format!("{} took too long to answer.", self.name),
                ));
            }
            let line = match self
                .lines
                .recv_timeout(left.min(Duration::from_millis(500)))
            {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    if let Ok(Some(status)) = self.child.try_wait() {
                        return Err(disconnected(format!(
                            "{} exited ({status}).\n{}",
                            self.name,
                            self.stderr_text()
                        )));
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let detail = self.stderr_text();
                    return Err(AppError::new(
                        "MCP_DISCONNECTED",
                        format!(
                            "{} stopped. {}",
                            self.name,
                            detail.lines().last().unwrap_or("It printed no error.")
                        ),
                    )
                    .with_detail(detail));
                }
            };
            // Servers may print logs on stdout; only JSON lines count.
            let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if matches_id(&value, id) {
                return Ok(Some(value));
            }
            if let Some(reply) = reply_to_server_request(&value) {
                self.write_line(&reply)?;
            }
        }
    }

    fn close(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        self.close();
    }
}

pub struct HttpTransport {
    client: reqwest::blocking::Client,
    url: String,
    headers: BTreeMap<String, String>,
    session: Option<String>,
    protocol: Option<String>,
}

impl HttpTransport {
    pub fn new(url: &str, headers: &BTreeMap<String, String>) -> Result<Self, AppError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(240))
            // A redirect could carry the user's token to another site.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(Self {
            client,
            url: url.to_string(),
            headers: headers.clone(),
            session: None,
            protocol: None,
        })
    }
}

/// Find the answer with `id` in an HTTP body: one JSON message, a batch, or
/// server-sent events.
pub fn find_in_body(body: &str, id: u64) -> Option<Value> {
    let trimmed = body.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let value: Value = serde_json::from_str(trimmed).ok()?;
        return match value {
            Value::Array(items) => items.into_iter().find(|m| matches_id(m, id)),
            single if matches_id(&single, id) => Some(single),
            _ => None,
        };
    }
    trimmed
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|data| serde_json::from_str::<Value>(data.trim()).ok())
        .find(|m| matches_id(m, id))
}

impl Transport for HttpTransport {
    fn send(
        &mut self,
        message: &Value,
        expect_id: Option<u64>,
        timeout: Duration,
    ) -> Result<Option<Value>, AppError> {
        let mut request = self
            .client
            .post(&self.url)
            .timeout(timeout)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("User-Agent", "LocalAide-MCP");
        for (name, value) in &self.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        if let Some(session) = &self.session {
            request = request.header("Mcp-Session-Id", session.as_str());
        }
        if let Some(version) = &self.protocol {
            request = request.header("MCP-Protocol-Version", version.as_str());
        }
        let response = request.body(message.to_string()).send().map_err(|e| {
            AppError::new(
                "MCP_UNREACHABLE",
                format!("Could not reach the MCP server: {e}"),
            )
            .retryable()
        })?;
        let status = response.status();
        if let Some(session) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            self.session = Some(session.to_string());
        }
        if status.as_u16() == 404 && self.session.is_some() {
            return Err(disconnected("The server ended the session."));
        }
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(AppError::new(
                "MCP_AUTH",
                "The MCP server wants a sign-in or token. Add an Authorization header in Settings → Tools and MCP.",
            ));
        }
        if status.is_redirection() {
            return Err(AppError::invalid_input(
                "The MCP server sent LocalAide to another address. Use the final address instead.",
            ));
        }
        if !status.is_success() {
            return Err(AppError::new(
                "MCP_ERROR",
                format!("The MCP server answered with an error ({status})."),
            ));
        }
        let Some(id) = expect_id else {
            return Ok(None);
        };
        let is_events = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.contains("text/event-stream"));
        if is_events {
            // An event stream can stay open after the answer; stop reading
            // as soon as it arrives.
            let mut read = 0usize;
            for line in BufReader::new(response).lines() {
                let line = line.map_err(|e| disconnected(e.to_string()))?;
                read += line.len();
                if read > MAX_HTTP_BYTES {
                    return Err(AppError::invalid_input(
                        "The MCP server sent too much data.",
                    ));
                }
                if let Some(data) = line.strip_prefix("data:")
                    && let Ok(value) = serde_json::from_str::<Value>(data.trim())
                    && matches_id(&value, id)
                {
                    return Ok(Some(value));
                }
            }
            return Err(disconnected("The event stream ended without an answer."));
        }
        let bytes = response.bytes().map_err(|e| disconnected(e.to_string()))?;
        if bytes.len() > MAX_HTTP_BYTES {
            return Err(AppError::invalid_input(
                "The MCP server sent too much data.",
            ));
        }
        let body = String::from_utf8_lossy(&bytes);
        find_in_body(&body, id)
            .map(Some)
            .ok_or_else(|| AppError::internal("The MCP server's answer could not be read."))
    }

    fn set_protocol_version(&mut self, version: &str) {
        self.protocol = Some(version.to_string());
    }

    fn close(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = self
                .client
                .delete(&self.url)
                .timeout(Duration::from_secs(3))
                .header("Mcp-Session-Id", session)
                .send();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_found_in_every_body_shape() {
        let json = r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true}}"#;
        assert!(find_in_body(json, 2).is_some());
        assert!(find_in_body(json, 3).is_none());
        let batch = r#"[{"jsonrpc":"2.0","method":"notifications/x"},{"jsonrpc":"2.0","id":5,"result":{}}]"#;
        assert!(find_in_body(batch, 5).is_some());
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"log\"}\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{}}\n";
        assert!(find_in_body(sse, 7).is_some());
    }

    #[test]
    fn server_requests_get_answers() {
        let ping = json!({"jsonrpc":"2.0","id":9,"method":"ping"});
        assert_eq!(reply_to_server_request(&ping).unwrap()["result"], json!({}));
        let other = json!({"jsonrpc":"2.0","id":"a","method":"roots/list"});
        assert_eq!(
            reply_to_server_request(&other).unwrap()["error"]["code"],
            -32601
        );
        let note = json!({"jsonrpc":"2.0","method":"notifications/progress"});
        assert!(reply_to_server_request(&note).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_local_server_answers_over_stdio() {
        // A tiny "server": answers the first request line with id 1.
        let script = r#"read line; echo 'not json'; echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25"}}'; sleep 1"#;
        let mut t = StdioTransport::spawn(
            "test",
            "sh",
            &["-c".to_string(), script.to_string()],
            &BTreeMap::new(),
            None,
        )
        .unwrap();
        let answer = t
            .send(
                &json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
                Some(1),
                Duration::from_secs(5),
            )
            .unwrap()
            .unwrap();
        assert_eq!(answer["result"]["protocolVersion"], "2025-11-25");
        t.close();
    }
}
