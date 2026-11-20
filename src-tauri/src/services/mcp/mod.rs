//! Model Context Protocol client (ADR-0012).
//!
//! The user adds MCP servers in Settings → Tools and MCP. A local server is
//! a program LocalAide starts and speaks JSON-RPC with over its standard
//! input and output; a remote server is reached over Streamable HTTP. The
//! servers' tools are offered to the model in chat, and every call asks the
//! user first unless they marked the server as trusted.
//!
//! This client speaks the session-based protocol (2025-11-25 and earlier),
//! which servers on the newer stateless revision still accept.

pub mod import;
pub mod presets;
mod transport;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::config::{McpServerConfig, McpTransport};
use transport::{HttpTransport, StdioTransport, Transport};

/// Protocol revision offered in `initialize`.
pub const PROTOCOL_VERSION: &str = "2025-11-25";
const INIT_TIMEOUT: Duration = Duration::from_secs(45);
const LIST_TIMEOUT: Duration = Duration::from_secs(30);
const CALL_TIMEOUT: Duration = Duration::from_secs(180);
/// Tool output larger than this is cut; the model cannot use more anyway.
const MAX_OUTPUT_CHARS: usize = 100_000;
const MAX_TOOLS_PER_SERVER: usize = 128;

/// One tool a server offers.
#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    /// JSON Schema for the arguments.
    pub input_schema: Value,
}

/// What a tool call returned.
#[derive(Debug, Clone, PartialEq)]
pub struct McpCallOutput {
    pub text: String,
    /// The server marked the result as a failure.
    pub is_error: bool,
}

struct Session {
    fingerprint: String,
    transport: Box<dyn Transport>,
    next_id: u64,
    tools: Option<Vec<McpTool>>,
}

impl Session {
    fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, AppError> {
        self.next_id += 1;
        let id = self.next_id;
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let answer = self
            .transport
            .send(&message, Some(id), timeout)?
            .ok_or_else(|| AppError::internal("The MCP server sent no answer."))?;
        if let Some(error) = answer.get("error") {
            let text = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(AppError::new(
                "MCP_ERROR",
                format!("The MCP server answered with an error: {text}"),
            ));
        }
        Ok(answer.get("result").cloned().unwrap_or(Value::Null))
    }

    fn notify(&mut self, method: &str) -> Result<(), AppError> {
        let message = json!({"jsonrpc": "2.0", "method": method});
        self.transport
            .send(&message, None, LIST_TIMEOUT)
            .map(|_| ())
    }
}

/// Keeps one live session per server. Local programs keep running between
/// calls and stop when the app exits or the server is changed.
#[derive(Default)]
pub struct McpManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<Session>>>>,
}

fn fingerprint(cfg: &McpServerConfig) -> String {
    serde_json::to_string(&cfg.transport).unwrap_or_default()
}

/// Check a server entry before it is saved or started.
pub fn validate(cfg: &McpServerConfig) -> Result<(), AppError> {
    if cfg.name.trim().is_empty() {
        return Err(AppError::invalid_input("Give the server a name."));
    }
    match &cfg.transport {
        McpTransport::Stdio { command, .. } => {
            if command.trim().is_empty() {
                return Err(AppError::invalid_input(
                    "Enter the command that starts the server, for example npx.",
                ));
            }
        }
        McpTransport::Http { url, headers } => {
            crate::config::validate_base_url(url.trim()).map_err(AppError::invalid_input)?;
            for name in headers.keys() {
                let ok = !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
                if !ok {
                    return Err(AppError::invalid_input(format!(
                        "\"{name}\" is not a valid header name."
                    )));
                }
            }
        }
    }
    Ok(())
}

impl McpManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn session(&self, cfg: &McpServerConfig) -> Result<Arc<Mutex<Session>>, AppError> {
        let print = fingerprint(cfg);
        {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = sessions.get(&cfg.id) {
                let same = existing
                    .lock()
                    .map(|s| s.fingerprint == print)
                    .unwrap_or(false);
                if same {
                    return Ok(existing.clone());
                }
            }
        }
        // Changed or new: close the old one, then connect.
        self.disconnect(&cfg.id);
        validate(cfg)?;
        let transport: Box<dyn Transport> = match &cfg.transport {
            McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => Box::new(StdioTransport::spawn(
                &cfg.name,
                command,
                args,
                env,
                cwd.as_deref(),
            )?),
            McpTransport::Http { url, headers } => {
                Box::new(HttpTransport::new(url.trim(), headers)?)
            }
        };
        let mut session = Session {
            fingerprint: print,
            transport,
            next_id: 0,
            tools: None,
        };
        let init = session.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "LocalAide", "version": env!("CARGO_PKG_VERSION")}
            }),
            INIT_TIMEOUT,
        )?;
        if let Some(version) = init.get("protocolVersion").and_then(Value::as_str) {
            session.transport.set_protocol_version(version);
        }
        session.notify("notifications/initialized")?;
        let shared = Arc::new(Mutex::new(session));
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(cfg.id.clone(), shared.clone());
        Ok(shared)
    }

    /// The server's tools. Connects on first use; the list is kept for the
    /// session.
    pub fn list_tools(&self, cfg: &McpServerConfig) -> Result<Vec<McpTool>, AppError> {
        self.with_session(cfg, |session| {
            if let Some(tools) = &session.tools {
                return Ok(tools.clone());
            }
            let mut tools = Vec::new();
            let mut cursor: Option<String> = None;
            for _ in 0..10 {
                let params = match &cursor {
                    Some(c) => json!({"cursor": c}),
                    None => json!({}),
                };
                let page = session.request("tools/list", params, LIST_TIMEOUT)?;
                tools.extend(parse_tools(&page));
                cursor = page
                    .get("nextCursor")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if cursor.is_none() || tools.len() >= MAX_TOOLS_PER_SERVER {
                    break;
                }
            }
            tools.truncate(MAX_TOOLS_PER_SERVER);
            session.tools = Some(tools.clone());
            Ok(tools)
        })
    }

    /// Call one tool. Output is cut at 100,000 characters.
    pub fn call_tool(
        &self,
        cfg: &McpServerConfig,
        tool: &str,
        arguments: Value,
    ) -> Result<McpCallOutput, AppError> {
        let arguments = if arguments.is_object() {
            arguments
        } else {
            json!({})
        };
        self.with_session(cfg, |session| {
            let result = session.request(
                "tools/call",
                json!({"name": tool, "arguments": arguments.clone()}),
                CALL_TIMEOUT,
            )?;
            Ok(parse_call_result(&result))
        })
    }

    /// Run `work` on the server's session. A broken session (the program
    /// exited, the remote session expired) is reconnected once.
    fn with_session<T>(
        &self,
        cfg: &McpServerConfig,
        work: impl Fn(&mut Session) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        for attempt in 0..2 {
            let shared = self.session(cfg)?;
            let outcome = {
                let mut session = shared.lock().unwrap_or_else(|e| e.into_inner());
                work(&mut session)
            };
            match outcome {
                Err(e) if attempt == 0 && e.code == "MCP_DISCONNECTED" => {
                    self.disconnect(&cfg.id);
                    continue;
                }
                other => return other,
            }
        }
        Err(AppError::new(
            "MCP_DISCONNECTED",
            "The MCP server stopped answering.",
        ))
    }

    /// True while a session with this server is open.
    pub fn is_connected(&self, id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(id)
    }

    /// Close one server's session and stop its program.
    pub fn disconnect(&self, id: &str) {
        let removed = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        if let Some(session) = removed {
            let mut session = session.lock().unwrap_or_else(|e| e.into_inner());
            session.transport.close();
        }
    }

    /// Close every session, at app exit.
    pub fn disconnect_all(&self) {
        let ids: Vec<String> = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.disconnect(&id);
        }
    }
}

