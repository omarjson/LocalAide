//! Reads MCP server lists written for other apps, so a user can paste the
//! block from a server's README or another client's config file:
//!
//! - Claude Desktop, Cursor and Windsurf: `{"mcpServers": {name: {command, args, env} | {url, headers}}}`
//! - VS Code: `{"servers": {name: {type: "stdio" | "http", ...}}}`
//! - OpenCode: `{"mcp": {name: {type: "local", command: [...], environment} | {type: "remote", url, headers}}}`
//! - Crush: `{"mcp": {name: {type: "stdio" | "http", command, args, env, url, headers}}}`
//! - A bare `{name: {...}}` map, or one server object.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::app::error::AppError;
use crate::config::McpTransport;

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let v = match v {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => return None,
                    };
                    Some((k.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// One server object in any of the supported shapes.
pub fn parse_server(entry: &Value) -> Option<McpTransport> {
    let obj = entry.as_object()?;
    let kind = obj
        .get("type")
        .or_else(|| obj.get("transport"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let url = obj
        .get("url")
        .or_else(|| obj.get("serverUrl"))
        .and_then(Value::as_str);
    if let Some(url) = url
        && !matches!(kind.as_str(), "stdio" | "local")
    {
        return Some(McpTransport::Http {
            url: url.trim().to_string(),
            headers: string_map(obj.get("headers")),
        });
    }
    let env = {
        let mut env = string_map(obj.get("env"));
        env.extend(string_map(obj.get("environment")));
        env
    };
    let cwd = obj.get("cwd").and_then(Value::as_str).map(str::to_string);
    match obj.get("command") {
        // OpenCode writes the program and its arguments as one list.
        Some(Value::Array(parts)) => {
            let mut parts = parts.iter().filter_map(Value::as_str).map(str::to_string);
            let command = parts.next()?;
            let mut args: Vec<String> = parts.collect();
            args.extend(strings(obj.get("args")));
            Some(McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            })
        }
        Some(Value::String(command)) if !command.trim().is_empty() => Some(McpTransport::Stdio {
            command: command.trim().to_string(),
            args: strings(obj.get("args")),
            env,
            cwd,
        }),
        _ => None,
    }
}

fn servers_in(map: &Map<String, Value>) -> Vec<(String, McpTransport, bool)> {
    map.iter()
        .filter_map(|(name, entry)| {
            let enabled = entry
                .get("enabled")
                .and_then(Value::as_bool)
                .or_else(|| entry.get("disabled").and_then(Value::as_bool).map(|d| !d))
                .unwrap_or(true);
            parse_server(entry).map(|t| (name.clone(), t, enabled))
        })
        .collect()
}

/// Every server in a pasted block, as (name, transport, enabled).
pub fn parse_import(text: &str) -> Result<Vec<(String, McpTransport, bool)>, AppError> {
    let value: Value = serde_json::from_str(text.trim())
        .map_err(|e| AppError::invalid_input(format!("That is not valid JSON: {e}")))?;
    let Some(root) = value.as_object() else {
        return Err(AppError::invalid_input(
            "Paste a JSON object with your MCP servers.",
        ));
    };
    for key in ["mcpServers", "servers", "mcp", "mcp_servers"] {
        if let Some(Value::Object(map)) = root.get(key) {
            let found = servers_in(map);
            if !found.is_empty() {
                return Ok(found);
            }
        }
    }
    // One server on its own.
    if let Some(transport) = parse_server(&value) {
        let name = root
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("MCP server")
            .to_string();
        return Ok(vec![(name, transport, true)]);
    }
    let found = servers_in(root);
    if found.is_empty() {
        return Err(AppError::invalid_input(
            "No MCP servers were found in that text. It should list servers with a \"command\" or a \"url\".",
        ));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_desktop_format() {
        let found = parse_import(
            r#"{"mcpServers": {
                "fs": {"command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"], "env": {"A": "1"}},
                "docs": {"url": "https://mcp.context7.com/mcp", "headers": {"Authorization": "Bearer x"}},
                "off": {"command": "uvx", "disabled": true}
            }}"#,
        )
        .unwrap();
        assert_eq!(found.len(), 3);
        let docs = found.iter().find(|f| f.0 == "docs").unwrap();
        assert!(
            matches!(&docs.1, McpTransport::Http { headers, .. } if headers["Authorization"] == "Bearer x")
        );
        let fs = found.iter().find(|f| f.0 == "fs").unwrap();
        assert!(
            matches!(&fs.1, McpTransport::Stdio { command, args, env, .. } if command == "npx" && args.len() == 3 && env["A"] == "1")
        );
        assert!(!found.iter().find(|f| f.0 == "off").unwrap().2);
    }

    #[test]
    fn opencode_and_crush_formats() {
        let found = parse_import(
            r#"{"mcp": {
                "local": {"type": "local", "command": ["bunx", "my-server", "--x"], "environment": {"K": "v"}, "enabled": false},
                "remote": {"type": "remote", "url": "https://mcp.deepwiki.com/mcp"},
                "crush": {"type": "stdio", "command": "node", "args": ["s.js"]}
            }}"#,
        )
        .unwrap();
        let local = found.iter().find(|f| f.0 == "local").unwrap();
        assert!(
            matches!(&local.1, McpTransport::Stdio { command, args, .. } if command == "bunx" && args == &["my-server", "--x"])
        );
        assert!(!local.2);
        assert!(matches!(
            &found.iter().find(|f| f.0 == "remote").unwrap().1,
            McpTransport::Http { .. }
        ));
    }

    #[test]
    fn single_servers_and_junk() {
        let one = parse_import(r#"{"name": "Exa", "url": "https://mcp.exa.ai/mcp"}"#).unwrap();
        assert_eq!(one[0].0, "Exa");
        assert!(parse_import("not json").is_err());
        assert!(parse_import(r#"{"a": 1}"#).is_err());
    }
}
