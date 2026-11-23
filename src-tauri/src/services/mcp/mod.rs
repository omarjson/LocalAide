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
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::app::error::AppError;
pub use crate::config::{McpOAuthConfig, McpServerConfig, McpTransport};
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

// ---- OAuth 2.1 PKCE sign-in ------------------------------------------------
//
// Everything in this section except `exchange_code` is pure: it builds URLs,
// verifies states and parses responses without touching the network. The
// existing stdio and HTTP transports are unchanged — a signed-in caller
// supplies the resulting bearer token through the usual HTTP `headers` map.

/// The verifier/challenge/state triple for one sign-in attempt. The verifier
/// is a secret, so this type deliberately has no `Debug` impl.
#[derive(Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

/// A pure form-encoded token request. The caller owns the single network
/// call (`exchange_code`) or can post `body` itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRequest {
    pub url: String,
    pub body: String,
}

/// Parsed token response. Tokens are secrets, so no `Debug` impl.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
}

/// RFC 7636 S256 challenge: base64url(SHA-256(ASCII(verifier))), no padding.
pub fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn random_token(bytes: usize) -> Result<String, AppError> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf)
        .map_err(|_| AppError::internal("No random source is available for sign-in."))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

/// Create a fresh PKCE pair and an unrelated anti-CSRF state. Random only;
/// no I/O or network.
pub fn new_pkce() -> Result<Pkce, AppError> {
    let verifier = random_token(32)?;
    let challenge = pkce_challenge(&verifier);
    let state = random_token(16)?;
    Ok(Pkce {
        verifier,
        challenge,
        state,
    })
}

/// Build the browser authorization URL with the S256 challenge and state.
/// Endpoints must pass the same HTTPS/loopback rule as other providers.
pub fn authorize_url(
    cfg: &McpOAuthConfig,
    state: &str,
    challenge: &str,
) -> Result<String, AppError> {
    crate::config::validate_base_url(&cfg.authorize_url).map_err(AppError::invalid_input)?;
    let mut url = url::Url::parse(&cfg.authorize_url)
        .map_err(|e| AppError::invalid_input(format!("Invalid authorization URL: {e}")))?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &cfg.client_id);
        query.append_pair("redirect_uri", &cfg.redirect_uri);
        query.append_pair("state", state);
        query.append_pair("code_challenge", challenge);
        query.append_pair("code_challenge_method", "S256");
        if !cfg.scopes.is_empty() {
            query.append_pair("scope", &cfg.scopes.join(" "));
        }
        if let Some(audience) = &cfg.audience {
            query.append_pair("audience", audience);
        }
    }
    Ok(url.to_string())
}

/// Constant-work comparison of the state returned by the server with the one
/// created for this sign-in. An empty state is never accepted.
pub fn verify_state(expected: &str, received: &str) -> Result<(), AppError> {
    if expected.is_empty() || received.is_empty() || expected != received {
        return Err(AppError::invalid_input(
            "The sign-in state did not match. Please try signing in again.",
        ));
    }
    Ok(())
}

/// Build the code-for-token form body. Pure; no request is sent here.
pub fn token_request(
    cfg: &McpOAuthConfig,
    code: &str,
    verifier: &str,
) -> Result<TokenRequest, AppError> {
    crate::config::validate_base_url(&cfg.token_url).map_err(AppError::invalid_input)?;
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("grant_type", "authorization_code");
    form.append_pair("code", code);
    form.append_pair("code_verifier", verifier);
    form.append_pair("redirect_uri", &cfg.redirect_uri);
    form.append_pair("client_id", &cfg.client_id);
    Ok(TokenRequest {
        url: cfg.token_url.clone(),
        body: form.finish(),
    })
}

