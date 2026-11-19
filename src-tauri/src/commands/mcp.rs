//! Settings → Tools and MCP: the user's MCP servers.
//!
//! Header and environment values can hold tokens, so they never travel back
//! to the window; it gets a short hint instead, and a save without a new
//! value keeps the stored one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::commands::cloud::{load_config, save_config};
use crate::config::{McpServerConfig, McpTransport};
use crate::services::mcp::presets::{McpPreset, PRESETS};
use crate::services::mcp::{import::parse_import, validate};

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
    /// "stdio" or "http".
    pub kind: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub url: String,
    pub env: Vec<KeyValueHint>,
    pub headers: Vec<KeyValueHint>,
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
            kind: "stdio".into(),
            command: command.clone(),
            args: args.clone(),
            cwd: cwd.clone(),
            url: String::new(),
            env: hints(env),
            headers: Vec::new(),
            connected,
        },
        McpTransport::Http { url, headers } => McpServerDto {
            id: s.id.clone(),
            name: s.name.clone(),
            enabled: s.enabled,
            auto_approve: s.auto_approve,
            kind: "http".into(),
            command: String::new(),
            args: Vec::new(),
            cwd: None,
            url: url.clone(),
            env: Vec::new(),
            headers: hints(headers),
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

/// Closing a session can wait on a program or a web address, so the
/// commands that change servers run off the window's thread.
#[tauri::command]
pub async fn mcp_server_save(
    state: State<'_, AppState>,
    req: McpServerSaveRequest,
) -> Result<Vec<McpServerDto>, AppError> {
    with_app_state(&state, move |owned| save_server(&owned, req)).await
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
    let server = McpServerConfig {
        id: existing
            .as_ref()
            .map(|s| s.id.clone())
            .unwrap_or_else(new_id),
        name: req.name.trim().to_string(),
        enabled: req.enabled,
        auto_approve: req.auto_approve,
        transport,
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
            transport,
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
}
