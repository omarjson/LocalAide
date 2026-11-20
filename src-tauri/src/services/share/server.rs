//! The web server behind a shared link. Listens on 127.0.0.1 only; the
//! tunnel is the only way in from outside. Every /api request needs a token.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use super::Shared;
use super::auth::{Role, bearer};
use crate::app::error::AppError;
use crate::app::events::ChatStreamEvent;
use crate::app::state::AppState;
use crate::domain::chat::{ChatMessage, ChatStreamRequest, ToolDecisionRequest};
use crate::services::agent::Sink;
use crate::services::agent::approval::Decision;

const PAGE: &str = include_str!("page.html");
const SCRIPT: &str = include_str!("page.js");
const MAX_BODY: usize = 2 * 1024 * 1024;

/// Refused attempts in the last minute; past the limit, every request waits.
static FAILURES: Mutex<Vec<Instant>> = Mutex::new(Vec::new());
const MAX_FAILURES_PER_MINUTE: usize = 30;

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header")
}

fn secure(mut r: Response<Box<dyn Read + Send>>, kind: &str) -> Response<Box<dyn Read + Send>> {
    for (n, v) in [
        ("Content-Type", kind),
        (
            "Content-Security-Policy",
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
        ("Referrer-Policy", "no-referrer"),
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        (
            "Permissions-Policy",
            "camera=(), microphone=(), geolocation=()",
        ),
    ] {
        r.add_header(header(n, v));
    }
    r
}

fn boxed(body: Vec<u8>, status: u16) -> Response<Box<dyn Read + Send>> {
    let len = body.len();
    Response::new(
        StatusCode(status),
        vec![],
        Box::new(std::io::Cursor::new(body)) as Box<dyn Read + Send>,
        Some(len),
        None,
    )
}

fn json_response(v: &Value, status: u16) -> Response<Box<dyn Read + Send>> {
    secure(
        boxed(v.to_string().into_bytes(), status),
        "application/json; charset=utf-8",
    )
}

fn error(status: u16, message: &str) -> Response<Box<dyn Read + Send>> {
    json_response(&json!({"error": message}), status)
}

/// Streams lines from a channel as a chunked response body.
struct LineReader {
    rx: Receiver<String>,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for LineReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.buf.len() {
            match self.rx.recv() {
                Ok(line) => {
                    self.buf = line.into_bytes();
                    self.buf.push(b'\n');
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = out.len().min(self.buf.len() - self.pos);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

fn stream_response(rx: Receiver<String>) -> Response<Box<dyn Read + Send>> {
    let reader = LineReader {
        rx,
        buf: Vec::new(),
        pos: 0,
    };
    secure(
        Response::new(
            StatusCode(200),
            vec![],
            Box::new(reader) as Box<dyn Read + Send>,
            None,
            None,
        ),
        "application/x-ndjson; charset=utf-8",
    )
}

struct ChannelSink(Mutex<Sender<String>>);

impl Sink for ChannelSink {
    fn emit(&self, event: ChatStreamEvent) {
        if let Ok(line) = serde_json::to_string(&event)
            && let Ok(tx) = self.0.lock()
        {
            let _ = tx.send(line);
        }
    }
}

/// Start listening on a free local port. Returns the port.
pub fn start(state: AppState, shared: Arc<Shared>, stop: Arc<AtomicBool>) -> Result<u16, AppError> {
    let server = Server::http("127.0.0.1:0")
        .map_err(|e| AppError::internal(format!("The share server did not start: {e}")))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .ok_or_else(|| AppError::internal("The share server has no port."))?;
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match server.recv_timeout(Duration::from_millis(200)) {
                Ok(Some(req)) => {
                    let state = state.clone();
                    let shared = shared.clone();
                    let stop = stop.clone();
                    std::thread::spawn(move || handle(req, &state, &shared, &stop));
                }
                Ok(None) => {}
                Err(_) => break,
            }
        }
    });
    Ok(port)
}

fn too_many_failures() -> bool {
    let mut f = FAILURES.lock().unwrap_or_else(|e| e.into_inner());
    f.retain(|t| t.elapsed() < Duration::from_secs(60));
    f.len() >= MAX_FAILURES_PER_MINUTE
}

fn note_failure() {
    FAILURES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Instant::now());
}

fn read_body(req: &mut Request) -> Result<Value, &'static str> {
    if req.body_length().is_some_and(|n| n > MAX_BODY) {
        return Err("The request is too large.");
    }
    let mut buf = Vec::new();
    req.as_reader()
        .take(MAX_BODY as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|_| "The request could not be read.")?;
    if buf.len() > MAX_BODY {
        return Err("The request is too large.");
    }
    if buf.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&buf).map_err(|_| "The request is not valid JSON.")
}

