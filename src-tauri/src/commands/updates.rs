//! Update check and the in-app updater (ADR-0005).
//!
//! Nothing here runs on its own. The user presses "Check for updates", and an
//! update is installed only after a second press. Installs go through
//! `tauri-plugin-updater`, which refuses any package whose signature does not
//! match the public key in `plugins.updater.pubkey`.
//!
//! Builds made without an update key (an empty `pubkey`) cannot install
//! updates. They fall back to the old behaviour: ask GitHub for the newest
//! release and link to its download page.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::inference::InferenceEngine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_updater::UpdaterExt;

const RELEASES_API: &str = "https://api.github.com/repos/omarjson/LocalAide/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/omarjson/LocalAide/releases/latest";
/// Release notes can be long; the Settings card shows a short preview.
const MAX_NOTES_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResponse {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: String,
    /// True when this build can download and install the update itself.
    pub can_install: bool,
    /// Release notes for the newer version, when the update feed has them.
    pub notes: Option<String>,
}

/// Progress of `update_install`, streamed over a Channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgressEvent {
    /// "downloading", "installing" or "restarting".
    pub stage: String,
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// True when this build carries an update public key.
fn updater_configured<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|v| v.get("pubkey"))
        .and_then(|v| v.as_str())
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
}

fn trim_notes(notes: Option<String>) -> Option<String> {
    let notes = notes?.trim().to_string();
    if notes.is_empty() {
        return None;
    }
    if notes.chars().count() <= MAX_NOTES_CHARS {
        return Some(notes);
    }
    let cut: String = notes.chars().take(MAX_NOTES_CHARS).collect();
    Some(format!("{cut}…"))
}

#[tauri::command]
pub async fn update_check<R: Runtime>(app: AppHandle<R>) -> Result<UpdateCheckResponse, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();

    if updater_configured(&app) {
        match app.updater() {
            Ok(updater) => match updater.check().await {
                Ok(Some(update)) => {
                    return Ok(UpdateCheckResponse {
                        current_version: current,
                        latest_version: Some(update.version.clone()),
                        update_available: true,
                        release_url: RELEASES_PAGE.to_string(),
                        can_install: true,
                        notes: trim_notes(update.body.clone()),
                    });
                }
                Ok(None) => {
                    return Ok(UpdateCheckResponse {
                        latest_version: Some(current.clone()),
                        current_version: current,
                        update_available: false,
                        release_url: RELEASES_PAGE.to_string(),
                        can_install: true,
                        notes: None,
                    });
                }
                // A release without an update feed (or a network hiccup):
                // fall back to the plain release check below.
                Err(e) => log::warn!("Update feed check failed, using the release check: {e}"),
            },
            Err(e) => log::warn!("Updater unavailable: {e}"),
        }
    }

    let (latest, notes) = latest_release().await;
    let update_available = latest
        .as_ref()
        .map(|latest| version_newer(&current, latest))
        .unwrap_or(false);

    Ok(UpdateCheckResponse {
        current_version: current,
        latest_version: latest,
        update_available,
        release_url: RELEASES_PAGE.to_string(),
        can_install: false,
        notes: if update_available { notes } else { None },
    })
}

/// The newest published release's version and notes. Offline or any HTTP
/// failure yields `(None, None)`.
async fn latest_release() -> (Option<String>, Option<String>) {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    else {
        return (None, None);
    };
    let response = client
        .get(RELEASES_API)
        .header("User-Agent", "LocalAide-update-check")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await;
    match response {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(body) => (
                body["tag_name"]
                    .as_str()
                    .map(|t| t.trim_start_matches('v').to_string()),
                trim_notes(body["body"].as_str().map(str::to_string)),
            ),
            Err(_) => (None, None),
        },
        _ => (None, None),
    }
}

/// Download, verify and install the newest version, then restart.
///
/// The engines are stopped before the installer runs, so no `llama-server`
/// process holds files the installer must replace. Your data under
/// `~/.aiassistant/` is not touched by an update.
#[tauri::command]
pub async fn update_install<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    channel: tauri::ipc::Channel<UpdateProgressEvent>,
) -> Result<(), AppError> {
    if !updater_configured(&app) {
        return Err(AppError::new(
            "UPDATE_UNAVAILABLE",
            "This copy of LocalAide cannot install updates by itself. Download the new version from the release page.",
        ));
    }
    let updater = app
        .updater()
        .map_err(|e| AppError::internal(format!("The updater could not start: {e}")))?;
    let update = updater
        .check()
        .await
        .map_err(|e| {
            AppError::new("UPDATE_CHECK_FAILED", "Could not reach the update server.")
                .with_detail(e.to_string())
                .retryable()
        })?
        .ok_or_else(|| AppError::new("UPDATE_NONE", "You already have the newest version."))?;

    let shared = state.inner().clone();
    let mut downloaded: u64 = 0;
    let progress = channel.clone();
    let finished = channel.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.send(UpdateProgressEvent {
                    stage: "downloading".to_string(),
                    downloaded,
                    total,
                });
            },
            move || {
                let _ = finished.send(UpdateProgressEvent {
                    stage: "installing".to_string(),
                    downloaded: 0,
                    total: None,
                });
                stop_engines(&shared);
            },
        )
        .await
        .map_err(|e| {
            AppError::new(
                "UPDATE_INSTALL_FAILED",
                "The update could not be installed. Your current version still works.",
            )
            .with_detail(e.to_string())
            .retryable()
        })?;

    let _ = channel.send(UpdateProgressEvent {
        stage: "restarting".to_string(),
        downloaded: 0,
        total: None,
    });
    log::info!("Update {} installed; restarting", update.version);
    app.restart();
}

fn stop_engines(state: &AppState) {
    let _ = state.inference_engine.unload();
    state.embedding_engine.stop();
    state.rerank_engine.stop();
    if let Err(e) = state.library.checkpoint() {
        log::warn!("Database checkpoint before update failed: {e}");
    }
}

/// True when `latest` is strictly newer than `current` (dotted numerics).
fn version_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(current), parse(latest));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if y > x {
            return true;
        }
        if y < x {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{trim_notes, version_newer};

    #[test]
    fn detects_newer_versions() {
        assert!(version_newer("0.1.0", "0.1.1"));
        assert!(version_newer("0.1.0", "1.0.0"));
        assert!(version_newer("0.1.9", "0.2.0"));
        assert!(!version_newer("0.1.0", "0.1.0"));
        assert!(!version_newer("0.2.0", "0.1.9"));
        assert!(!version_newer("0.1.0", "0.1.0-beta"));
    }

    #[test]
    fn notes_are_trimmed_and_capped() {
        assert_eq!(trim_notes(None), None);
        assert_eq!(trim_notes(Some("  \n ".into())), None);
        assert_eq!(trim_notes(Some(" hi ".into())).as_deref(), Some("hi"));
        let long = "é".repeat(5000);
        let cut = trim_notes(Some(long)).unwrap_or_default();
        assert_eq!(cut.chars().count(), 4001);
    }
}
