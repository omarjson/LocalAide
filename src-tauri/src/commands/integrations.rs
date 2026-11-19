//! Settings → Integrations: the link to an OpenCode server.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::opencode::{DEFAULT_URL, OpenCodeClient, OpenCodeHandoff, OpenCodeHealth};
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct OpenCodeSettings {
    /// The saved address, or None when OpenCode is not linked.
    pub base_url: Option<String>,
    pub default_url: String,
    pub has_password: bool,
}

fn load(state: &AppState) -> Result<crate::config::AppConfig, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    store.load().map_err(AppError::internal)
}

fn client(state: &AppState) -> Result<OpenCodeClient, AppError> {
    let cfg = load(state)?;
    let base = cfg.opencode.base_url.ok_or_else(|| {
        AppError::new(
            "OPENCODE_NOT_LINKED",
            "Link OpenCode in Settings → Integrations first.",
        )
    })?;
    OpenCodeClient::new(&base, cfg.opencode.password.as_deref())
}

#[tauri::command]
pub fn opencode_get(state: State<'_, AppState>) -> Result<OpenCodeSettings, AppError> {
    let cfg = load(&state)?;
    Ok(OpenCodeSettings {
        base_url: cfg.opencode.base_url,
        default_url: DEFAULT_URL.to_string(),
        has_password: cfg
            .opencode
            .password
            .as_deref()
            .is_some_and(|p| !p.is_empty()),
    })
}

/// Save the address (None or empty unlinks). `password`: None keeps the
/// saved one, an empty string clears it. A new address on another site
/// clears the saved password, which belongs to the old one.
#[tauri::command]
pub fn opencode_set(
    state: State<'_, AppState>,
    base_url: Option<String>,
    password: Option<String>,
) -> Result<OpenCodeSettings, AppError> {
    let base = base_url
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty());
    if let Some(b) = &base {
        crate::config::validate_base_url(b).map_err(AppError::invalid_input)?;
    }
    {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        let mut cfg = store.load().map_err(AppError::internal)?;
        let moved = match (&cfg.opencode.base_url, &base) {
            (Some(old), Some(new)) => match (url::Url::parse(old), url::Url::parse(new)) {
                (Ok(a), Ok(b)) => a.origin() != b.origin(),
                _ => true,
            },
            _ => false,
        };
        if moved || base.is_none() {
            cfg.opencode.password = None;
        }
        if let Some(p) = password {
            cfg.opencode.password = Some(p).filter(|p| !p.is_empty());
        }
        cfg.opencode.base_url = base;
        store.save(&cfg).map_err(AppError::internal)?;
    }
    opencode_get(state)
}

/// Ask the linked server whether it answers.
#[tauri::command]
pub async fn opencode_check(state: State<'_, AppState>) -> Result<OpenCodeHealth, AppError> {
    let client = client(&state)?;
    client.health().await
}

/// Start an OpenCode session with this task. Returns at once; OpenCode
/// works on it in the background.
#[tauri::command]
pub async fn opencode_send(
    state: State<'_, AppState>,
    title: Option<String>,
    task: String,
) -> Result<OpenCodeHandoff, AppError> {
    let client = client(&state)?;
    client.hand_off(title.as_deref().unwrap_or(""), &task).await
}
