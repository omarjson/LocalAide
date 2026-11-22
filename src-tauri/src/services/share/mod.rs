//! Share my assistant: a small web server on 127.0.0.1 and a Cloudflare
//! quick tunnel that gives it a public https link.
//!
//! Every request needs a token made on this computer. There are two, one
//! per permission: Viewer (read conversations) and Remote user (also chat,
//! answer approvals for its own requests, run Check my setup). Tool calls
//! from the link use the same approval flow and audit log as the app, and
//! always ask: the app's auto-approve setting never applies to the link.
//! Stopping sharing ends the tunnel and the server, so old links die.

pub mod auth;
pub mod qr;
mod server;
mod tunnel;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::app::error::AppError;
use crate::app::state::AppState;
use auth::{Role, Tokens};

/// A running share.
struct Running {
    stop: Arc<AtomicBool>,
    port: u16,
    url: Option<String>,
    tunnel: Option<tunnel::Tunnel>,
    started_at: u64,
    shared: Arc<Shared>,
    /// Both screens show this; the phone derives the same from its token.
    pair_code: String,
    /// The remote link as a QR, made once at start.
    qr_svg: String,
}

/// What the server threads share with the app.
pub(crate) struct Shared {
    pub tokens: Tokens,
    pub requests: AtomicU64,
    pub refused: AtomicU64,
    /// Chat jobs started through the link; only these can be approved or
    /// cancelled from it.
    pub jobs: Mutex<HashSet<String>>,
    pub last_seen: AtomicU64,
}

static SHARE: Mutex<Option<Running>> = Mutex::new(None);

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct ShareStatus {
    pub on: bool,
    pub url: Option<String>,
    pub started_at: Option<u64>,
    pub requests: u64,
    pub refused: u64,
    /// Seconds since 1970 of the last accepted request.
    pub last_seen: Option<u64>,
    pub cloudflared: crate::services::externals::ToolStatus,
}

/// Returned once, when sharing starts: the links with their tokens.
#[derive(Debug, Clone, Serialize)]
pub struct ShareStarted {
    pub status: ShareStatus,
    pub viewer_link: String,
    pub remote_link: String,
    /// What both screens show to confirm they are the same session:
    /// eight capital letters and digits, grouped XXXX-XXXX.
    pub pair_code: String,
    /// The remote link as a QR, ready to drop into the page.
    pub qr_svg: String,
}

/// What Settings → Share needs to pair a phone.
#[derive(Debug, Clone, Serialize)]
pub struct PairInfo {
    pub pair_code: String,
    pub qr_svg: String,
    pub on: bool,
}

/// The confirmation code for a link token: the first five bytes of its
/// SHA-256, Crockford-base32'd. No 0/O/1/I/L/U, so reading it aloud is
/// unambiguous. The phone derives the same code from the token it holds,
/// so matching codes mean "this is the same assistant".
pub fn pair_code_from(token: &str) -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let digest = Sha256::digest(token.as_bytes());
    let mut code = String::with_capacity(9);
    let mut acc: u64 = 0;
    let mut bits = 0u32;
    for &b in digest.iter().take(5) {
        acc = (acc << 8) | b as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((acc >> bits) & 0x1f) as usize;
            code.push(ALPHABET[index] as char);
        }
    }
    // 5 bytes = exactly 8 five-bit groups.
    let chars: Vec<char> = code.chars().collect();
    let (a, b) = (&chars[0..4], &chars[4..8]);
    format!(
        "{}-{}",
        a.iter().collect::<String>(),
        b.iter().collect::<String>()
    )
}

/// The pair info of the running share session, QR included.
pub fn pair_info() -> PairInfo {
    let (code, qr) = {
        let guard = SHARE.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(r) => (r.pair_code.clone(), r.qr_svg.clone()),
            None => (String::new(), String::new()),
        }
    };
    PairInfo {
        pair_code: code,
        qr_svg: qr,
        on: status().on,
    }
}

pub fn status() -> ShareStatus {
    let guard = SHARE.lock().unwrap_or_else(|e| e.into_inner());
    let cloudflared = crate::services::externals::status_of("cloudflared");
    match guard.as_ref() {
        Some(r) => ShareStatus {
            on: true,
            url: r.url.clone(),
            started_at: Some(r.started_at),
            requests: r.shared.requests.load(Ordering::Relaxed),
            refused: r.shared.refused.load(Ordering::Relaxed),
            last_seen: Some(r.shared.last_seen.load(Ordering::Relaxed)).filter(|t| *t > 0),
            cloudflared,
        },
        None => ShareStatus {
            on: false,
            url: None,
            started_at: None,
            requests: 0,
            refused: 0,
            last_seen: None,
            cloudflared,
        },
    }
}

