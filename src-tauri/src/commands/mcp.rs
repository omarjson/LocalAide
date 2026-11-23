//! Settings → Tools and MCP: the user's MCP servers.
//!
//! Header and environment values can hold tokens, so they never travel back
//! to the window; it gets a short hint instead, and a save without a new
//! value keeps the stored one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app::blocking::{run_blocking, with_app_state};
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::commands::cloud::{load_config, save_config};
use crate::config::{McpOAuthConfig, McpServerConfig, McpTransport};
use crate::services::mcp::import::parse_import;
use crate::services::mcp::presets::{McpPreset, PRESETS};
use crate::services::mcp::{
    DEFAULT_OAUTH_REDIRECT_URI, authorize_url, begin_sign_in, exchange_code, new_pkce,
    oauth_token_key, take_sign_in, validate, verify_state,
};

#[derive(Debug, Clone, Serialize)]
pub struct KeyValueHint {
    pub name: String,
    /// "••••" plus the last characters of longer values.
    pub hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpServerDto {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub auto_approve: bool,
    /// Tool names explicitly exposed to the model. Empty = all tools.
    pub enabled_tools: Vec<String>,
    /// Tool names never exposed to the model.
    pub disabled_tools: Vec<String>,
    /// "stdio" or "http".
    pub kind: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub url: String,
    pub env: Vec<KeyValueHint>,
    pub headers: Vec<KeyValueHint>,
    /// OAuth client metadata. Never contains a token; empty when unset.
    pub oauth: Option<McpOAuthConfig>,
    /// A session is open right now.
    pub connected: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyValueInput {
    pub name: String,
    /// None keeps the stored value for this name.
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerSaveRequest {
    pub id: Option<String>,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub enabled_tools: Vec<String>,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    pub kind: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub env: Vec<KeyValueInput>,
    #[serde(default)]
    pub headers: Vec<KeyValueInput>,
    /// Omit (or send null) on an edit to keep the stored OAuth settings.
    #[serde(default)]
    pub oauth: Option<McpOAuthConfig>,
}

/// OAuth 2.1 PKCE sign-in. The window only ever sees public client metadata,
/// the authorization URL, the state and a safe status — never a token.
#[derive(Debug, Clone, Deserialize)]
pub struct McpOAuthStartRequest {
    pub server_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpOAuthStartDto {
    /// Open this in the user's browser.
    pub authorize_url: String,
    /// Echoed back by the authorization server; passed to `mcp_oauth_finish`.
    pub state: String,
    /// The loopback address the code is sent to.
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpOAuthFinishRequest {
    pub server_id: String,
    pub state: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpOAuthStatusDto {
    pub server_id: String,
    pub signed_in: bool,
    /// Plain words for the UI. No token, verifier or header is ever returned.
    pub message: String,
    /// From the token response; "Bearer" in practice. Safe to display.
    pub token_type: String,
    /// From the token response; safe to display.
    pub expires_in: Option<u64>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpToggleRequest {
    pub id: String,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub auto_approve: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolDto {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpImportResult {
    pub added: usize,
    /// Servers already in the list (same name and settings).
    pub skipped: usize,
    pub servers: Vec<McpServerDto>,
}

fn hint(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= 8 {
        "••••".to_string()
    } else {
        format!(
            "••••{}",
            chars[chars.len() - 4..].iter().collect::<String>()
        )
    }
}

fn hints(map: &BTreeMap<String, String>) -> Vec<KeyValueHint> {
    map.iter()
        .map(|(name, value)| KeyValueHint {
            name: name.clone(),
            hint: hint(value),
        })
        .collect()
}

fn dto(state: &AppState, s: &McpServerConfig) -> McpServerDto {
    let connected = state.mcp.is_connected(&s.id);
    match &s.transport {
        McpTransport::Stdio {
            command,
            args,
            env,
            cwd,
        } => McpServerDto {
            id: s.id.clone(),
            name: s.name.clone(),
            enabled: s.enabled,
            auto_approve: s.auto_approve,
            enabled_tools: s.enabled_tools.clone(),
            disabled_tools: s.disabled_tools.clone(),
            kind: "stdio".into(),
            command: command.clone(),
            args: args.clone(),
            cwd: cwd.clone(),
            url: String::new(),
            env: hints(env),
            headers: Vec::new(),
            oauth: s.oauth.clone(),
            connected,
        },
        McpTransport::Http { url, headers } => McpServerDto {
            id: s.id.clone(),
            name: s.name.clone(),
            enabled: s.enabled,
            auto_approve: s.auto_approve,
            enabled_tools: s.enabled_tools.clone(),
            disabled_tools: s.disabled_tools.clone(),
            kind: "http".into(),
            command: String::new(),
            args: Vec::new(),
            cwd: None,
            url: url.clone(),
            env: Vec::new(),
            headers: hints(headers),
            oauth: s.oauth.clone(),
            connected,
        },
    }
}

fn list(state: &AppState, cfg: &crate::config::AppConfig) -> Vec<McpServerDto> {
    cfg.mcp_servers.iter().map(|s| dto(state, s)).collect()
}

/// Merge typed pairs with the stored ones: a pair without a value keeps
/// the stored value; a pair missing from the list is removed.
fn merge(inputs: &[KeyValueInput], stored: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    inputs
        .iter()
        .filter(|kv| !kv.name.trim().is_empty())
        .filter_map(|kv| {
            let name = kv.name.trim().to_string();
            let value = match &kv.value {
                Some(v) => Some(v.clone()),
                None => stored.get(&name).cloned(),
            }?;
            Some((name, value))
        })
        .collect()
}

fn new_id() -> String {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        bytes = nanos.to_le_bytes();
    }
    format!(
        "mcp-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[tauri::command]
pub fn mcp_servers_list(state: State<'_, AppState>) -> Result<Vec<McpServerDto>, AppError> {
    let cfg = load_config(&state)?;
    Ok(list(&state, &cfg))
}

#[tauri::command]
pub fn mcp_presets() -> Vec<McpPreset> {
    PRESETS.to_vec()
}

/// One MCP server found in ZCode's own plugin cache, ready to add with a
/// click. ArtCraft family: the documents, pdf, presentations, spreadsheets
/// and photocraft panels ship MCP servers that speak the same protocol.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArtCraftPreset {
    /// Suggested server id, "artcraft-<plugin>".
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: std::collections::BTreeMap<String, String>,
    /// Whether the server entry file is really on disk.
    pub ready: bool,
    /// Whether an MCP server with this id is already configured.
    pub added: bool,
}

/// Scan ZCode's plugin cache for plugins that ship MCP servers.
#[tauri::command]
pub fn mcp_artcraft_presets(state: State<'_, AppState>) -> Vec<ArtCraftPreset> {
    let configured: std::collections::HashSet<String> = load_config(&state)
        .map(|cfg| cfg.mcp_servers.iter().map(|s| s.id.clone()).collect())
        .unwrap_or_default();
    scan_plugin_cache(&configured)
}

fn scan_plugin_cache(configured: &std::collections::HashSet<String>) -> Vec<ArtCraftPreset> {
    let Some(home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
    else {
        return Vec::new();
    };
    let cache = home
        .join(".zcode")
        .join("cli")
        .join("plugins")
        .join("cache")
        .join("zcode-plugins-official");
    let Ok(entries) = std::fs::read_dir(&cache) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        let plugin_name = entry.file_name().to_string_lossy().to_string();
        // Newest version folder wins; versions sort like 0.2.10 vs 0.2.9,
        // which string sort gets wrong, so compare numerically.
        let Ok(versions) = std::fs::read_dir(&plugin_dir) else {
            continue;
        };
        let mut version_dirs: Vec<(std::path::PathBuf, (u64, u64, u64))> = versions
            .flatten()
            .filter(|v| v.path().join(".zcode-plugin").join("plugin.json").is_file())
            .filter_map(|v| {
                let name = v.file_name().to_string_lossy().to_string();
                let parts: Vec<u64> = name.split('.').map(|p| p.parse().unwrap_or(0)).collect();
                if parts.len() == 3 {
                    Some((v.path(), (parts[0], parts[1], parts[2])))
                } else {
                    None
                }
            })
            .collect();
        version_dirs.sort_by_key(|a| std::cmp::Reverse(a.1));
        let Some((dir, _)) = version_dirs.into_iter().next() else {
            continue;
        };
        let version = dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let Ok(manifest) = std::fs::read_to_string(dir.join(".zcode-plugin").join("plugin.json"))
        else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&manifest) else {
            continue;
        };
        let Some(servers) = json.get("mcpServers").and_then(|m| m.as_object()) else {
            continue;
        };
        let (server_name, server) = match servers.iter().next() {
            Some(pair) => pair,
            None => continue,
        };
        let command = server
            .get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("node")
            .to_string();
        let raw_args: Vec<String> = server
            .get("args")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|v| v.replace("${ZCODE_PLUGIN_ROOT}", &dir.to_string_lossy()))
                    .collect()
            })
            .unwrap_or_default();
        let cwd = server
            .get("cwd")
            .and_then(|c| c.as_str())
            .map(|c| c.replace("${ZCODE_PROJECT_DIR}", ""));
        let env: std::collections::BTreeMap<String, String> = server
            .get("env")
            .and_then(|e| e.as_object())
            .map(|e| {
                e.iter()
                    .filter_map(|(k, v)| {
                        v.as_str()
                            .map(|v| (k.clone(), v.replace("${ZCODE_PROJECT_DIR}", "")))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // The server entry must exist for the preset to be ready to run.
        let entry = raw_args.iter().find(|a| a.ends_with(".mjs"));
        let ready = entry
            .map(|e| std::path::Path::new(e).is_file())
            .unwrap_or(false);
        let id = format!("artcraft-{plugin_name}");
        let name = json
            .get("description")
            .and_then(|d| d.as_str())
            .map(|d| {
                let plugin = plugin_name
                    .chars()
                    .next()
                    .unwrap_or('a')
                    .to_uppercase()
                    .collect::<String>()
                    + &plugin_name[1..];
                format!("{plugin} — {}", d.split('.').next().unwrap_or(d))
            })
            .unwrap_or_else(|| format!("ArtCraft {plugin_name}"));
        out.push(ArtCraftPreset {
            added: configured.contains(&id),
            id,
            name,
            description: server_name.clone(),
            version,
            command,
            args: raw_args,
            cwd,
            env,
            ready,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Closing a session can wait on a program or a web address, so the
/// commands that change servers run off the window's thread.
#[tauri::command]
pub async fn mcp_server_save(
    state: State<'_, AppState>,
    req: McpServerSaveRequest,
) -> Result<Vec<McpServerDto>, AppError> {
    with_app_state(&state, move |owned| save_server(&owned, req)).await
}

/// Normalize and check OAuth settings before they reach config.json. Only
/// public client metadata is stored here; the token lives in the keychain.
/// Both endpoints follow the same HTTPS-or-loopback rule as other providers.
fn prepare_oauth(mut oauth: McpOAuthConfig, server_id: &str) -> Result<McpOAuthConfig, AppError> {
    oauth.server_id = server_id.to_string();
    if oauth.redirect_uri.trim().is_empty() {
        oauth.redirect_uri = DEFAULT_OAUTH_REDIRECT_URI.to_string();
    }
    oauth.client_id = oauth.client_id.trim().to_string();
    oauth.authorize_url = oauth.authorize_url.trim().to_string();
    oauth.token_url = oauth.token_url.trim().to_string();
    oauth.redirect_uri = oauth.redirect_uri.trim().to_string();
    if oauth.client_id.is_empty() {
        return Err(AppError::invalid_input("Enter the OAuth client id."));
    }
    crate::config::validate_base_url(&oauth.authorize_url).map_err(AppError::invalid_input)?;
    crate::config::validate_base_url(&oauth.token_url).map_err(AppError::invalid_input)?;
    crate::config::validate_base_url(&oauth.redirect_uri).map_err(AppError::invalid_input)?;
    Ok(oauth)
}

fn save_server(state: &AppState, req: McpServerSaveRequest) -> Result<Vec<McpServerDto>, AppError> {
    let mut cfg = load_config(state)?;
    let existing = req
        .id
        .as_deref()
        .and_then(|id| cfg.mcp_servers.iter().find(|s| s.id == id))
        .cloned();
    if req.id.is_some() && existing.is_none() {
        return Err(AppError::invalid_input("That server no longer exists."));
    }
    let (stored_env, stored_headers) = match existing.as_ref().map(|s| &s.transport) {
        Some(McpTransport::Stdio { env, .. }) => (env.clone(), BTreeMap::new()),
        Some(McpTransport::Http { headers, .. }) => (BTreeMap::new(), headers.clone()),
        None => (BTreeMap::new(), BTreeMap::new()),
    };
    let transport = match req.kind.as_str() {
        "stdio" => McpTransport::Stdio {
            command: req.command.trim().to_string(),
            args: req
                .args
                .iter()
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .collect(),
            env: merge(&req.env, &stored_env),
            cwd: req.cwd.clone().filter(|c| !c.trim().is_empty()),
        },
        "http" => {
            // A stored token goes only to the address it was entered for.
            let same_site = matches!(
                existing.as_ref().map(|s| &s.transport),
                Some(McpTransport::Http { url, .. }) if same_origin(url, req.url.trim())
            );
            let stored = if same_site {
                stored_headers
            } else {
                BTreeMap::new()
            };
            McpTransport::Http {
                url: req.url.trim().to_string(),
                headers: merge(&req.headers, &stored),
            }
        }
        _ => {
            return Err(AppError::invalid_input(
                "Choose a local program or a web address.",
            ));
        }
    };
    let id = existing
        .as_ref()
        .map(|s| s.id.clone())
        .unwrap_or_else(new_id);
    // OAuth belongs to HTTP servers only. A client that omits it keeps the
    // stored settings, so a name-only edit cannot silently drop sign-in.
    let oauth = if req.kind == "http" {
        match req
            .oauth
            .clone()
            .or_else(|| existing.as_ref().and_then(|s| s.oauth.clone()))
        {
            Some(config) => Some(prepare_oauth(config, &id)?),
            None => None,
        }
    } else {
        None
    };
    let server = McpServerConfig {
        id,
        name: req.name.trim().to_string(),
        enabled: req.enabled,
        auto_approve: req.auto_approve,
        // Older clients omit these fields; keep whatever was stored so a
        // save that only changes the name does not silently drop tool gates.
        enabled_tools: if req.enabled_tools.is_empty() {
            existing
                .as_ref()
                .map(|s| s.enabled_tools.clone())
                .unwrap_or_default()
        } else {
            req.enabled_tools
        },
        disabled_tools: if req.disabled_tools.is_empty() {
            existing
                .as_ref()
                .map(|s| s.disabled_tools.clone())
                .unwrap_or_default()
        } else {
            req.disabled_tools
        },
        transport,
        oauth,
    };
    validate(&server)?;
    if cfg
        .mcp_servers
        .iter()
        .any(|s| s.id != server.id && s.name.eq_ignore_ascii_case(&server.name))
    {
        return Err(AppError::invalid_input(
            "Another server has this name. Pick a different one.",
        ));
    }
    state.mcp.disconnect(&server.id);
    match cfg.mcp_servers.iter_mut().find(|s| s.id == server.id) {
        Some(slot) => *slot = server,
        None => cfg.mcp_servers.push(server),
    }
    save_config(state, &cfg)?;
    Ok(list(state, &cfg))
}

fn same_origin(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    }
}

#[tauri::command]
pub async fn mcp_server_toggle(
    state: State<'_, AppState>,
    req: McpToggleRequest,
) -> Result<Vec<McpServerDto>, AppError> {
    with_app_state(&state, move |owned| toggle_server(&owned, req)).await
}

fn toggle_server(state: &AppState, req: McpToggleRequest) -> Result<Vec<McpServerDto>, AppError> {
    let mut cfg = load_config(state)?;
    let Some(server) = cfg.mcp_servers.iter_mut().find(|s| s.id == req.id) else {
        return Err(AppError::invalid_input("That server no longer exists."));
    };
    if let Some(enabled) = req.enabled {
        server.enabled = enabled;
        if !enabled {
            state.mcp.disconnect(&req.id);
        }
    }
    if let Some(auto) = req.auto_approve {
        server.auto_approve = auto;
    }
    save_config(state, &cfg)?;
    Ok(list(state, &cfg))
}

#[tauri::command]
pub async fn mcp_server_delete(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<McpServerDto>, AppError> {
    with_app_state(&state, move |owned| delete_server(&owned, &id)).await
}

fn delete_server(state: &AppState, id: &str) -> Result<Vec<McpServerDto>, AppError> {
    let mut cfg = load_config(state)?;
    let before = cfg.mcp_servers.len();
    cfg.mcp_servers.retain(|s| s.id != id);
    if cfg.mcp_servers.len() == before {
        return Err(AppError::invalid_input("That server no longer exists."));
    }
    state.mcp.disconnect(id);
    save_config(state, &cfg)?;
    Ok(list(state, &cfg))
}

/// Connect to a server and list its tools. Starts a local server's program.
#[tauri::command]
pub async fn mcp_server_test(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<McpToolDto>, AppError> {
    with_app_state(&state, move |owned| {
        let cfg = load_config(&owned)?;
        let server = cfg
            .mcp_servers
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| AppError::invalid_input("That server no longer exists."))?;
        // A fresh session, so the test shows the server as it is now.
        owned.mcp.disconnect(&server.id);
        let tools = owned.mcp.list_tools(&server)?;
        Ok(tools
            .into_iter()
            .map(|t| McpToolDto {
                name: t.name,
                description: t.description.chars().take(300).collect(),
            })
            .collect())
    })
    .await
}

/// Add every server from a pasted config block (Claude Desktop, Cursor,
/// VS Code, OpenCode or Crush format).
#[tauri::command]
pub fn mcp_import(state: State<'_, AppState>, text: String) -> Result<McpImportResult, AppError> {
    if text.len() > 512 * 1024 {
        return Err(AppError::invalid_input("That text is too long."));
    }
    let found = parse_import(&text)?;
    let mut cfg = load_config(&state)?;
    let mut added = 0;
    let mut skipped = 0;
    for (name, transport, enabled) in found {
        let name = name.trim().chars().take(60).collect::<String>();
        if cfg
            .mcp_servers
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(&name) || s.transport == transport)
        {
            skipped += 1;
            continue;
        }
        let server = McpServerConfig {
            id: new_id(),
            name,
            enabled,
            auto_approve: false,
            enabled_tools: vec![],
            disabled_tools: vec![],
            transport,
            oauth: None,
        };
        if validate(&server).is_err() {
            skipped += 1;
            continue;
        }
        cfg.mcp_servers.push(server);
        added += 1;
    }
    save_config(&state, &cfg)?;
    Ok(McpImportResult {
        added,
        skipped,
        servers: list(&state, &cfg),
    })
}

/// Begin a PKCE sign-in. Reads the saved OAuth metadata, creates a fresh
/// verifier/state pair, and hands the browser an authorization URL. The
/// verifier stays in this process; only the URL and state go to the window.
#[tauri::command]
pub async fn mcp_oauth_start(
    state: State<'_, AppState>,
    req: McpOAuthStartRequest,
) -> Result<McpOAuthStartDto, AppError> {
    with_app_state(&state, move |owned| oauth_start(&owned, &req.server_id)).await
}

fn oauth_start(state: &AppState, server_id: &str) -> Result<McpOAuthStartDto, AppError> {
    let cfg = load_config(state)?;
    let server = cfg
        .mcp_servers
        .iter()
        .find(|s| s.id == server_id)
        .ok_or_else(|| AppError::invalid_input("That server no longer exists."))?;
    let oauth = server.oauth.as_ref().ok_or_else(|| {
        AppError::invalid_input("Save an authorization URL and token URL for this server first.")
    })?;
    let pkce = new_pkce()?;
    let authorize = authorize_url(oauth, &pkce.state, &pkce.challenge)?;
    let redirect_uri = oauth.redirect_uri.clone();
    begin_sign_in(oauth.clone(), pkce.clone());
    Ok(McpOAuthStartDto {
        authorize_url: authorize,
        state: pkce.state,
        redirect_uri,
    })
}

/// Finish a PKCE sign-in: check the one-time state, exchange the code, and
/// put the access token in the OS keychain under a stable key id. Returns
/// only a safe status — the token is never logged or sent to the window.
#[tauri::command]
pub async fn mcp_oauth_finish(req: McpOAuthFinishRequest) -> Result<McpOAuthStatusDto, AppError> {
    run_blocking(move || oauth_finish(&req)).await
}

fn oauth_finish(req: &McpOAuthFinishRequest) -> Result<McpOAuthStatusDto, AppError> {
    let (cfg, pkce) = take_sign_in(&req.server_id)?;
    verify_state(&pkce.state, req.state.trim())?;
    let code = req.code.trim();
    if code.is_empty() {
        return Err(AppError::invalid_input(
            "Paste the code from the callback address.",
        ));
    }
    let token = exchange_code(&cfg, code, &pkce.verifier)?;
    // Only the key id is derived from the server id; the token itself is
    // never used in a log line or a DTO.
    crate::services::secrets::save(&oauth_token_key(&req.server_id), &token.access_token)?;
    Ok(McpOAuthStatusDto {
        server_id: req.server_id.clone(),
        signed_in: true,
        message: "Signed in. The access token is kept in your OS keychain, not in the config file."
            .to_string(),
        token_type: token.token_type,
        expires_in: token.expires_in,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_values_are_kept_unless_replaced() {
        let stored = BTreeMap::from([
            ("TOKEN".to_string(), "secret-value-1234".to_string()),
            ("OLD".to_string(), "x".to_string()),
        ]);
        let merged = merge(
            &[
                KeyValueInput {
                    name: "TOKEN".into(),
                    value: None,
                },
                KeyValueInput {
                    name: "NEW".into(),
                    value: Some("v".into()),
                },
                KeyValueInput {
                    name: "MISSING".into(),
                    value: None,
                },
            ],
            &stored,
        );
        assert_eq!(merged["TOKEN"], "secret-value-1234");
        assert_eq!(merged["NEW"], "v");
        assert!(!merged.contains_key("OLD"));
        assert!(!merged.contains_key("MISSING"));
        assert_eq!(hint("secret-value-1234"), "••••1234");
        assert_eq!(hint("short"), "••••");
    }

    #[test]
    fn oauth_endpoints_must_be_https_or_loopback() {
        let base = McpOAuthConfig {
            client_id: "client-1".into(),
            authorize_url: "https://auth.example.com/authorize".into(),
            token_url: "https://auth.example.com/token".into(),
            ..McpOAuthConfig::default()
        };
        let saved = prepare_oauth(base.clone(), "mcp-1").unwrap();
        assert_eq!(saved.server_id, "mcp-1");
        assert_eq!(saved.redirect_uri, DEFAULT_OAUTH_REDIRECT_URI);

        let mut plain = base.clone();
        plain.token_url = "http://auth.example.com/token".into();
        assert!(prepare_oauth(plain, "mcp-1").is_err());

        let mut loopback = base;
        loopback.authorize_url = "http://127.0.0.1:9000/authorize".into();
        loopback.token_url = "http://localhost:9000/token".into();
        assert!(prepare_oauth(loopback, "mcp-1").is_ok());
    }
}
