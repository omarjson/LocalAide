//! Live transcription over WebSocket: the window sends audio while the user
//! speaks, and interim and final text come back as it is recognised.
//! Providers: Gemini Live, Deepgram and Soniox. One online-log row per
//! session.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::Serialize;
use serde_json::{Value, json};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::app::error::AppError;
use crate::services::cloud_audit;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveEvent {
    /// Ready for audio.
    Open,
    /// Text so far, including words that may still change.
    Interim {
        text: String,
    },
    /// Confirmed text so far.
    Final {
        text: String,
    },
    /// The session ended; `text` is the whole transcript.
    Done {
        text: String,
    },
    Error {
        message: String,
    },
}

enum Cmd {
    Audio(Vec<u8>),
    Stop,
}

#[derive(Default)]
pub struct LiveHub {
    sessions: Mutex<HashMap<String, Sender<Cmd>>>,
}

type Socket = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn set_timeout(ws: &mut Socket, d: Duration) {
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(Some(d));
        }
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_ref().set_read_timeout(Some(d));
        }
        _ => {}
    }
}

/// What a provider needs: where to connect, the headers, the first message.
struct Plan {
    url: String,
    headers: Vec<(&'static str, String)>,
    hello: Option<String>,
    label: &'static str,
}

fn plan(provider: &str, model: &str, key: &str) -> Result<Plan, AppError> {
    Ok(match provider {
        "deepgram" => Plan {
            url: format!(
                "wss://api.deepgram.com/v1/listen?model={model}&encoding=linear16&sample_rate=16000&channels=1&interim_results=true&smart_format=true&language=multi"
            ),
            headers: vec![("Authorization", format!("Token {key}"))],
            hello: None,
            label: "Deepgram",
        },
        "soniox" => Plan {
            url: "wss://stt-rt.soniox.com/transcribe-websocket".into(),
            headers: vec![("Authorization", format!("Bearer {key}"))],
            hello: Some(
                json!({
                    "api_key": key,
                    "model": model,
                    "audio_format": "pcm_s16le",
                    "sample_rate": 16000,
                    "num_channels": 1,
                    "language_hints": ["en", "ar"],
                })
                .to_string(),
            ),
            label: "Soniox",
        },
        "gemini" => Plan {
            url: "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent".into(),
            headers: vec![("x-goog-api-key", key.to_string())],
            hello: Some(
                json!({"setup": {
                    "model": format!("models/{model}"),
                    "generationConfig": {"responseModalities": ["TEXT"]},
                    "inputAudioTranscription": {},
                }})
                .to_string(),
            ),
            label: "Google Gemini",
        },
        _ => return Err(AppError::invalid_input("That provider has no live transcription.")),
    })
}

/// Text from one provider message: (new final text, interim text).
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub final_text: String,
    pub interim: Option<String>,
    pub ready: bool,
    pub finished: bool,
    pub error: Option<String>,
}

pub fn parse(provider: &str, v: &Value) -> Parsed {
    let mut p = Parsed::default();
    match provider {
        "deepgram" => {
            if v.get("type").and_then(Value::as_str) == Some("Results") {
                let t = v
                    .pointer("/channel/alternatives/0/transcript")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if v.get("is_final").and_then(Value::as_bool).unwrap_or(false) {
                    p.final_text = t;
                } else {
                    p.interim = Some(t);
                }
            } else if v.get("type").and_then(Value::as_str) == Some("Metadata") {
                p.finished = true;
            } else if let Some(err) = v
                .get("err_msg")
                .or_else(|| v.get("description"))
                .and_then(Value::as_str)
            {
                p.error = Some(err.to_string());
            }
        }
        "soniox" => {
            if let Some(msg) = v.get("error_message").and_then(Value::as_str) {
                p.error = Some(msg.to_string());
            }
            let mut interim = String::new();
            for tok in v
                .get("tokens")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let t = tok.get("text").and_then(Value::as_str).unwrap_or("");
                if t.starts_with('<') && t.ends_with('>') {
                    continue; // markers such as <end>
                }
                if tok
                    .get("is_final")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    p.final_text.push_str(t);
                } else {
                    interim.push_str(t);
                }
            }
            if !interim.is_empty() {
                p.interim = Some(interim);
            }
            p.finished = v.get("finished").and_then(Value::as_bool).unwrap_or(false);
        }
        "gemini" => {
            if v.get("setupComplete").is_some() {
                p.ready = true;
            }
            if let Some(sc) = v.get("serverContent") {
                if let Some(t) = sc
                    .pointer("/inputTranscription/text")
                    .and_then(Value::as_str)
                {
                    p.final_text = t.to_string();
                }
                if let Some(t) = sc
                    .pointer("/interimInputTranscription/text")
                    .and_then(Value::as_str)
                {
                    p.interim = Some(t.to_string());
                }
            }
            if let Some(e) = v.pointer("/error/message").and_then(Value::as_str) {
                p.error = Some(e.to_string());
            }
        }
        _ => {}
    }
    p
}