/// Start sharing. `tunnel = false` serves on 127.0.0.1 only (for trying the
/// page on this computer).
pub fn start(state: AppState, with_tunnel: bool) -> Result<ShareStarted, AppError> {
    let mut guard = SHARE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return Err(AppError::invalid_input(
            "Sharing is already on. Stop it first to make new links.",
        ));
    }
    let cloudflared = if with_tunnel {
        Some(
            crate::services::externals::cloudflared_binary().ok_or_else(|| {
                AppError::new(
                    "CLOUDFLARED_MISSING",
                    "cloudflared is not on this computer yet. Download it in Settings → Share.",
                )
            })?,
        )
    } else {
        None
    };
    let tokens = Tokens::generate()?;
    let viewer = tokens.viewer_plain.clone();
    let remote = tokens.remote_plain.clone();
    let shared = Arc::new(Shared {
        tokens: tokens.forget_plain(),
        requests: AtomicU64::new(0),
        refused: AtomicU64::new(0),
        jobs: Mutex::new(HashSet::new()),
        last_seen: AtomicU64::new(0),
    });
    let stop = Arc::new(AtomicBool::new(false));
    let port = server::start(state.clone(), shared.clone(), stop.clone())?;
    let local = format!("http://127.0.0.1:{port}");
    let (url, tunnel) = match cloudflared {
        Some(bin) => match tunnel::Tunnel::open(&bin, port) {
            Ok(t) => (t.url.clone(), Some(t)),
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                return Err(e);
            }
        },
        None => (local, None),
    };
    state.tools.record_event(
        "share",
        "share",
        &url,
        "started",
        Some("Sharing started; links made for Viewer and Remote user"),
    );
    let remote_link = format!("{url}/#k={remote}");
    let pair_code = pair_code_from(&remote);
    let qr_svg = qr::svg(&remote_link)?;
    *guard = Some(Running {
        stop,
        port,
        url: Some(url.clone()),
        tunnel,
        started_at: now(),
        shared,
        pair_code: pair_code.clone(),
        qr_svg: qr_svg.clone(),
    });
    drop(guard);
    Ok(ShareStarted {
        status: status(),
        viewer_link: format!("{url}/#k={viewer}"),
        remote_link,
        pair_code,
        qr_svg,
    })
}

/// Stop sharing: the tunnel closes, the server stops, the tokens are gone.
pub fn stop(state: &AppState) -> ShareStatus {
    let running = SHARE.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(mut r) = running {
        r.stop.store(true, Ordering::Relaxed);
        if let Some(t) = r.tunnel.as_mut() {
            t.close();
        }
        // Answers still running for the link are cancelled.
        let jobs: Vec<String> = r
            .shared
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        for j in jobs {
            state.jobs.cancel_job(&j);
        }
        state.tools.record_event(
            "share",
            "share",
            r.url.as_deref().unwrap_or(""),
            "stopped",
            Some(&format!("port {}", r.port)),
        );
    }
    status()
}