/// Read a `tools/list` page.
pub fn parse_tools(page: &Value) -> Vec<McpTool> {
    page.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(Value::as_str)?.trim().to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let description = t
                        .get("description")
                        .and_then(Value::as_str)
                        .or_else(|| t.get("title").and_then(Value::as_str))
                        .unwrap_or("")
                        .to_string();
                    let input_schema = t
                        .get("inputSchema")
                        .filter(|s| s.is_object())
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
                    Some(McpTool {
                        name,
                        description,
                        input_schema,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Turn a `tools/call` result into text for the model.
pub fn parse_call_result(result: &Value) -> McpCallOutput {
    let mut parts: Vec<String> = Vec::new();
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        for item in content {
            match item.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = item.get("text").and_then(Value::as_str) {
                        parts.push(t.to_string());
                    }
                }
                Some("resource") => {
                    let resource = item.get("resource");
                    if let Some(t) = resource.and_then(|r| r.get("text")).and_then(Value::as_str) {
                        parts.push(t.to_string());
                    } else if let Some(uri) =
                        resource.and_then(|r| r.get("uri")).and_then(Value::as_str)
                    {
                        parts.push(format!("[resource {uri}]"));
                    }
                }
                Some("resource_link") => {
                    if let Some(uri) = item.get("uri").and_then(Value::as_str) {
                        parts.push(format!("[link {uri}]"));
                    }
                }
                Some("image") => parts.push("[image not shown]".to_string()),
                Some("audio") => parts.push("[audio not shown]".to_string()),
                _ => {}
            }
        }
    }
    if parts.is_empty()
        && let Some(structured) = result.get("structuredContent").filter(|v| !v.is_null())
    {
        parts.push(structured.to_string());
    }
    let mut text = parts.join("\n");
    if text.chars().count() > MAX_OUTPUT_CHARS {
        text =
            text.chars().take(MAX_OUTPUT_CHARS).collect::<String>() + "\n[cut: the tool sent more]";
    }
    McpCallOutput {
        text,
        is_error: result.get("isError").and_then(Value::as_bool) == Some(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn tool_lists_and_results_parse() {
        let tools = parse_tools(&json!({"tools": [
            {"name": "resolve", "description": "Find a library", "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}}},
            {"name": "  "},
            {"name": "bare"}
        ]}));
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[1].input_schema["type"], "object");

        let out = parse_call_result(&json!({"content": [
            {"type": "text", "text": "one"},
            {"type": "image", "data": "...", "mimeType": "image/png"},
            {"type": "resource", "resource": {"uri": "file:///a", "text": "two"}}
        ]}));
        assert_eq!(out.text, "one\n[image not shown]\ntwo");
        assert!(!out.is_error);

        let out = parse_call_result(
            &json!({"content": [], "structuredContent": {"a": 1}, "isError": true}),
        );
        assert_eq!(out.text, "{\"a\":1}");
        assert!(out.is_error);
    }

    #[test]
    fn entries_are_checked() {
        let mut cfg = McpServerConfig {
            id: "m1".into(),
            name: "Docs".into(),
            enabled: true,
            auto_approve: false,
            transport: McpTransport::Http {
                url: "http://example.com/mcp".into(),
                headers: BTreeMap::new(),
            },
        };
        assert!(validate(&cfg).is_err(), "plain HTTP only on this computer");
        cfg.transport = McpTransport::Http {
            url: "https://mcp.example.com/mcp".into(),
            headers: BTreeMap::from([("Bad Header".to_string(), "x".to_string())]),
        };
        assert!(validate(&cfg).is_err());
        cfg.transport = McpTransport::Stdio {
            command: " ".into(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        };
        assert!(validate(&cfg).is_err());
    }
}
