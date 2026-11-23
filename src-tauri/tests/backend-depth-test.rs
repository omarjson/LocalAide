//! Backend-depth tests for Stream C: rotation, doctor repair, goal loops,
//! config write caching and MCP PKCE helpers. Nothing here reaches the
//! network beyond 127.0.0.1; all cryptographic helpers are pure.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use app_lib::config::{AppConfig, ConfigStore, snapshot_is_dirty};
use app_lib::services::doctor::RepairPaths;
use app_lib::services::goal::{
    Goal, GoalStatus, StepDecision, StopReason, decide_next, drive, step_prompt,
};
use app_lib::services::inference::remote::{RemoteEngine, RemoteTarget, rotated_key};
use app_lib::services::inference::{InferenceToken, PromptMessage};

// ---- credential rotation --------------------------------------------------

#[test]
fn rotation_helper_only_retries_on_auth_status_with_a_new_key() {
    let supply = |status: u16| match status {
        401 => Some("second".to_string()),
        _ => None,
    };
    assert_eq!(
        rotated_key("first", 401, &supply).as_deref(),
        Some("second")
    );
    assert_eq!(rotated_key("first", 429, &supply), None);
    assert_eq!(rotated_key("first", 500, &supply), None);
    let same = |_status: u16| Some("first".to_string());
    assert_eq!(rotated_key("first", 401, &same), None, "no pointless retry");
    let blank = |_status: u16| Some("  ".to_string());
    assert_eq!(rotated_key("first", 401, &blank), None);
}

struct Server {
    base_url: String,
    requests: Arc<AtomicUsize>,
    auths: Arc<Mutex<Vec<String>>>,
    done: thread::JoinHandle<()>,
}

/// Serves a canned sequence of HTTP responses, one per connection. `Connection:
/// close` forces a fresh connection per request so a retry is observable.
fn serve_sequence(responses: Vec<(String, String)>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let auths = Arc::new(Mutex::new(Vec::new()));
    let counted = requests.clone();
    let seen = auths.clone();
    let done = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        for (status, body) in responses {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(pair) => break pair,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return;
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return,
                }
            };
            counted.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).into_owned();
            let header = request.lines().find_map(|line| {
                if line.to_ascii_lowercase().starts_with("authorization:") {
                    line.split_once(':')
                        .map(|(_, value)| value.trim().to_string())
                } else {
                    None
                }
            });
            if let Some(auth) = header {
                seen.lock().unwrap().push(auth);
            }
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    Server {
        base_url: format!("http://{addr}"),
        requests,
        auths,
        done,
    }
}

fn target(url: &str, key: &str) -> RemoteTarget {
    RemoteTarget {
        label: "Loopback".into(),
        base_url: url.into(),
        model: "test-model".into(),
        api_key: key.into(),
    }
}

fn messages() -> Vec<PromptMessage> {
    vec![PromptMessage {
        role: "user".into(),
        content: "hello".into(),
        images: vec![],
    }]
}

#[test]
fn a_401_rotates_the_key_and_retries_once() {
    let body = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
    let server = serve_sequence(vec![
        ("401 Unauthorized".to_string(), "unauthorized".to_string()),
        ("200 OK".to_string(), body.to_string()),
    ]);
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, "first"));
    engine.set_key_supplier(Arc::new(|status| {
        if status == 401 {
            Some("second".to_string())
        } else {
            None
        }
    }));
    let tokens = Arc::new(Mutex::new(Vec::new()));
    let collected = tokens.clone();
    let result = engine.generate(
        &messages(),
        32,
        0.0,
        Arc::new(AtomicBool::new(false)),
        Box::new(move |t: InferenceToken| {
            collected.lock().unwrap().push(t.token);
        }),
    );
    server.done.join().unwrap();
    assert!(result.is_ok(), "{:?}", result.err());
    assert_eq!(*tokens.lock().unwrap(), vec!["ok"]);
    assert_eq!(server.requests.load(Ordering::SeqCst), 2);
    let auths = server.auths.lock().unwrap();
    assert_eq!(auths.len(), 2, "{auths:?}");
    assert!(auths[0].contains("first"), "{auths:?}");
    assert!(auths[1].contains("second"), "{auths:?}");
}

#[test]
fn a_401_without_a_spare_key_keeps_the_original_hint_and_does_not_retry() {
    let server = serve_sequence(vec![(
        "401 Unauthorized".to_string(),
        "unauthorized".to_string(),
    )]);
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let result = engine.generate(
        &messages(),
        32,
        0.0,
        Arc::new(AtomicBool::new(false)),
        Box::new(|_t: InferenceToken| {}),
    );
    server.done.join().unwrap();
    let err = result.expect_err("401 is still an error").to_string();
    assert!(err.contains("needs an API key"), "{err}");
    assert_eq!(server.requests.load(Ordering::SeqCst), 1);
}

// ---- doctor repair --------------------------------------------------------