fn handle(mut req: Request, state: &AppState, shared: &Shared, stop: &AtomicBool) {
    let path = req.url().split('?').next().unwrap_or("/").to_string();
    let method = req.method().clone();

    // The page itself holds no data; everything under /api needs a token.
    if method == Method::Get && (path == "/" || path == "/index.html") {
        let _ = req.respond(secure(
            boxed(PAGE.as_bytes().to_vec(), 200),
            "text/html; charset=utf-8",
        ));
        return;
    }
    if method == Method::Get && path == "/app.js" {
        let _ = req.respond(secure(
            boxed(SCRIPT.as_bytes().to_vec(), 200),
            "text/javascript; charset=utf-8",
        ));
        return;
    }
    if !path.starts_with("/api/") {
        let _ = req.respond(error(404, "Not found."));
        return;
    }
    if too_many_failures() {
        std::thread::sleep(Duration::from_secs(1));
        let _ = req.respond(error(
            429,
            "Too many refused requests. Try again in a minute.",
        ));
        return;
    }
    let auth = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .map(|h| h.value.as_str().to_string());
    let Some(role) = bearer(auth.as_deref()).and_then(|t| shared.tokens.check(t)) else {
        shared.refused.fetch_add(1, Ordering::Relaxed);
        note_failure();
        let _ = req.respond(error(
            401,
            "This link needs its token. Open the full link you were given.",
        ));
        return;
    };
    shared.requests.fetch_add(1, Ordering::Relaxed);
    shared.last_seen.store(super::now(), Ordering::Relaxed);
    let remote = role == Role::Remote;
    let needs_remote = || {
        error(
            403,
            "This link can only view. Ask for a Remote user link to chat.",
        )
    };

    let response = match (method, path.as_str()) {
        (Method::Get, "/api/me") => json_response(
            &json!({
                "role": super::role_name(role),
                "version": env!("CARGO_PKG_VERSION"),
                "model": current_model(state),
            }),
            200,
        ),
        (Method::Get, "/api/threads") => match crate::commands::chat::chat_thread_list_work() {
            Ok(list) => json_response(&json!(list), 200),
            Err(e) => error(500, &e.message),
        },
        (Method::Get, p) if p.starts_with("/api/threads/") => {
            let id = p.trim_start_matches("/api/threads/").to_string();
            match crate::commands::chat::chat_thread_load_work(id) {
                Ok(messages) => json_response(&json!(messages), 200),
                Err(e) => error(404, &e.message),
            }
        }
        (Method::Post, p) if p.starts_with("/api/threads/") => {
            if !remote {
                needs_remote()
            } else {
                let id = p.trim_start_matches("/api/threads/").to_string();
                match read_body(&mut req) {
                    Err(m) => error(400, m),
                    Ok(body) => save_thread(id, body),
                }
            }
        }
        (Method::Post, "/api/chat") => {
            if !remote {
                needs_remote()
            } else {
                match read_body(&mut req) {
                    Err(m) => error(400, m),
                    Ok(body) => match chat(state, shared, body) {
                        Ok(rx) => stream_response(rx),
                        Err(e) => error(400, &e.message),
                    },
                }
            }
        }
        (Method::Post, "/api/decide") => {
            if !remote {
                needs_remote()
            } else {
                match read_body(&mut req) {
                    Err(m) => error(400, m),
                    Ok(body) => decide(state, shared, body),
                }
            }
        }
        (Method::Post, "/api/cancel") => {
            if !remote {
                needs_remote()
            } else {
                match read_body(&mut req) {
                    Err(m) => error(400, m),
                    Ok(body) => {
                        let job = body
                            .get("job_id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let ours = shared
                            .jobs
                            .lock()
                            .map(|j| j.contains(&job))
                            .unwrap_or(false);
                        if ours {
                            state.jobs.cancel_job(&job);
                            json_response(&json!({"ok": true}), 200)
                        } else {
                            error(403, "That answer was not started from this link.")
                        }
                    }
                }
            }
        }
        (Method::Post, "/api/selftest") => {
            if !remote {
                needs_remote()
            } else {
                selftest(state)
            }
        }
        _ => error(404, "Not found."),
    };
    if !stop.load(Ordering::Relaxed) {
        let _ = req.respond(response);
    }
}

fn current_model(state: &AppState) -> Value {
    let id = state.model_manager.active_model_id();
    let name = id
        .as_deref()
        .and_then(|i| state.model_service.get_model_entry(i))
        .map(|e| e.name);
    let online = matches!(
        state.inference_engine.current_target(),
        crate::services::inference::router::ChatTarget::Remote(_)
    );
    json!({"name": name, "online": online, "loaded": state.inference_engine.local().endpoint().is_some()})
}

#[derive(Deserialize)]
struct SaveBody {
    title: String,
    #[serde(default)]
    messages: Vec<ChatMessage>,
}

fn save_thread(id: String, body: Value) -> Response<Box<dyn Read + Send>> {
    let Ok(b) = serde_json::from_value::<SaveBody>(body) else {
        return error(400, "The chat could not be read.");
    };
    match crate::commands::chat::chat_thread_save_work(id, b.title, None, b.messages) {
        Ok(()) => json_response(&json!({"ok": true}), 200),
        Err(e) => error(400, &e.message),
    }
}

fn chat(state: &AppState, shared: &Shared, body: Value) -> Result<Receiver<String>, AppError> {
    let mut req: ChatStreamRequest = serde_json::from_value(body)
        .map_err(|e| AppError::invalid_input(format!("The message could not be read: {e}")))?;
    // The link chats with the app's current model; it cannot pick another.
    req.target = None;
    let (run, model_label, job_id, cancel) =
        crate::commands::chat::prepare_run(state, req, "share")?;
    shared
        .jobs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(job_id.clone());
    let (tx, rx) = channel::<String>();
    let _ = tx.send(json!({"type": "job", "payload": {"job_id": job_id}}).to_string());
    let _ = tx.send(
        serde_json::to_string(&ChatStreamEvent::Status {
            label: format!("Answering with {model_label}"),
        })
        .unwrap_or_default(),
    );
    let sink = ChannelSink(Mutex::new(tx.clone()));
    let owned = state.clone();
    let jobs = state.jobs.clone();
    std::thread::spawn(move || {
        let result = crate::services::agent::run(&owned, run, &cancel, &sink);
        let last = match result {
            Ok(done) => {
                jobs.complete_job(&job_id);
                ChatStreamEvent::Done {
                    finish_reason: if done.cancelled {
                        "cancelled".into()
                    } else {
                        done.finish_reason
                    },
                    total_tokens: done.tokens,
                }
            }
            Err(e) => {
                jobs.fail_job(&job_id, &e.message);
                ChatStreamEvent::Error { error: e.message }
            }
        };
        sink.emit(last);
    });
    Ok(rx)
}

fn decide(state: &AppState, shared: &Shared, body: Value) -> Response<Box<dyn Read + Send>> {
    let Ok(d) = serde_json::from_value::<ToolDecisionRequest>(body) else {
        return error(400, "Send approval_id and decision.");
    };
    let job = d.approval_id.split(':').next().unwrap_or("");
    let ours = shared.jobs.lock().map(|j| j.contains(job)).unwrap_or(false);
    if !ours {
        return error(403, "Only questions from this link can be answered here.");
    }
    let Some(decision) = Decision::parse(&d.decision) else {
        return error(400, "Answer allow, allow_chat or deny.");
    };
    json_response(
        &json!({"delivered": state.agent.approvals.decide(&d.approval_id, decision)}),
        200,
    )
}

fn selftest(state: &AppState) -> Response<Box<dyn Read + Send>> {
    let (tx, rx) = channel::<String>();
    let owned = state.clone();
    std::thread::spawn(move || {
        let steps = Mutex::new(tx.clone());
        let emit: Arc<dyn Fn(crate::commands::selftest::SelfTestStep) + Send + Sync> =
            Arc::new(move |s| {
                if let (Ok(line), Ok(tx)) = (
                    serde_json::to_string(&json!({"type": "step", "payload": s})),
                    steps.lock(),
                ) {
                    let _ = tx.send(line);
                }
            });
        let report =
            tauri::async_runtime::block_on(crate::commands::selftest::run_selftest(&owned, emit));
        let _ = tx.send(json!({"type": "report", "payload": report}).to_string());
    });
    stream_response(rx)
}
