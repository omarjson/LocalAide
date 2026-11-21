//! The llama.cpp model engine. LocalAide does not ship llama-server: the
//! first time a model loads, it downloads the official llama.cpp build for
//! this computer's processor (pinned by checksum, see `externals`) and
//! keeps it in `~/.aiassistant/tools/`. The window hears about the
//! download through the `engine-download` event.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::app::error::AppError;
use crate::services::externals;

static APP: OnceLock<AppHandle> = OnceLock::new();
static ONE_AT_A_TIME: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

/// Called once at startup so downloads can report progress.
pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineDownloadEvent {
    /// "downloading", "ready" or "failed".
    pub state: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub message: Option<String>,
}

fn emit(event: EngineDownloadEvent) {
    if let Some(app) = APP.get() {
        let _ = app.emit("engine-download", event);
    }
}

/// What the Settings and self-test screens show about the engine.
#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub installed: bool,
    pub build: &'static str,
    /// "x64" or "ARM64": the processor the engine build is for.
    pub cpu: &'static str,
    pub download_bytes: Option<u64>,
    /// The engine came from LOCALAIDE_LLAMA_SERVER or sits next to the app.
    pub custom: bool,
    pub path: Option<String>,
}

pub fn info() -> EngineInfo {
    let found = crate::services::inference::sidecar::locate_binary().ok();
    let fetched = externals::llama_binary();
    let pin = externals::llama_pin();
    EngineInfo {
        installed: found.is_some(),
        build: externals::LLAMA_BUILD,
        cpu: externals::native_arch().label(),
        download_bytes: pin.map(|p| p.size),
        custom: found.is_some() && found != fetched,
        path: found.map(|p| p.to_string_lossy().into_owned()),
    }
}

/// The llama-server program, downloading it first when it is missing.
/// Several callers at once share one download.
pub async fn ensure() -> Result<PathBuf, AppError> {
    if let Ok(path) = crate::services::inference::sidecar::locate_binary() {
        return Ok(path);
    }
    let _turn = ONE_AT_A_TIME
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    if let Ok(path) = crate::services::inference::sidecar::locate_binary() {
        return Ok(path);
    }
    let pin = externals::llama_pin().ok_or_else(|| {
        AppError::invalid_input(format!(
            "There is no model engine build for this computer ({} {}).",
            std::env::consts::OS,
            externals::native_arch().label()
        ))
    })?;
    log::info!(
        "Downloading the model engine llama.cpp {} for {}",
        pin.version,
        externals::native_arch().label()
    );
    emit(EngineDownloadEvent {
        state: "downloading",
        downloaded_bytes: 0,
        total_bytes: pin.size,
        percent: 0.0,
        message: None,
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let last = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let res = externals::fetch(pin, cancel, move |p| {
        // A few updates a second are plenty.
        let step = p.bytes_downloaded / 500_000;
        if step != last.swap(step, Ordering::Relaxed) || p.bytes_downloaded == p.bytes_total {
            emit(EngineDownloadEvent {
                state: "downloading",
                downloaded_bytes: p.bytes_downloaded,
                total_bytes: p.bytes_total,
                percent: p.percent,
                message: None,
            });
        }
    })
    .await;
    match &res {
        Ok(_) => emit(EngineDownloadEvent {
            state: "ready",
            downloaded_bytes: pin.size,
            total_bytes: pin.size,
            percent: 100.0,
            message: None,
        }),
        Err(e) => emit(EngineDownloadEvent {
            state: "failed",
            downloaded_bytes: 0,
            total_bytes: pin.size,
            percent: 0.0,
            message: Some(e.message.clone()),
        }),
    }
    res.map_err(|e| {
        AppError::model_load_failed(
            "engine",
            &format!(
                "The model engine could not be downloaded from github.com: {}",
                e.message
            ),
        )
    })
}