#[test]
fn doctor_repair_creates_safe_files_and_never_deletes_user_data() {
    let dir = std::env::temp_dir().join(format!("backend-depth-doctor-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let paths = RepairPaths {
        log_dir: dir.join("logs"),
        catalog_cache: dir.join("catalog-cache.json"),
        library_db: dir.join("index/library.db"),
    };
    let bundled = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../catalog/models.json"
    ))
    .expect("bundled catalog is present");

    let report = app_lib::services::doctor::repair_at(&paths, &bundled);
    assert!(report.remaining.is_empty(), "{:?}", report.remaining);
    assert!(paths.log_dir.is_dir());
    assert!(paths.catalog_cache.is_file());
    assert!(paths.library_db.is_file());
    assert!(
        report
            .fixed
            .iter()
            .any(|line| line.contains("log directory"))
    );
    assert!(
        report
            .fixed
            .iter()
            .any(|line| line.contains("catalog cache"))
    );
    assert!(
        report
            .fixed
            .iter()
            .any(|line| line.contains("library database"))
    );

    // An existing cache is user data from the app's point of view: repair
    // leaves it exactly as it found it.
    let sentinel = "{\"already\":\"here\"}";
    fs::write(&paths.catalog_cache, sentinel).unwrap();
    let second = app_lib::services::doctor::repair_at(&paths, &bundled);
    assert_eq!(fs::read_to_string(&paths.catalog_cache).unwrap(), sentinel);
    assert!(
        !second
            .fixed
            .iter()
            .any(|line| line.contains("catalog cache")),
        "{:?}",
        second.fixed
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---- goal continuation ----------------------------------------------------

#[test]
fn goal_loop_stops_at_budget_and_when_blocked() {
    let goal = Goal {
        id: "g1".into(),
        thread_id: "t1".into(),
        objective: "ship it".into(),
        budget: 2,
        status: GoalStatus::Active,
        created_at: 0,
    };
    let queued = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let driven = drive(&goal, &queued, false);
    assert_eq!(driven.applied, 2, "never more steps than budget");
    assert_eq!(driven.goal.budget, 0);
    assert_eq!(
        driven.decision,
        StepDecision::Stop(StopReason::BudgetExhausted)
    );

    let blocked = drive(&goal, &queued, true);
    assert_eq!(blocked.applied, 0);
    assert_eq!(blocked.goal.budget, 2, "a blocked turn spends no budget");
    assert_eq!(blocked.decision, StepDecision::Stop(StopReason::Blocked));

    let one = Goal {
        budget: 1,
        ..goal.clone()
    };
    match decide_next(&one, &[], false) {
        StepDecision::Continue {
            prompt,
            budget_left,
        } => {
            assert_eq!(budget_left, 1);
            assert!(prompt.contains("ship it"), "{prompt}");
        }
        other => panic!("expected a prompt, got {other:?}"),
    }
    let done = Goal {
        status: GoalStatus::Completed,
        ..goal
    };
    assert_eq!(
        decide_next(&done, &[], false),
        StepDecision::Stop(StopReason::Completed)
    );
    assert!(step_prompt(&one, &[]).contains("Steps left: 1"));
}

// ---- config write cache ---------------------------------------------------

#[test]
fn config_dirty_check_and_save_cache_behave_as_expected() {
    assert!(snapshot_is_dirty(None, "{}"));
    assert!(!snapshot_is_dirty(Some("{}"), "{}"));
    assert!(snapshot_is_dirty(Some("old"), "new"));

    let dir = std::env::temp_dir().join(format!("backend-depth-config-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let path = dir.join("config.json");
    let store = ConfigStore::at(path.clone());
    let mut cfg = AppConfig::default();
    store.save(&cfg).unwrap();
    let first = fs::read_to_string(&path).unwrap();
    store.save(&cfg).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), first);
    cfg.context_tokens = 4096;
    store.save(&cfg).unwrap();
    assert_ne!(
        fs::read_to_string(&path).unwrap(),
        first,
        "a change still writes"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---- MCP OAuth PKCE -------------------------------------------------------

#[test]
fn mcp_pkce_helpers_build_urls_verify_state_and_parse_tokens() {
    use app_lib::services::mcp::{
        McpOAuthConfig, authorize_url, new_pkce, parse_token_response, pkce_challenge,
        token_request, verify_state,
    };

    let cfg = McpOAuthConfig {
        server_id: "docs".into(),
        client_id: "client-1".into(),
        authorize_url: "https://auth.example.com/authorize".into(),
        token_url: "https://auth.example.com/token".into(),
        redirect_uri: "http://127.0.0.1:8765/callback".into(),
        scopes: vec!["mcp.read".into()],
        audience: None,
    };
    let pkce = new_pkce().unwrap();
    assert_eq!(pkce.challenge, pkce_challenge(&pkce.verifier));
    let url = authorize_url(&cfg, &pkce.state, &pkce.challenge).unwrap();
    assert!(url.contains("code_challenge_method=S256"), "{url}");
    assert!(url.contains(&pkce.state), "{url}");
    assert!(url.contains("client-1"), "{url}");
    assert!(verify_state(&pkce.state, &pkce.state).is_ok());
    assert!(verify_state(&pkce.state, "attacker").is_err());

    let request = token_request(&cfg, "the-code", &pkce.verifier).unwrap();
    assert_eq!(request.url, cfg.token_url);
    assert!(request.body.contains("grant_type=authorization_code"));
    assert!(request.body.contains("code=the-code"));
    let token = parse_token_response(r#"{"access_token":"t","token_type":"Bearer"}"#).unwrap();
    assert_eq!(token.access_token, "t");
    assert!(parse_token_response(r#"{"error":"access_denied"}"#).is_err());
}
