//! Settings → Share my assistant.

use tauri::State;

use crate::app::error::AppError;
use crate::app::events::DownloadProgressEvent;
use crate::app::state::AppState;
use crate::services::externals::{self, ToolStatus};
use crate::services::share::{self, ShareStarted, ShareStatus};

#[tauri::command]
pub fn share_status() -> ShareStatus {
    share::status()
}

/// Start sharing. `tunnel` false serves on this computer only, for trying
/// the page before the link goes public. `lan` opts into binding on all
/// interfaces; the pairing token is required either way.
#[tauri::command]
pub async fn share_start(
    state: State<'_, AppState>,
    tunnel: Option<bool>,
    lan: Option<bool>,
) -> Result<ShareStarted, AppError> {
    let owned: AppState = (*state).clone();
    let with_tunnel = tunnel.unwrap_or(true);
    let with_lan = lan.unwrap_or(false);
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(share::start(owned, with_tunnel, with_lan));
    });
    rx.await
        .unwrap_or_else(|_| Err(AppError::internal("Sharing stopped while it started.")))
}

#[tauri::command]
pub fn share_stop(state: State<'_, AppState>) -> ShareStatus {
    share::stop(&state)
}

/// The pairing card for the phone: the QR of the remote link and the
/// XXXX-XXXX code both screens confirm with.
#[tauri::command]
pub fn share_pair() -> share::PairInfo {
    share::pair_info()
}

/// Download cloudflared (pinned release, checksum checked).
#[tauri::command]
pub async fn share_cloudflared_fetch(
    state: State<'_, AppState>,
    channel: tauri::ipc::Channel<DownloadProgressEvent>,
) -> Result<ToolStatus, AppError> {
    let pin = externals::cloudflared_pin().ok_or_else(|| {
        AppError::invalid_input(
            "There is no cloudflared build for this computer. Install it from Cloudflare.",
        )
    })?;
    let (job_id, cancel) = state.jobs.create_job("download_cloudflared");
    let res = externals::fetch(pin, cancel, move |p| {
        let _ = channel.send(DownloadProgressEvent {
            model_id: "cloudflared".into(),
            downloaded_bytes: p.bytes_downloaded,
            total_bytes: p.bytes_total,
            percent: p.percent,
            speed_bps: p.speed_bps,
            status: "downloading".into(),
        });
    })
    .await;
    match res {
        Ok(_) => {
            state.jobs.complete_job(&job_id);
            Ok(externals::status_of("cloudflared"))
        }
        Err(e) => {
            state.jobs.fail_job(&job_id, &e.message);
            Err(e)
        }
    }
}
