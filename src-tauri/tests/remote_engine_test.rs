//! Loopback HTTP tests for the opt-in online engine and the mode router.
//! Nothing here touches the network beyond 127.0.0.1.

use app_lib::app::error::AppError;
use app_lib::services::inference::remote::{RemoteEngine, RemoteTarget};
use app_lib::services::inference::router::{EngineMode, RouterEngine};
use app_lib::services::inference::{InferenceEngine, InferenceToken, PromptMessage};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Server {
    base_url: String,
    requests: Arc<AtomicUsize>,
    done: thread::JoinHandle<()>,
}

/// Serves one canned HTTP response and counts how many times it was hit.
fn serve_once(body: &str, status_line: &str, extra: &str) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let body = body.to_string();
    let status = status_line.to_string();
    let extra = extra.to_string();
    let done = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one request");
        count.fetch_add(1, Ordering::SeqCst);
        let mut buf = [0u8; 8192];
        let _ = stream.read(&mut buf);
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });
    Server {
        base_url: format!("http://{addr}"),
        requests,
        done,
    }
}

fn target(url: &str, key: &str) -> RemoteTarget {
    RemoteTarget {
        label: "Test provider".into(),
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

fn sse(text: &str) -> String {
    format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n")
}

fn run(
    engine: &RemoteEngine,
    cancel: Arc<AtomicBool>,
) -> (
    Result<app_lib::services::inference::GenerationMetrics, AppError>,
    Vec<String>,
) {
    let tokens = Arc::new(Mutex::new(Vec::new()));
    let collected = tokens.clone();
    let result = engine.generate(
        &messages(),
        64,
        0.0,
        cancel,
        Box::new(move |t: InferenceToken| {
            if let Ok(mut guard) = tokens.lock() {
                guard.push(t.token);
            }
        }),
    );
    (result, collected.lock().unwrap().clone())
}

#[test]
fn a_clean_sse_stream_returns_every_token() {
    let body = format!("{}{}data: [DONE]\n\n", sse("Hello "), sse("world"));
    let server = serve_once(&body, "200 OK", "");
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let (result, tokens) = run(&engine, Arc::new(AtomicBool::new(false)));
    server.done.join().unwrap();
    let metrics = result.expect("clean stream succeeds");
    assert_eq!(tokens, vec!["Hello ", "world"]);
    assert!(!metrics.cancelled);
    assert_eq!(metrics.total_tokens, 2);
    assert_eq!(server.requests.load(Ordering::SeqCst), 1);
}

#[test]
fn a_stream_that_closes_without_done_is_an_error() {
    let server = serve_once(&sse("partial"), "200 OK", "");
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let (result, tokens) = run(&engine, Arc::new(AtomicBool::new(false)));
    server.done.join().unwrap();
    let err = result
        .expect_err("truncated stream is an error")
        .to_string();
    assert!(err.contains("closed the stream before completion"), "{err}");
    assert!(err.contains("received 1 tokens"), "{err}");
    assert_eq!(tokens, vec!["partial"]);
}

#[test]
fn a_mid_stream_error_event_is_not_shown_as_an_answer() {
    let body = format!(
        "{}data: {{\"error\":{{\"message\":\"quota\"}}}}\n\n",
        sse("partial")
    );
    let server = serve_once(&body, "200 OK", "");
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let (result, _tokens) = run(&engine, Arc::new(AtomicBool::new(false)));
    server.done.join().unwrap();
    let err = result.expect_err("streamed error fails").to_string();
    assert!(err.contains("streamed an error"), "{err}");
    assert!(err.contains("quota"), "{err}");
}

#[test]
fn a_401_explains_the_key_problem() {
    let server = serve_once("unauthorized", "401 Unauthorized", "");
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let (result, _) = run(&engine, Arc::new(AtomicBool::new(false)));
    server.done.join().unwrap();
    let err = result.expect_err("401 fails").to_string();
    assert!(err.contains("needs an API key"), "{err}");
}

#[test]
fn a_401_with_a_key_says_the_key_is_wrong() {
    let server = serve_once("unauthorized", "401 Unauthorized", "");
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, "sk-wrong"));
    let (result, _) = run(&engine, Arc::new(AtomicBool::new(false)));
    server.done.join().unwrap();
    let err = result.expect_err("401 fails").to_string();
    assert!(err.contains("looks wrong"), "{err}");
}

#[test]
fn redirects_are_not_followed() {
    let redirect_to = TcpListener::bind("127.0.0.1:0").unwrap();
    let redirect_url = format!("http://{}", redirect_to.local_addr().unwrap());
    redirect_to.set_nonblocking(true).unwrap();
    let server = serve_once("", "302 Found", &format!("Location: {redirect_url}\r\n"));
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let (result, _) = run(&engine, Arc::new(AtomicBool::new(false)));
    server.done.join().unwrap();
    assert!(result.is_err(), "302 must not be treated as an answer");
    // A non-followed redirect never opens a connection to the target.
    assert!(matches!(redirect_to.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
}

#[test]
fn cancelling_after_a_token_ends_cleanly() {
    let body = format!("{}{}data: [DONE]\n\n", sse("first"), sse("second"));
    let server = serve_once(&body, "200 OK", "");
    let engine = RemoteEngine::new();
    engine.configure(target(&server.base_url, ""));
    let flag = Arc::new(AtomicBool::new(false));
    let captured = flag.clone();
    let tokens = Arc::new(Mutex::new(Vec::new()));
    let collected = tokens.clone();
    let result = engine.generate(
        &messages(),
        64,
        0.0,
        flag,
        Box::new(move |t: InferenceToken| {
            if let Ok(mut guard) = tokens.lock() {
                guard.push(t.token);
            }
            captured.store(true, Ordering::SeqCst);
        }),
    );
    server.done.join().unwrap();
    let metrics = result.expect("cancellation is not an error");
    assert!(metrics.cancelled);
    assert_eq!(*collected.lock().unwrap(), vec!["first"]);
}

#[test]
fn router_mode_switching_is_explicit() {
    let router = RouterEngine::new();
    assert_eq!(router.mode(), EngineMode::Local);
    router.set_cloud_target(target("http://127.0.0.1:1/v1", "test-key"));
    assert_eq!(router.mode(), EngineMode::Cloud);
    assert_eq!(router.cloud_target().unwrap().model, "test-model");

    // Loading a local model is an explicit return to local inference.
    let absent = std::path::Path::new("/nonexistent/model.gguf");
    let _ = router.load(absent, 2048);
    assert_eq!(router.mode(), EngineMode::Local);
    assert!(router.cloud_target().is_none());

    // Cloud mode with no target is not "ready", whatever the UI toggle says.
    router.set_cloud_mode();
    assert_eq!(router.mode(), EngineMode::Cloud);
    assert!(!router.status().loaded);
    router.set_local();
    assert_eq!(router.mode(), EngineMode::Local);
    thread::sleep(Duration::from_millis(1));
}