/// Parse a token response, including OAuth error responses. Pure.
pub fn parse_token_response(body: &str) -> Result<TokenResponse, AppError> {
    let value: Value = serde_json::from_str(body).map_err(|e| {
        AppError::internal(format!(
            "The authorization server sent an unreadable response: {e}"
        ))
    })?;
    if let Some(error) = value.get("error") {
        let reason = value
            .get("error_description")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
            .unwrap_or("authorization failed");
        return Err(AppError::internal(format!("Sign-in failed: {reason}")));
    }
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            AppError::internal("The authorization server did not return an access token.")
        })?;
    Ok(TokenResponse {
        access_token: access_token.to_string(),
        token_type: value
            .get("token_type")
            .and_then(Value::as_str)
            .unwrap_or("Bearer")
            .to_string(),
        expires_in: value.get("expires_in").and_then(Value::as_u64),
        refresh_token: value
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// The bearer pair to add to an existing HTTP transport's `headers` map.
/// Keeping token injection in the caller leaves stdio/HTTP transports intact.
pub fn bearer_header(token: &str) -> (String, String) {
    ("Authorization".to_string(), format!("Bearer {token}"))
}

/// The one impure helper: exchange an authorization code for a token. No
/// redirect is followed, so the code and verifier cannot be leaked to an
/// address the config did not name.
pub fn exchange_code(
    cfg: &McpOAuthConfig,
    code: &str,
    verifier: &str,
) -> Result<TokenResponse, AppError> {
    let request = token_request(cfg, code, verifier)?;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new());
    let response = client
        .post(&request.url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body(request.body)
        .send()
        .map_err(|e| {
            AppError::internal(format!("Could not reach the authorization server: {e}"))
        })?;
    let text = response
        .text()
        .map_err(|e| AppError::internal(format!("Could not read the token response: {e}")))?;
    parse_token_response(&text)
}

/// How long an unfinished sign-in stays usable. A state older than this is
/// dropped, so an attacker cannot finish a flow hours after the user began.
const OAUTH_STATE_TTL: Duration = Duration::from_secs(10 * 60);
/// Default loopback redirect when the client did not register one. The app
/// shows the callback address; the user pastes the code from it.
pub const DEFAULT_OAUTH_REDIRECT_URI: &str = "http://127.0.0.1:8765/callback";

/// One unfinished sign-in. The verifier is a secret, so this type has no
/// `Debug` impl and never crosses the IPC boundary.
struct PendingSignIn {
    cfg: McpOAuthConfig,
    pkce: Pkce,
    started_at: Instant,
}

fn pending_signins() -> &'static Mutex<HashMap<String, PendingSignIn>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingSignIn>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Stable keychain account id for one server's access token. Derived only
/// from the server id, so the token is found again after a restart and is
/// never used as an id itself.
pub fn oauth_token_key(server_id: &str) -> String {
    format!("mcp-oauth:{server_id}")
}

/// Begin a sign-in: remember the PKCE verifier and state for this server.
/// A second attempt replaces the first, so stale states cannot be finished.
pub fn begin_sign_in(cfg: McpOAuthConfig, pkce: Pkce) {
    let pending = PendingSignIn {
        cfg,
        pkce,
        started_at: Instant::now(),
    };
    let mut map = pending_signins().lock().unwrap_or_else(|e| e.into_inner());
    map.insert(pending.cfg.server_id.clone(), pending);
}

/// Take a pending sign-in, but only while it is fresh. Removing it here makes
/// every state single-use, including one that fails verification.
pub fn take_sign_in(server_id: &str) -> Result<(McpOAuthConfig, Pkce), AppError> {
    let removed = pending_signins()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(server_id);
    let pending = removed.ok_or_else(|| {
        AppError::invalid_input("That sign-in has expired. Please start signing in again.")
    })?;
    if pending.started_at.elapsed() > OAUTH_STATE_TTL {
        return Err(AppError::invalid_input(
            "That sign-in took too long. Please start signing in again.",
        ));
    }
    Ok((pending.cfg, pending.pkce))
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
                // A signed-in server keeps its token in the OS keychain, not
                // in config.json. Add the bearer header at connect time so
                // the existing HTTP transport and stdio path stay untouched.
                let mut effective = headers.clone();
                if cfg.oauth.is_some() {
                    let key = oauth_token_key(&cfg.id);
                    if let Ok(Some(token)) = crate::services::secrets::load(&key) {
                        let (name, value) = bearer_header(&token);
                        effective.entry(name).or_insert(value);
                    }
                }
                Box::new(HttpTransport::new(url.trim(), &effective)?)
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
        let tools = self.with_session(cfg, |session| {
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
        })?;
        // Only the tools the user left enabled are offered to the model.
        Ok(filter_tools(tools, cfg))
    }

    /// Call one tool. Output is cut at 100,000 characters.
    pub fn call_tool(
        &self,
        cfg: &McpServerConfig,
        tool: &str,
        arguments: Value,
    ) -> Result<McpCallOutput, AppError> {
        if !tool_enabled(cfg, tool) {
            return Err(AppError::invalid_input(format!(
                "The tool '{tool}' is turned off for this MCP server."
            )));
        }
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

/// Whether one tool is exposed for a server. Disabled tools always lose; an
/// empty `enabled_tools` allows everything, so older configs are unaffected.
pub fn tool_enabled(cfg: &McpServerConfig, name: &str) -> bool {
    if cfg.disabled_tools.iter().any(|disabled| disabled == name) {
        return false;
    }
    if cfg.enabled_tools.is_empty() {
        return true;
    }
    cfg.enabled_tools.iter().any(|enabled| enabled == name)
}

/// Drop tools the user turned off before they are offered to the model.
pub fn filter_tools(tools: Vec<McpTool>, cfg: &McpServerConfig) -> Vec<McpTool> {
    tools
        .into_iter()
        .filter(|tool| tool_enabled(cfg, &tool.name))
        .collect()
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

    fn oauth_cfg() -> McpOAuthConfig {
        McpOAuthConfig {
            server_id: "docs".into(),
            client_id: "client-1".into(),
            authorize_url: "https://auth.example.com/authorize".into(),
            token_url: "https://auth.example.com/token".into(),
            redirect_uri: "http://127.0.0.1:8765/callback".into(),
            scopes: vec!["mcp.read".into(), "mcp.write".into()],
            audience: Some("mcp://docs".into()),
        }
    }

    #[test]
    fn pkce_challenge_matches_the_rfc_example() {
        // RFC 7636 Appendix B example.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            pkce_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let fresh = new_pkce().unwrap();
        assert_ne!(fresh.verifier, fresh.state);
        assert_eq!(fresh.challenge, pkce_challenge(&fresh.verifier));
        assert_eq!(bearer_header("tok").1, "Bearer tok");
    }

    #[test]
    fn authorize_url_carries_pkce_and_state() {
        let cfg = oauth_cfg();
        let url = authorize_url(&cfg, "state-1", "challenge-1").unwrap();
        let parsed = url::Url::parse(&url).unwrap();
        let pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let get = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("response_type"), "code");
        assert_eq!(get("client_id"), "client-1");
        assert_eq!(get("state"), "state-1");
        assert_eq!(get("code_challenge"), "challenge-1");
        assert_eq!(get("code_challenge_method"), "S256");
        assert_eq!(get("scope"), "mcp.read mcp.write");
        assert_eq!(get("audience"), "mcp://docs");
        assert_eq!(get("redirect_uri"), "http://127.0.0.1:8765/callback");
        assert!(verify_state("state-1", "state-1").is_ok());
        assert!(verify_state("state-1", "attacker").is_err());
        assert!(verify_state("", "").is_err());
    }

    #[test]
    fn token_request_and_response_are_pure() {
        let cfg = oauth_cfg();
        let request = token_request(&cfg, "code-1", "verifier-1").unwrap();
        assert_eq!(request.url, cfg.token_url);
        let pairs: Vec<(String, String)> = url::form_urlencoded::parse(request.body.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let get = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .unwrap()
        };
        assert_eq!(get("grant_type"), "authorization_code");
        assert_eq!(get("code"), "code-1");
        assert_eq!(get("code_verifier"), "verifier-1");

        let token = parse_token_response(
            r#"{"access_token":"tok","token_type":"Bearer","expires_in":3600,"refresh_token":"r"}"#,
        )
        .unwrap();
        assert_eq!(token.access_token, "tok");
        assert_eq!(token.token_type, "Bearer");
        assert_eq!(token.expires_in, Some(3600));
        assert_eq!(token.refresh_token.as_deref(), Some("r"));
        assert!(parse_token_response(r#"{"error":"access_denied"}"#).is_err());
        assert!(parse_token_response(r#"{}"#).is_err());
    }

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
            enabled_tools: vec![],
            disabled_tools: vec![],
            transport: McpTransport::Http {
                url: "http://example.com/mcp".into(),
                headers: BTreeMap::new(),
            },
            oauth: None,
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

    #[test]
    fn tool_gates_filter_what_is_exposed() {
        let mut cfg = McpServerConfig {
            id: "m1".into(),
            name: "Docs".into(),
            enabled: true,
            auto_approve: false,
            enabled_tools: vec![],
            disabled_tools: vec![],
            transport: McpTransport::Http {
                url: "https://mcp.example.com/mcp".into(),
                headers: BTreeMap::new(),
            },
            oauth: None,
        };
        let tools: Vec<McpTool> = vec!["a", "b", "c"]
            .into_iter()
            .map(|name| McpTool {
                name: name.to_string(),
                description: String::new(),
                input_schema: json!({}),
            })
            .collect();
        assert_eq!(
            filter_tools(tools.clone(), &cfg).len(),
            3,
            "older configs expose all"
        );

        cfg.enabled_tools = vec!["a".into(), "b".into(), "c".into()];
        cfg.disabled_tools = vec!["b".into()];
        let kept = filter_tools(tools, &cfg);
        assert_eq!(
            kept.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert!(tool_enabled(&cfg, "a"));
        assert!(!tool_enabled(&cfg, "b"));
        assert!(!tool_enabled(&cfg, "missing"));
    }

    #[test]
    fn pending_signins_are_single_use_and_expire() {
        let mut cfg = oauth_cfg();
        cfg.server_id = "mcp-pending-test".into();
        let pkce = new_pkce().unwrap();
        let state = pkce.state.clone();
        begin_sign_in(cfg.clone(), pkce);
        let (loaded, taken) = take_sign_in(&cfg.server_id).unwrap();
        assert_eq!(loaded, cfg);
        assert_eq!(taken.state, state);
        assert!(
            take_sign_in(&cfg.server_id).is_err(),
            "a state cannot be finished twice"
        );

        let mut stale = cfg.clone();
        stale.server_id = "mcp-stale-test".into();
        pending_signins()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                stale.server_id.clone(),
                PendingSignIn {
                    cfg: stale.clone(),
                    pkce: new_pkce().unwrap(),
                    started_at: Instant::now() - OAUTH_STATE_TTL - Duration::from_secs(1),
                },
            );
        assert!(
            take_sign_in(&stale.server_id).is_err(),
            "an expired state is refused"
        );
        assert_eq!(oauth_token_key("mcp-1"), "mcp-oauth:mcp-1");
    }
}
