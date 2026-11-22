//! Policy / configuration commands: permission rules, tool hooks, routing and
//! agent files. All values live in the local AppConfig (or as plain agent
//! markdown files); nothing here talks to a network.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::commands::cloud::{load_config, save_config};
use crate::config::{
    AppConfig, HooksConfig, PermissionRule, RoutingConfig, ToolDecision,
};
use crate::services::agents::{Agent, AgentsService};

/// Everything the UI needs to show and edit the policy.
#[derive(Debug, Clone, Serialize)]
pub struct PolicySettings {
    pub permissions: Vec<PermissionRule>,
    pub hooks: HooksConfig,
    pub routing: RoutingConfig,
    /// Effective provider order (from the priority list or config order).
    pub provider_order: Vec<String>,
}

impl From<&AppConfig> for PolicySettings {
    fn from(cfg: &AppConfig) -> Self {
        Self {
            permissions: cfg.permissions.clone(),
            hooks: cfg.hooks.clone(),
            routing: cfg.routing.clone(),
            provider_order: cfg.provider_order(),
        }
    }
}

/// `None` fields are left untouched, so a partial update never clears the
/// other settings by accident.
#[derive(Debug, Clone, Deserialize)]
pub struct PolicyUpdate {
    #[serde(default)]
    pub permissions: Option<Vec<PermissionRule>>,
    #[serde(default)]
    pub hooks: Option<HooksConfig>,
    #[serde(default)]
    pub routing: Option<RoutingConfig>,
}

#[tauri::command]
pub fn policy_config_get(state: State<'_, AppState>) -> Result<PolicySettings, AppError> {
    let cfg = load_config(&state)?;
    Ok(PolicySettings::from(&cfg))
}

#[tauri::command]
pub fn policy_config_set(
    state: State<'_, AppState>,
    update: PolicyUpdate,
) -> Result<PolicySettings, AppError> {
    let mut cfg = load_config(&state)?;
    if let Some(rules) = update.permissions {
        for rule in &rules {
            if rule.action.trim().is_empty() || rule.resource.trim().is_empty() {
                return Err(AppError::invalid_input(
                    "Every permission rule needs an action and a resource.",
                ));
            }
        }
        cfg.permissions = rules;
    }
    if let Some(hooks) = update.hooks {
        cfg.hooks = hooks;
    }
    if let Some(routing) = update.routing {
        cfg.routing = routing;
    }
    save_config(&state, &cfg)?;
    // Make the new policy effective immediately: tool calls use the rules and
    // hooks, and the engine uses the provider order and key pool.
    state.tools.apply_config(&cfg);
    let provider_id = cfg.active_cloud_provider.clone();
    state
        .inference_engine
        .apply_routing(&cfg, provider_id.as_deref());
    Ok(PolicySettings::from(&cfg))
}

/// Preview the decision a tool call would get, without running it.
#[tauri::command]
pub fn policy_decide(
    state: State<'_, AppState>,
    action: String,
    resource: String,
) -> ToolDecision {
    let cfg = load_config(&state).unwrap_or_default();
    cfg.decide_tool_call(&action, &resource)
}

/// Agent files (markdown in `~/.aiassistant/agents`).
#[tauri::command]
pub fn agents_list() -> Result<Vec<Agent>, AppError> {
    AgentsService::new().list()
}

#[tauri::command]
pub fn agents_read(path: String) -> Result<Agent, AppError> {
    AgentsService::new().read(&path)
}

#[tauri::command]
pub fn agents_save(
    name: String,
    model: Option<String>,
    tools: Option<Vec<String>>,
    prompt: String,
) -> Result<Agent, AppError> {
    AgentsService::new().save(
        &name,
        model.as_deref().unwrap_or(""),
        tools.unwrap_or_default(),
        &prompt,
    )
}

/// Authoritatively set an MCP server's tool gates. An empty allow-list means
/// "all tools"; the deny-list always wins.
#[tauri::command]
pub fn mcp_server_set_tools(
    state: State<'_, AppState>,
    id: String,
    enabled_tools: Vec<String>,
    disabled_tools: Vec<String>,
) -> Result<(), AppError> {
    let mut cfg = load_config(&state)?;
    let server = cfg
        .mcp_servers
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::invalid_input("That server no longer exists."))?;
    server.enabled_tools = enabled_tools;
    server.disabled_tools = disabled_tools;
    save_config(&state, &cfg)?;
    Ok(())
}