fn join(acc: &str, piece: &str) -> String {
    let piece = piece.trim();
    if acc.is_empty() {
        piece.to_string()
    } else if piece.is_empty() {
        acc.to_string()
    } else {
        format!("{} {}", acc.trim_end(), piece)
    }
}

impl LiveHub {
    pub fn start(
        &self,
        provider: &str,
        model: &str,
        key: &str,
        emit: Box<dyn Fn(LiveEvent) + Send>,
    ) -> Result<String, AppError> {
        if key.trim().is_empty() {
            return Err(AppError::invalid_input(
                "Add your key for this provider in Settings → Voice first.",
            ));
        }
        let plan = plan(provider, model, key.trim())?;
        let mut id = [0u8; 8];
        let _ = getrandom::fill(&mut id);
        let id = hex::encode(id);
        let (tx, rx) = channel();
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.clone(), tx);
        let provider = provider.to_string();
        let model = model.to_string();
        let key = key.trim().to_string();
        std::thread::spawn(move || {
            let result = run(&provider, &model, &key, plan, rx, &*emit);
            if let Err(e) = result {
                emit(LiveEvent::Error { message: e.message });
            }
        });
        Ok(id)
    }

    pub fn push(&self, id: &str, pcm: Vec<u8>) -> bool {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_some_and(|tx| tx.send(Cmd::Audio(pcm)).is_ok())
    }

    pub fn stop(&self, id: &str) -> bool {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id)
            .is_some_and(|tx| tx.send(Cmd::Stop).is_ok())
    }
}

fn send_audio(provider: &str, ws: &mut Socket, pcm: Vec<u8>) -> Result<(), tungstenite::Error> {
    if provider == "gemini" {
        let msg = json!({"realtimeInput": {"audio": {
            "data": base64::engine::general_purpose::STANDARD.encode(&pcm),
            "mimeType": "audio/pcm;rate=16000",
        }}});
        ws.send(Message::Text(msg.to_string().into()))
    } else {
        ws.send(Message::Binary(pcm.into()))
    }
}