pub(crate) fn role_name(role: Role) -> &'static str {
    match role {
        Role::Viewer => "viewer",
        Role::Remote => "remote",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_codes_are_grouped_capitals_and_digits() {
        let code = pair_code_from("abcdef0123456789abcdef0123456789abcdef0123456789ab");
        // XXXX-XXXX, nine characters with the dash.
        assert_eq!(code.len(), 9);
        assert_eq!(code.as_bytes()[4], b'-');
        for c in code.chars().filter(|c| *c != '-') {
            assert!(
                c.is_ascii_digit() || c.is_ascii_uppercase(),
                "{c} is not readable"
            );
            assert!(!"IOU".contains(c), "{c} reads like another letter");
        }
        // The same token always makes the same code.
        assert_eq!(
            pair_code_from("abcdef0123456789abcdef0123456789abcdef0123456789ab"),
            code
        );
        // Different tokens differ.
        assert_ne!(
            pair_code_from("ffffffff0123456789abcdef0123456789abcdef0123456789ab"),
            code
        );
    }
    use std::sync::Arc as A;

    fn state() -> AppState {
        use crate::app::paths::*;
        let model_manager = A::new(crate::model_manager::ModelManager::new().unwrap());
        let library = A::new(crate::services::library::LibraryService::open_in_memory().unwrap());
        let memory = A::new(crate::services::memory::MemoryService::new(
            library.db(),
            library.clone(),
            memory_dir(),
            app_dir().join("note_suggestions"),
        ));
        let model_service = A::new(
            crate::services::models::ModelService::new(
                models_dir(),
                catalog_cache_path(),
                custom_models_path(),
            )
            .unwrap(),
        );
        let jail =
            A::new(crate::services::tools::sandbox::WorkspaceJail::new(workspace_dir()).unwrap());
        let tools = A::new(crate::services::tools::ToolService::new(jail, library.db()));
        let skills = A::new(crate::services::skills::SkillsService::new());
        AppState::new(model_manager, model_service, library, memory, skills, tools)
    }

    /// Starts a real share server (no tunnel) and checks every permission
    /// rule over HTTP. Writes to the home folder, so it is opt-in:
    /// `HOME=/tmp/x cargo test -- --ignored real_share`.
    #[test]
    #[ignore]
    fn real_share_server_enforces_tokens_and_roles() {
        let st = state();
        let started = start(st.clone(), false).unwrap();
        let base = started.status.url.clone().unwrap();
        let token = |link: &str| link.split("#k=").nth(1).unwrap().to_string();
        let (viewer, remote) = (token(&started.viewer_link), token(&started.remote_link));
        let c = reqwest::blocking::Client::new();
        let get = |path: &str, tok: Option<&str>| {
            let mut r = c.get(format!("{base}{path}"));
            if let Some(t) = tok {
                r = r.bearer_auth(t);
            }
            r.send().unwrap()
        };
        let post = |path: &str, tok: &str, body: serde_json::Value| {
            c.post(format!("{base}{path}"))
                .bearer_auth(tok)
                .json(&body)
                .send()
                .unwrap()
        };

        let page = get("/", None);
        assert_eq!(page.status(), 200);
        assert!(
            page.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("default-src 'none'")
        );
        assert_eq!(get("/api/me", None).status(), 401, "no token, no data");
        assert_eq!(
            get("/api/me", Some("0".repeat(48).as_str())).status(),
            401,
            "a wrong token is refused"
        );
        let me: serde_json::Value = get("/api/me", Some(&viewer)).json().unwrap();
        assert_eq!(me["role"], "viewer");
        let me: serde_json::Value = get("/api/me", Some(&remote)).json().unwrap();
        assert_eq!(me["role"], "remote");

        assert_eq!(
            post(
                "/api/chat",
                &viewer,
                serde_json::json!({"message": "hi", "model": ""})
            )
            .status(),
            403,
            "viewers cannot chat"
        );
        assert_eq!(
            post(
                "/api/threads/t_share_test",
                &viewer,
                serde_json::json!({"title": "x"})
            )
            .status(),
            403
        );
        assert_eq!(
            post("/api/selftest", &viewer, serde_json::json!({})).status(),
            403
        );
        let saved = post(
            "/api/threads/t_share_test",
            &remote,
            serde_json::json!({"title": "From the link", "messages": [{"role": "user", "content": "hello"}]}),
        );
        assert_eq!(saved.status(), 200);
        let list: serde_json::Value = get("/api/threads", Some(&viewer)).json().unwrap();
        assert!(
            list.as_array()
                .unwrap()
                .iter()
                .any(|t| t["id"] == "t_share_test"),
            "viewers see the conversations"
        );
        let msgs: serde_json::Value = get("/api/threads/t_share_test", Some(&viewer))
            .json()
            .unwrap();
        assert_eq!(msgs[0]["content"], "hello");
        assert_eq!(
            post(
                "/api/decide",
                &remote,
                serde_json::json!({"approval_id": "job_from_the_app:c1", "decision": "allow"})
            )
            .status(),
            403,
            "the link cannot answer the app's own approvals"
        );
        assert_eq!(
            get("/api/threads/..%2F..%2Fetc", Some(&remote)).status(),
            404
        );

        let st2 = stop(&st);
        assert!(!st2.on);
        std::thread::sleep(std::time::Duration::from_millis(400));
        let dead = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap();
        assert!(
            dead.get(format!("{base}/api/me"))
                .bearer_auth(&remote)
                .send()
                .is_err(),
            "the link dies when sharing stops"
        );
        let _ = crate::commands::chat::chat_thread_delete_work("t_share_test".into());
    }
}
