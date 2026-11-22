//! Developer mode (Settings → Developer): experimental flags, the modules
//! that make up the app, and the chat mode instructions. Every flag and
//! module listed here is wired to real behaviour; nothing is decorative.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::DeveloperConfig;
use crate::services::inference::modes::{ChatModeInfo, MODES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;

/// One switch in Settings → Developer.
#[derive(Debug, Clone, Serialize)]
pub struct Toggle {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// The value when the user has not changed it.
    pub default: bool,
}

/// Experimental features. Off unless the user turns them on, and they only
/// apply while developer mode is on.
pub const FLAGS: &[Toggle] = &[
    Toggle {
        id: "system_prompt_editor",
        label: "Edit the system prompt",
        description: "Write the chat's system prompt yourself, and see the instruction each chat mode adds.",
        default: true,
    },
    Toggle {
        id: "sampling_controls",
        label: "Sampling controls",
        description: "Adjust temperature and answer length from the chat box.",
        default: true,
    },
    Toggle {
        id: "answer_stats",
        label: "Answer statistics",
        description: "Show tokens, time and speed under each answer.",
        default: false,
    },
    Toggle {
        id: "raw_messages",
        label: "Raw message view",
        description: "Add a button to see an answer as plain text, without formatting.",
        default: false,
    },
    Toggle {
        id: "request_log",
        label: "Request log panel",
        description: "Show backend timings here. Calls are measured in memory even when this is off.",
        default: false,
    },
    Toggle {
        id: "no_motion",
        label: "Turn off animations",
        description: "Stop all transitions and animations, whatever the system setting is.",
        default: false,
    },
    Toggle {
        id: "compact",
        label: "Compact layout",
        description: "Tighter spacing in chat and lists, to fit more on screen.",
        default: false,
    },
    Toggle {
        id: "send_ctrl_enter",
        label: "Send with Ctrl+Enter",
        description: "Enter starts a new line, and Ctrl+Enter (Cmd+Enter on a Mac) sends the message.",
        default: false,
    },
    Toggle {
        id: "wide_chat",
        label: "Wide chat column",
        description: "Let messages and the chat box use the full width of the window.",
        default: false,
    },
    Toggle {
        id: "perf_overlay",
        label: "Performance overlay",
        description: "Show a small on-screen readout of frame time and memory use while the app runs.",
        default: false,
    },
    Toggle {
        id: "trace_ipc_sizes",
        label: "Trace IPC payload sizes",
        description: "Log the byte size of each window-to-backend message, to find oversized chat payloads.",
        default: false,
    },
    Toggle {
        id: "vector_search_debug",
        label: "Vector search debug",
        description: "Show embedding scores and which document chunks were retrieved during library search.",
        default: false,
    },
    Toggle {
        id: "mock_slow_backend",
        label: "Simulate a slow backend",
        description: "Add artificial delay to inference and search replies, to exercise loading and cancel states.",
        default: false,
    },
    Toggle {
        id: "show_keychain_state",
        label: "Show keychain state",
        description: "Display whether each provider key lives in the OS keychain or in the config file.",
        default: false,
    },
    Toggle {
        id: "log_redaction_preview",
        label: "Log redaction preview",
        description: "Preview which secrets would be redacted from the diagnostic log before you share it.",
        default: false,
    },
    Toggle {
        id: "agent_debug",
        label: "Agent debug view",
        description: "Show raw tool calls, permission decisions and hook output in the chat inspector.",
        default: false,
    },
    Toggle {
        id: "tab_debug",
        label: "Tab debug view",
        description: "List open mockup panels and their state, to debug embedded views.",
        default: false,
    },
];

/// Parts of the app that can be switched off. On unless switched off; this
/// applies with or without developer mode, so a module stays off after the
/// user leaves developer mode.
pub const MODULES: &[Toggle] = &[
    Toggle {
        id: "memory",
        label: "Notes and file recall",
        description: "Chat looks in your memory notes and document library. Off hides the Memory screen.",
        default: true,
    },
    Toggle {
        id: "search",
        label: "Search screen",
        description: "Search your documents and the web from their own screen.",
        default: true,
    },
    Toggle {
        id: "web_in_chat",
        label: "Web in chat",
        description: "Web search in the chat's Tools menu and the /web command, and web research in Autonomous mode.",
        default: true,
    },
    Toggle {
        id: "terminal",
        label: "Terminal commands",
        description: "Chat may run commands in the workspace folder. Commands that change something ask first; destructive ones never run.",
        default: true,
    },
    Toggle {
        id: "skills",
        label: "Skills",
        description: "The Skills screen and the / menu in chat.",
        default: true,
    },
    Toggle {
        id: "mcp",
        label: "MCP servers",
        description: "Tools from MCP servers you add in Settings → Tools and MCP.",
        default: true,
    },
    Toggle {
        id: "online",
        label: "Online providers",
        description: "Settings for cloud and router providers.",
        default: true,
    },
    Toggle {
        id: "opencode",
        label: "OpenCode link",
        description: "Send tasks from chat to an OpenCode server on this computer.",
        default: true,
    },
    Toggle {
        id: "command_palette",
        label: "Command palette",
        description: "Ctrl/⌘+K to jump between screens and run commands.",
        default: true,
    },
    Toggle {
        id: "session_search",
        label: "Session search",
        description: "Search across past conversations from the chat history and command palette.",
        default: true,
    },
    Toggle {
        id: "multi_run",
        label: "Multi-run comparison",
        description: "Send one prompt to several models or providers and compare the answers side by side.",
        default: true,
    },
    Toggle {
        id: "agent_files",
        label: "Agent files",
        description: "Load agent personas from ~/.aiassistant/agents and use them in chat.",
        default: true,
    },
    Toggle {
        id: "goal_mode",
        label: "Goal mode",
        description: "Let the assistant work towards a goal over several steps, with checkpoints you can review.",
        default: true,
    },
    Toggle {
        id: "remote_ui",
        label: "Remote UI panels",
        description: "Render the interactive mockup panels the assistant can draw in chat.",
        default: true,
    },
    Toggle {
        id: "plugin_panels",
        label: "Plugin panels",
        description: "Show panels contributed by installed plugins in the sidebar.",
        default: true,
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperSettings {
    pub enabled: bool,
    /// Every flag with its effective value (stored, or the default).
    pub flags: BTreeMap<String, bool>,
    /// Every module with its effective value.
    pub modules: BTreeMap<String, bool>,
    pub flag_list: Vec<Toggle>,
    pub module_list: Vec<Toggle>,
    pub modes: Vec<ChatModeInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeveloperUpdate {
    pub enabled: Option<bool>,
    /// Flags or modules to change; unknown ids are refused.
    #[serde(default)]
    pub flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub modules: BTreeMap<String, bool>,
}

fn effective(list: &[Toggle], stored: &BTreeMap<String, bool>) -> BTreeMap<String, bool> {
    list.iter()
        .map(|t| {
            (
                t.id.to_string(),
                stored.get(t.id).copied().unwrap_or(t.default),
            )
        })
        .collect()
}

fn settings(cfg: &DeveloperConfig) -> DeveloperSettings {
    DeveloperSettings {
        enabled: cfg.enabled,
        flags: effective(FLAGS, &cfg.flags),
        modules: effective(MODULES, &cfg.modules),
        flag_list: FLAGS.to_vec(),
        module_list: MODULES.to_vec(),
        modes: MODES.to_vec(),
    }
}

/// Apply an update to the stored settings. Pure, so it is tested directly.
pub fn apply_update(cfg: &mut DeveloperConfig, update: DeveloperUpdate) -> Result<(), AppError> {
    for id in update.flags.keys() {
        if !FLAGS.iter().any(|f| f.id == id) {
            return Err(AppError::invalid_input(format!("Unknown flag: {id}")));
        }
    }
    for id in update.modules.keys() {
        if !MODULES.iter().any(|m| m.id == id) {
            return Err(AppError::invalid_input(format!("Unknown module: {id}")));
        }
    }
    if let Some(enabled) = update.enabled {
        cfg.enabled = enabled;
    }
    cfg.flags.extend(update.flags);
    cfg.modules.extend(update.modules);
    Ok(())
}

#[tauri::command]
pub fn developer_get(state: State<'_, AppState>) -> Result<DeveloperSettings, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;
    Ok(settings(&cfg.developer))
}

#[tauri::command]
pub fn developer_set(
    state: State<'_, AppState>,
    update: DeveloperUpdate,
) -> Result<DeveloperSettings, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    apply_update(&mut cfg.developer, update)?;
    store.save(&cfg).map_err(AppError::internal)?;
    Ok(settings(&cfg.developer))
}

/// Restore every flag and module to its default and leave developer mode.
#[tauri::command]
pub fn developer_reset(state: State<'_, AppState>) -> Result<DeveloperSettings, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.developer = DeveloperConfig::default();
    store.save(&cfg).map_err(AppError::internal)?;
    Ok(settings(&cfg.developer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_merge_and_unknown_ids_are_refused() {
        let mut cfg = DeveloperConfig::default();
        let mut flags = BTreeMap::new();
        flags.insert("answer_stats".to_string(), true);
        apply_update(
            &mut cfg,
            DeveloperUpdate {
                enabled: Some(true),
                flags,
                modules: BTreeMap::new(),
            },
        )
        .unwrap();
        assert!(cfg.enabled);
        assert!(settings(&cfg).flags["answer_stats"]);
        assert!(!settings(&cfg).flags["raw_messages"]);
        assert!(settings(&cfg).modules["skills"]);

        let mut bad = BTreeMap::new();
        bad.insert("rm_rf".to_string(), true);
        let err = apply_update(
            &mut cfg,
            DeveloperUpdate {
                enabled: None,
                flags: bad,
                modules: BTreeMap::new(),
            },
        );
        assert!(err.is_err());
        assert!(cfg.enabled, "a refused update changes nothing");
    }

    #[test]
    fn every_toggle_id_is_unique() {
        let mut ids: Vec<&str> = FLAGS.iter().chain(MODULES).map(|t| t.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn policy_stream_toggles_are_registered_with_labels() {
        for id in [
            "perf_overlay",
            "trace_ipc_sizes",
            "vector_search_debug",
            "mock_slow_backend",
            "show_keychain_state",
            "log_redaction_preview",
            "agent_debug",
            "tab_debug",
        ] {
            let toggle = FLAGS.iter().find(|t| t.id == id).unwrap_or_else(|| {
                panic!("missing flag {id}");
            });
            assert!(!toggle.label.is_empty(), "flag {id} needs a label");
            assert!(
                !toggle.description.is_empty(),
                "flag {id} needs a description"
            );
        }
        for id in [
            "session_search",
            "multi_run",
            "agent_files",
            "goal_mode",
            "remote_ui",
            "plugin_panels",
        ] {
            let toggle = MODULES.iter().find(|t| t.id == id).unwrap_or_else(|| {
                panic!("missing module {id}");
            });
            assert!(!toggle.label.is_empty(), "module {id} needs a label");
            assert!(
                !toggle.description.is_empty(),
                "module {id} needs a description"
            );
        }
    }
}