fn send_end(provider: &str, ws: &mut Socket) -> Result<(), tungstenite::Error> {
    match provider {
        "deepgram" => ws.send(Message::Text(r#"{"type":"CloseStream"}"#.into())),
        "soniox" => ws.send(Message::Binary(Vec::new().into())),
        _ => ws.send(Message::Text(
            json!({"realtimeInput": {"audioStreamEnd": true}})
                .to_string()
                .into(),
        )),
    }
}

fn run(
    provider: &str,
    model: &str,
    key: &str,
    plan: Plan,
    rx: Receiver<Cmd>,
    emit: &dyn Fn(LiveEvent),
) -> Result<(), AppError> {
    let audit = cloud_audit::start(
        plan.label,
        &plan.url,
        model,
        "voice (live)",
        "Live microphone audio",
        0,
        Some(key),
    );
    let mut request = plan
        .url
        .as_str()
        .into_client_request()
        .map_err(|e| AppError::internal(e.to_string()))?;
    for (name, value) in &plan.headers {
        let v = HeaderValue::from_str(value)
            .map_err(|_| AppError::invalid_input("The key has characters a header cannot hold."))?;
        request.headers_mut().insert(*name, v);
    }
    let (mut ws, _) = match tungstenite::connect(request) {
        Ok(c) => c,
        Err(e) => {
            let msg = match &e {
                tungstenite::Error::Http(r) => format!(
                    "{} refused the connection ({}). Check the key.",
                    plan.label,
                    r.status()
                ),
                _ => format!("Could not reach {}: {e}", plan.label),
            };
            audit.finish(false, &msg);
            return Err(AppError::new("VOICE_ONLINE_FAILED", msg));
        }
    };
    set_timeout(&mut ws, Duration::from_millis(40));
    if let Some(hello) = plan.hello {
        ws.send(Message::Text(hello.into()))
            .map_err(|e| AppError::internal(e.to_string()))?;
    }
    let mut ready = provider != "gemini";
    if ready {
        emit(LiveEvent::Open);
    }
    let mut waiting: Vec<Vec<u8>> = Vec::new();
    let mut finals = String::new();
    let mut bytes = 0usize;
    let mut stop_at: Option<Instant> = None;
    let started = Instant::now();
    let mut failure: Option<String> = None;
    let mut last_message = Instant::now();
    let mut finished = false;

    loop {
        // Audio and Stop from the window.
        loop {
            match rx.try_recv() {
                Ok(Cmd::Audio(pcm)) => {
                    bytes += pcm.len();
                    if ready {
                        let _ = send_audio(provider, &mut ws, pcm);
                    } else {
                        waiting.push(pcm);
                    }
                }
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => {
                    if stop_at.is_none() {
                        for pcm in waiting.drain(..) {
                            let _ = send_audio(provider, &mut ws, pcm);
                        }
                        let _ = send_end(provider, &mut ws);
                        stop_at = Some(Instant::now());
                    }
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        // After Stop: end when the provider says it is done, after 1.5 s
        // without news, or after 6 s at most.
        if let Some(t) = stop_at
            && (finished
                || t.elapsed() > Duration::from_secs(6)
                || (t.elapsed() > Duration::from_millis(1500)
                    && last_message.elapsed() > Duration::from_millis(1500)))
        {
            break;
        }
        if started.elapsed() > Duration::from_secs(15 * 60) {
            break;
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                last_message = Instant::now();
                finished |= handle(
                    provider,
                    t.as_str(),
                    &mut ws,
                    &mut ready,
                    &mut waiting,
                    &mut finals,
                    &mut failure,
                    emit,
                );
            }
            Ok(Message::Binary(b)) => {
                last_message = Instant::now();
                if let Ok(t) = std::str::from_utf8(&b) {
                    finished |= handle(
                        provider,
                        t,
                        &mut ws,
                        &mut ready,
                        &mut waiting,
                        &mut finals,
                        &mut failure,
                        emit,
                    );
                }
            }
            Ok(Message::Close(frame)) => {
                if finals.is_empty()
                    && stop_at.is_none()
                    && let Some(f) = frame
                    && !f.reason.is_empty()
                {
                    failure = Some(format!(
                        "{} closed the connection: {}",
                        plan.label, f.reason
                    ));
                }
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => break,
            Err(e) => {
                failure = Some(format!("The live connection broke: {e}"));
                break;
            }
        }
        if failure.is_some() {
            break;
        }
    }
    let _ = ws.close(None);
    let secs = crate::services::voice::pcm_seconds(bytes);
    match failure {
        Some(msg) => {
            audit.finish(false, &msg);
            Err(AppError::new("VOICE_ONLINE_FAILED", msg))
        }
        None => {
            audit.finish(true, &format!("{secs:.1} s of audio"));
            emit(LiveEvent::Done {
                text: finals.trim().to_string(),
            });
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle(
    provider: &str,
    text: &str,
    ws: &mut Socket,
    ready: &mut bool,
    waiting: &mut Vec<Vec<u8>>,
    finals: &mut String,
    failure: &mut Option<String>,
    emit: &dyn Fn(LiveEvent),
) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let p = parse(provider, &v);
    if let Some(e) = p.error {
        *failure = Some(e);
        return false;
    }
    if p.ready && !*ready {
        *ready = true;
        emit(LiveEvent::Open);
        for pcm in waiting.drain(..) {
            let _ = send_audio(provider, ws, pcm);
        }
    }
    if !p.final_text.trim().is_empty() {
        *finals = if provider == "soniox" {
            format!("{finals}{}", p.final_text)
        } else {
            join(finals, &p.final_text)
        };
        emit(LiveEvent::Final {
            text: finals.trim().to_string(),
        });
    }
    if let Some(i) = p.interim {
        emit(LiveEvent::Interim {
            text: join(finals, &i),
        });
    }
    p.finished
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_provider() {
        let d = parse(
            "deepgram",
            &json!({"type": "Results", "is_final": true, "channel": {"alternatives": [{"transcript": "hello"}]}}),
        );
        assert_eq!(d.final_text, "hello");
        let d = parse(
            "deepgram",
            &json!({"type": "Results", "is_final": false, "channel": {"alternatives": [{"transcript": "hel"}]}}),
        );
        assert_eq!(d.interim.as_deref(), Some("hel"));

        let s = parse(
            "soniox",
            &json!({"tokens": [{"text": "Hi", "is_final": true}, {"text": " the", "is_final": false}, {"text": "<end>", "is_final": true}]}),
        );
        assert_eq!(s.final_text, "Hi");
        assert_eq!(s.interim.as_deref(), Some(" the"));
        assert!(
            parse(
                "soniox",
                &json!({"error_code": 401, "error_message": "bad key"})
            )
            .error
            .is_some()
        );

        assert!(parse("gemini", &json!({"setupComplete": {}})).ready);
        let g = parse(
            "gemini",
            &json!({"serverContent": {"interimInputTranscription": {"text": "مرحبا"}}}),
        );
        assert_eq!(g.interim.as_deref(), Some("مرحبا"));
        let g = parse(
            "gemini",
            &json!({"serverContent": {"inputTranscription": {"text": "Hello world."}}}),
        );
        assert_eq!(g.final_text, "Hello world.");
    }

    #[test]
    fn joins_pieces_with_one_space() {
        assert_eq!(join("", " a "), "a");
        assert_eq!(join("a", "b"), "a b");
        assert_eq!(join("a ", ""), "a ");
    }
}
