//! OpenAI-compatible chat streaming, shared by the local `llama-server`
//! sidecar and the opt-in online providers.
//!
//! One request streams server-sent events. Each event can carry answer text
//! (`delta.content`), the model's thinking (`delta.reasoning_content` from
//! llama.cpp and DeepSeek, `delta.reasoning` from OpenRouter, Groq and
//! Ollama, or `<think>` tags inside the text), and tool calls
//! (`delta.tool_calls`, streamed in pieces). This module turns that stream
//! into [`Delta`] callbacks and one [`StreamOutcome`].
//!
//! Providers disagree on optional fields. When a provider refuses a request
//! with 400 or 422 and names a field, the request is sent again without it
//! (see [`adjust_for_error`]); the caller learns which fields were dropped.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::app::error::AppError;

/// How often the reply loop checks the cancel flag while the server is busy.
const STREAM_POLL: Duration = Duration::from_millis(200);
/// Longest silence before a reply counts as stuck. Prompt processing on a
/// large local context can take minutes, so the limit is generous.
const STREAM_STALL_LIMIT: Duration = Duration::from_secs(10 * 60);
/// Error bodies can be whole HTML pages; only this much is kept.
const ERROR_BODY_CHARS: usize = 400;
/// Error code when the model refused the tool list.
pub const TOOLS_UNSUPPORTED: &str = "TOOLS_UNSUPPORTED";

/// One turn of a conversation in the shape the chat endpoint expects.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatTurn {
    System(String),
    User {
        text: String,
        /// `data:image/...;base64,` URLs.
        images: Vec<String>,
    },
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        call_id: String,
        name: String,
        content: String,
    },
}

/// A tool call the model asked for. `arguments` is the JSON text the model
/// wrote; it may be malformed, so it is parsed only when the tool runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Which server the request goes to. Each one spells a few fields its own way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    /// The local llama.cpp server.
    LlamaServer,
    /// api.openai.com: `max_completion_tokens`, no temperature with reasoning.
    OpenAi,
    /// openrouter.ai and gateways built on it: `reasoning: {effort}`.
    OpenRouter,
    /// Any other OpenAI-compatible provider.
    Generic,
}

impl Flavor {
    /// The flavor for a provider address.
    pub fn for_base_url(base_url: &str) -> Self {
        let host = url::Url::parse(base_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
            .unwrap_or_default();
        if host == "api.openai.com" {
            Self::OpenAi
        } else if host == "openrouter.ai"
            || host.ends_with(".openrouter.ai")
            || host == "api.kilo.ai"
        {
            Self::OpenRouter
        } else {
            Self::Generic
        }
    }
}

/// How hard the model should think. `Auto` leaves the model's default alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effort {
    Auto,
    Off,
    Level(String),
}

impl Effort {
    /// "auto" or empty = Auto, "off"/"none" = Off, anything else a level.
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("auto") | Some("default") => Self::Auto,
            Some("off") | Some("none") => Self::Off,
            Some(level) => Self::Level(level.to_string()),
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }

    /// Extra tokens to allow for thinking on top of the answer length.
    pub fn thinking_allowance(&self) -> i32 {
        match self {
            Self::Off => 0,
            Self::Auto => 4096,
            Self::Level(level) => match level.as_str() {
                "minimal" => 1024,
                "low" => 2048,
                "medium" => 4096,
                "high" => 8192,
                _ => 12_288,
            },
        }
    }
}

/// Everything about one request besides the messages.
#[derive(Debug, Clone)]
pub struct CallOptions {
    pub max_tokens: i32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    /// OpenAI-style tool schemas. Empty = no tools.
    pub tools: Vec<Value>,
    /// Ask the model to answer without calling a tool this round.
    pub forbid_tools: bool,
    pub effort: Effort,
    /// Ask the server to report token usage at the end of the stream.
    pub usage: bool,
    /// The OpenAI field name for the answer limit.
    pub max_completion_tokens: bool,
}

/// Where the request goes.
#[derive(Clone)]
pub struct Endpoint {
    /// The full `/chat/completions` address.
    pub url: String,
    /// Bearer token. Not `Debug` on purpose: it may be an API key.
    pub bearer: Option<String>,
    /// The provider's name, for error messages.
    pub label: String,
    pub flavor: Flavor,
    /// The `model` field. Empty for the local server, which has one model.
    pub model: String,
    /// True when a clean close without `[DONE]` means the answer was cut.
    pub require_done: bool,
}

/// A piece of the streamed reply.
#[derive(Debug, Clone, PartialEq)]
pub enum Delta {
    Content(String),
    Reasoning(String),
}

/// What one streamed reply produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StreamOutcome {
    pub content: String,
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
    /// From the usage block, when the server sends one.
    pub completion_tokens: Option<usize>,
    /// Content and reasoning pieces received, a fallback token count.
    pub pieces: usize,
    pub cancelled: bool,
    pub saw_done: bool,
    pub first_token_after: Option<Duration>,
}

/// A field removed after the provider refused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjustment {
    DroppedTools,
    DroppedEffort,
    DroppedTemperature,
    DroppedUsage,
    SwitchedTokenField,
    DroppedImages,
}

/// Convert one turn to the wire format. Images become content parts.
/// `tool_names` adds the tool's name to tool results, which some local chat
/// templates print; strict providers reject the extra field.
pub fn turn_to_wire(turn: &ChatTurn, tool_names: bool) -> Value {
    match turn {
        ChatTurn::System(text) => json!({ "role": "system", "content": text }),
        ChatTurn::User { text, images } => {
            if images.is_empty() {
                return json!({ "role": "user", "content": text });
            }
            let mut parts = vec![json!({ "type": "text", "text": text })];
            for img in images {
                parts.push(json!({ "type": "image_url", "image_url": { "url": img } }));
            }
            json!({ "role": "user", "content": parts })
        }
        ChatTurn::Assistant { text, tool_calls } => {
            let mut msg = Map::new();
            msg.insert("role".into(), json!("assistant"));
            msg.insert("content".into(), json!(text));
            if !tool_calls.is_empty() {
                let calls: Vec<Value> = tool_calls
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.id,
                            "type": "function",
                            "function": { "name": c.name, "arguments": c.arguments }
                        })
                    })
                    .collect();
                msg.insert("tool_calls".into(), Value::Array(calls));
            }
            Value::Object(msg)
        }
        ChatTurn::Tool {
            call_id,
            name,
            content,
        } => {
            let mut msg = json!({ "role": "tool", "tool_call_id": call_id, "content": content });
            if tool_names {
                msg["name"] = json!(name);
            }
            msg
        }
    }
}

/// Build the request body for one round.
pub fn build_body(endpoint: &Endpoint, turns: &[ChatTurn], opts: &CallOptions) -> Value {
    let local = endpoint.flavor == Flavor::LlamaServer;
    let messages: Vec<Value> = turns.iter().map(|t| turn_to_wire(t, local)).collect();
    let mut body = Map::new();
    if !endpoint.model.is_empty() {
        body.insert("model".into(), json!(endpoint.model));
    }
    body.insert("messages".into(), Value::Array(messages));
    body.insert("stream".into(), json!(true));
    let limit_field = if opts.max_completion_tokens {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };
    body.insert(limit_field.into(), json!(opts.max_tokens));
    if let Some(t) = opts.temperature {
        body.insert("temperature".into(), json!(t));
    }
    if let Some(p) = opts.top_p {
        body.insert("top_p".into(), json!(p));
    }
    if !opts.tools.is_empty() {
        body.insert("tools".into(), Value::Array(opts.tools.clone()));
        body.insert(
            "tool_choice".into(),
            json!(if opts.forbid_tools { "none" } else { "auto" }),
        );
    }
    match (&opts.effort, endpoint.flavor) {
        (Effort::Auto, _) => {}
        (Effort::Off, Flavor::OpenRouter) => {
            body.insert("reasoning".into(), json!({ "effort": "none" }));
        }
        (Effort::Level(level), Flavor::OpenRouter) => {
            body.insert("reasoning".into(), json!({ "effort": level }));
        }
        (Effort::Off, _) => {
            body.insert("reasoning_effort".into(), json!("none"));
        }
        (Effort::Level(level), _) => {
            body.insert("reasoning_effort".into(), json!(level));
        }
    }
    if opts.usage {
        body.insert("stream_options".into(), json!({ "include_usage": true }));
    }
    Value::Object(body)
}

/// The options to start with for an endpoint.
pub fn initial_options(
    flavor: Flavor,
    max_tokens: i32,
    temperature: f32,
    tools: Vec<Value>,
    effort: Effort,
) -> CallOptions {
    let reasoning_requested = matches!(effort, Effort::Level(_));
    CallOptions {
        max_tokens,
        // OpenAI's reasoning models accept only the default temperature.
        temperature: if flavor == Flavor::OpenAi && reasoning_requested {
            None
        } else {
            Some(temperature)
        },
        top_p: if flavor == Flavor::LlamaServer {
            Some(0.9)
        } else {
            None
        },
        tools,
        forbid_tools: false,
        effort,
        usage: matches!(
            flavor,
            Flavor::LlamaServer | Flavor::OpenAi | Flavor::OpenRouter
        ),
        max_completion_tokens: flavor == Flavor::OpenAi,
    }
}

/// After a 400 or 422, decide which field to drop. Returns None when nothing
/// optional is left to try.
pub fn adjust_for_error(
    opts: &mut CallOptions,
    turns: &mut [ChatTurn],
    error_body: &str,
) -> Option<Adjustment> {
    let text = error_body.to_ascii_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|w| text.contains(w));

    // A conversation that no longer fits is not fixed by dropping fields.
    if mentions(&[
        "context size",
        "context length",
        "context_length",
        "context window",
        "maximum context",
        "exceeds the available",
        "too many tokens",
        "prompt is too long",
    ]) {
        return None;
    }

    if !opts.max_completion_tokens && mentions(&["max_completion_tokens"]) {
        opts.max_completion_tokens = true;
        return Some(Adjustment::SwitchedTokenField);
    }
    if opts.usage && mentions(&["stream_options", "include_usage"]) {
        opts.usage = false;
        return Some(Adjustment::DroppedUsage);
    }
    if !matches!(opts.effort, Effort::Auto) && mentions(&["reasoning", "effort", "thinking"]) {
        opts.effort = Effort::Auto;
        return Some(Adjustment::DroppedEffort);
    }
    if opts.temperature.is_some() && mentions(&["temperature"]) {
        opts.temperature = None;
        return Some(Adjustment::DroppedTemperature);
    }
    if !opts.tools.is_empty() && mentions(&["tool", "function"]) {
        opts.tools.clear();
        return Some(Adjustment::DroppedTools);
    }
    let has_images = turns
        .iter()
        .any(|t| matches!(t, ChatTurn::User { images, .. } if !images.is_empty()));
    if has_images && mentions(&["image", "vision", "multimodal", "content part"]) {
        for turn in turns.iter_mut() {
            if let ChatTurn::User { images, .. } = turn {
                images.clear();
            }
        }
        return Some(Adjustment::DroppedImages);
    }
    // Nothing named. Optional extras go first, tools last, since tools are
    // what the chat most wants to keep.
    if opts.usage {
        opts.usage = false;
        return Some(Adjustment::DroppedUsage);
    }
    if !matches!(opts.effort, Effort::Auto) {
        opts.effort = Effort::Auto;
        return Some(Adjustment::DroppedEffort);
    }
    if !opts.tools.is_empty() {
        opts.tools.clear();
        return Some(Adjustment::DroppedTools);
    }
    None
}

/// Splits `<think>…</think>` out of streamed text. Tags may arrive split
/// across pieces, so a possible partial tag is held back until the next
/// piece shows what it is.
#[derive(Debug, Default)]
pub struct ThinkSplitter {
    pending: String,
    in_think: bool,
}

const OPEN_TAG: &str = "<think>";
const CLOSE_TAG: &str = "</think>";

impl ThinkSplitter {
    pub fn push(&mut self, piece: &str) -> Vec<Delta> {
        self.pending.push_str(piece);
        let mut out = Vec::new();
        loop {
            let tag = if self.in_think { CLOSE_TAG } else { OPEN_TAG };
            let tag_at = self.pending.find(tag);
            // A stray close tag outside thinking: what came before it was
            // thinking the server did not mark.
            let stray_at = if self.in_think {
                None
            } else {
                self.pending.find(CLOSE_TAG)
            };
            if let Some(pos) = stray_at
                && tag_at.is_none_or(|t| pos < t)
            {
                let before: String = self.pending[..pos].to_string();
                if !before.trim().is_empty() {
                    out.push(Delta::Reasoning(before));
                }
                self.pending.drain(..pos + CLOSE_TAG.len());
                continue;
            }
            if let Some(pos) = tag_at {
                let before: String = self.pending[..pos].to_string();
                self.emit(&before, &mut out);
                self.pending.drain(..pos + tag.len());
                self.in_think = !self.in_think;
                continue;
            }
            // Hold back a suffix that could be the start of a tag.
            let keep = partial_tag_suffix(&self.pending, tag).max(if self.in_think {
                0
            } else {
                partial_tag_suffix(&self.pending, CLOSE_TAG)
            });
            let cut = self.pending.len() - keep;
            let ready: String = self.pending[..cut].to_string();
            self.emit(&ready, &mut out);
            self.pending.drain(..cut);
            break;
        }
        out
    }

    /// Whatever is held back at the end of the stream.
    pub fn finish(&mut self) -> Vec<Delta> {
        let rest = std::mem::take(&mut self.pending);
        let mut out = Vec::new();
        self.emit(&rest, &mut out);
        out
    }

    fn emit(&self, text: &str, out: &mut Vec<Delta>) {
        if text.is_empty() {
            return;
        }
        if self.in_think {
            out.push(Delta::Reasoning(text.to_string()));
        } else {
            out.push(Delta::Content(text.to_string()));
        }
    }
}

/// Length of the longest suffix of `text` that is a proper prefix of `tag`.
fn partial_tag_suffix(text: &str, tag: &str) -> usize {
    let max = tag.len().saturating_sub(1).min(text.len());
    (1..=max)
        .rev()
        .find(|&n| {
            text.is_char_boundary(text.len() - n) && tag.starts_with(&text[text.len() - n..])
        })
        .unwrap_or(0)
}

/// Tool calls arrive in pieces keyed by `index`. Some providers (Gemini)
/// send each call whole and leave the index out.
#[derive(Debug, Default)]
pub struct ToolCallAccumulator {
    calls: Vec<ToolCall>,
    indexes: Vec<u64>,
}

impl ToolCallAccumulator {
    pub fn push(&mut self, pieces: &[Value]) {
        for (position, piece) in pieces.iter().enumerate() {
            let id = piece
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            let index = piece.get("index").and_then(Value::as_u64);
            let slot = match (index, id) {
                (Some(i), _) => self
                    .indexes
                    .iter()
                    .position(|&x| x == i)
                    .unwrap_or_else(|| {
                        self.indexes.push(i);
                        self.calls.push(empty_call());
                        self.calls.len() - 1
                    }),
                (None, Some(id)) => {
                    self.calls
                        .iter()
                        .position(|c| c.id == id)
                        .unwrap_or_else(|| {
                            self.indexes.push(u64::MAX - position as u64);
                            self.calls.push(empty_call());
                            self.calls.len() - 1
                        })
                }
                (None, None) => {
                    self.indexes.push(u64::MAX - 1000 - self.calls.len() as u64);
                    self.calls.push(empty_call());
                    self.calls.len() - 1
                }
            };
            let call = &mut self.calls[slot];
            if let Some(id) = id
                && call.id.is_empty()
            {
                call.id = id.to_string();
            }
            let function = piece.get("function");
            if let Some(name) = function.and_then(|f| f.get("name")).and_then(Value::as_str)
                && call.name.is_empty()
            {
                call.name = name.to_string();
            }
            match function.and_then(|f| f.get("arguments")) {
                Some(Value::String(part)) => call.arguments.push_str(part),
                // A few servers send the arguments as an object.
                Some(other @ Value::Object(_)) => call.arguments = other.to_string(),
                _ => {}
            }
        }
    }

    /// The finished calls. Calls without a name are dropped; missing ids are
    /// filled in, since the reply must name the call it answers.
    pub fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .filter(|c| !c.name.trim().is_empty())
            .enumerate()
            .map(|(i, mut c)| {
                if c.id.is_empty() {
                    c.id = format!("call_{i}");
                }
                if c.arguments.trim().is_empty() {
                    c.arguments = "{}".to_string();
                }
                c
            })
            .collect()
    }
}

fn empty_call() -> ToolCall {
    ToolCall {
        id: String::new(),
        name: String::new(),
        arguments: String::new(),
    }
}

/// Apply one parsed SSE event. Returns the text pieces it carried, or the
/// error message when the provider streamed an error.
pub fn apply_event(
    event: &Value,
    splitter: &mut ThinkSplitter,
    calls: &mut ToolCallAccumulator,
    outcome: &mut StreamOutcome,
) -> Result<Vec<Delta>, String> {
    if let Some(err) = event.get("error").filter(|e| !e.is_null()) {
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| err.to_string());
        return Err(message);
    }
    if let Some(tokens) = event
        .get("usage")
        .and_then(|u| u.get("completion_tokens"))
        .and_then(Value::as_u64)
    {
        outcome.completion_tokens = Some(tokens as usize);
    }
    let Some(choice) = event.get("choices").and_then(|c| c.get(0)) else {
        return Ok(Vec::new());
    };
    if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
        outcome.finish_reason = Some(reason.to_string());
    }
    let delta = choice.get("delta").or_else(|| choice.get("message"));
    let Some(delta) = delta else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for key in ["reasoning_content", "reasoning", "thinking"] {
        if let Some(text) = delta.get(key).and_then(Value::as_str)
            && !text.is_empty()
        {
            out.push(Delta::Reasoning(text.to_string()));
            break;
        }
    }
    if let Some(text) = delta.get("content").and_then(Value::as_str)
        && !text.is_empty()
    {
        out.extend(splitter.push(text));
    }
    if let Some(pieces) = delta.get("tool_calls").and_then(Value::as_array) {
        calls.push(pieces);
    }
    Ok(out)
}

/// A refused request, before any text arrived.
#[derive(Debug)]
pub enum StreamError {
    Http { status: u16, body: String },
    Other(AppError),
}

/// Plain words for a provider's HTTP error.
pub fn http_error_message(label: &str, status: u16, body: &str, has_key: bool) -> String {
    let hint = match status {
        401 | 403 if !has_key => {
            " This provider needs an API key. Add one in Settings, Online providers."
        }
        401 | 403 => " The API key looks wrong or unauthorized.",
        402 | 429 => {
            " The free limit is used up for now. Wait a minute and try again, or add your own key."
        }
        404 => " The model name may be wrong. Pick another model.",
        _ if body.to_ascii_lowercase().contains("context") => {
            " The conversation is too long for this model. Start a new chat, or pick a model with a larger context."
        }
        _ => "",
    };
    let body: String = body.chars().take(ERROR_BODY_CHARS).collect();
    format!("{label} returned {status}:{hint} {}", body.trim())
}

/// Send one request and stream the reply. Text pieces go to `on_delta` as
/// they arrive. Cancel is checked every 200 ms, also while the server is
/// still reading the prompt.
pub fn stream_once(
    client: &reqwest::blocking::Client,
    endpoint: &Endpoint,
    body: &Value,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<StreamOutcome, StreamError> {
    let start = Instant::now();
    let mut request = client.post(&endpoint.url).json(body);
    if let Some(key) = endpoint.bearer.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.bearer_auth(key);
    }
    let response = request.send().map_err(|e| {
        StreamError::Other(AppError::internal(format!(
            "Could not reach {}: {e}. Check the address and your connection.",
            endpoint.label
        )))
    })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().unwrap_or_default();
        return Err(StreamError::Http { status, body });
    }

    // A helper thread reads the stream, so this loop can check the cancel
    // flag while the server is silent. When this function returns, the
    // receiver drops; the helper stops at its next line and closes the
    // connection, which tells the server to stop generating.
    let (lines_tx, lines_rx) = std::sync::mpsc::channel::<std::io::Result<String>>();
    std::thread::spawn(move || {
        for line in BufReader::new(response).lines() {
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut outcome = StreamOutcome::default();
    let mut splitter = ThinkSplitter::default();
    let mut calls = ToolCallAccumulator::default();
    let mut last_activity = Instant::now();
    let mut deliver = |pieces: Vec<Delta>, outcome: &mut StreamOutcome| {
        for piece in pieces {
            if outcome.first_token_after.is_none() {
                outcome.first_token_after = Some(start.elapsed());
            }
            outcome.pieces += 1;
            match &piece {
                Delta::Content(t) => outcome.content.push_str(t),
                Delta::Reasoning(t) => outcome.reasoning.push_str(t),
            }
            on_delta(piece);
        }
    };

    loop {
        if cancel.load(Ordering::Relaxed) {
            outcome.cancelled = true;
            break;
        }
        let line = match lines_rx.recv_timeout(STREAM_POLL) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                if last_activity.elapsed() > STREAM_STALL_LIMIT {
                    return Err(StreamError::Other(AppError::internal(format!(
                        "{} stopped responding. Try again.",
                        endpoint.label
                    ))));
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        last_activity = Instant::now();
        let line = line.map_err(|e| {
            StreamError::Other(AppError::internal(format!(
                "The answer stream broke off: {e}"
            )))
        })?;
        // The SSE format allows the space after "data:" to be left out.
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.strip_prefix(' ').unwrap_or(data);
        if data.trim() == "[DONE]" {
            outcome.saw_done = true;
            break;
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        match apply_event(&event, &mut splitter, &mut calls, &mut outcome) {
            Ok(pieces) => deliver(pieces, &mut outcome),
            Err(message) => {
                return Err(StreamError::Other(AppError::internal(format!(
                    "{} streamed an error: {message}",
                    endpoint.label
                ))));
            }
        }
    }
    let rest = splitter.finish();
    deliver(rest, &mut outcome);
    outcome.tool_calls = calls.finish();
    if endpoint.require_done
        && !outcome.cancelled
        && !outcome.saw_done
        && outcome.finish_reason.is_none()
    {
        return Err(StreamError::Other(AppError::internal(format!(
            "{} closed the stream before the answer was complete.",
            endpoint.label
        ))));
    }
    Ok(outcome)
}

/// Stream one round, retrying without optional fields the provider refuses.
/// Returns the outcome and the adjustments that were needed. Retries happen
/// only before any text arrived, so nothing is shown twice.
pub fn stream_with_retries(
    client: &reqwest::blocking::Client,
    endpoint: &Endpoint,
    turns: &mut [ChatTurn],
    opts: &mut CallOptions,
    cancel: &AtomicBool,
    on_delta: &mut dyn FnMut(Delta),
) -> Result<(StreamOutcome, Vec<Adjustment>), AppError> {
    let mut adjustments = Vec::new();
    loop {
        let body = build_body(endpoint, turns, opts);
        match stream_once(client, endpoint, &body, cancel, on_delta) {
            Ok(outcome) => return Ok((outcome, adjustments)),
            Err(StreamError::Http { status, body }) => {
                let retryable = matches!(status, 400 | 422 | 500) && adjustments.len() < 4;
                // A local server answers 500 when its template cannot render
                // tools; other providers use 400 and 422.
                if retryable
                    && (status != 500 || endpoint.flavor == Flavor::LlamaServer)
                    && let Some(adjustment) = adjust_for_error(opts, turns, &body)
                {
                    log::info!(
                        "{} refused a field ({status}); retrying with {adjustment:?}",
                        endpoint.label
                    );
                    if adjustment == Adjustment::DroppedTools {
                        // The caller decides how to answer without tools.
                        return Err(AppError::new(
                            TOOLS_UNSUPPORTED,
                            format!("{} cannot use tools.", endpoint.label),
                        ));
                    }
                    adjustments.push(adjustment);
                    continue;
                }
                let has_key = endpoint
                    .bearer
                    .as_deref()
                    .is_some_and(|k| !k.trim().is_empty());
                return Err(AppError::internal(http_error_message(
                    &endpoint.label,
                    status,
                    &body,
                    has_key,
                )));
            }
            Err(StreamError::Other(e)) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_all(pieces: &[&str]) -> (String, String) {
        let mut s = ThinkSplitter::default();
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut all: Vec<Delta> = pieces.iter().flat_map(|p| s.push(p)).collect();
        all.extend(s.finish());
        for d in all {
            match d {
                Delta::Content(t) => content.push_str(&t),
                Delta::Reasoning(t) => reasoning.push_str(&t),
            }
        }
        (content, reasoning)
    }

    #[test]
    fn think_tags_split_across_pieces() {
        let (c, r) = split_all(&["<thi", "nk>plan it", "</th", "ink>Answer"]);
        assert_eq!(r, "plan it");
        assert_eq!(c, "Answer");
        let (c, r) = split_all(&["Hello <b>world</b>"]);
        assert_eq!(c, "Hello <b>world</b>");
        assert!(r.is_empty());
        let (c, r) = split_all(&["a <", "b"]);
        assert_eq!(c, "a <b");
        assert!(r.is_empty());
    }

    #[test]
    fn a_stray_close_tag_marks_earlier_text_as_thinking() {
        let (c, r) = split_all(&["thinking here</think>", "The answer"]);
        assert_eq!(r, "thinking here");
        assert_eq!(c, "The answer");
    }

    #[test]
    fn multibyte_text_near_a_tag_boundary_is_safe() {
        let (c, _) = split_all(&["héllo ✓ <", "thi", "ngs"]);
        assert_eq!(c, "héllo ✓ <things");
    }

    #[test]
    fn reasoning_fields_and_tool_calls_are_read() {
        let mut s = ThinkSplitter::default();
        let mut calls = ToolCallAccumulator::default();
        let mut out = StreamOutcome::default();
        let events = [
            json!({"choices":[{"delta":{"reasoning_content":"Let me look"}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"web_search","arguments":"{\"qu"}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ery\":\"rust\"}"}}]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
            json!({"choices":[],"usage":{"completion_tokens":42}}),
        ];
        let mut pieces = Vec::new();
        for e in &events {
            pieces.extend(apply_event(e, &mut s, &mut calls, &mut out).unwrap());
        }
        assert_eq!(pieces, vec![Delta::Reasoning("Let me look".into())]);
        let calls = calls.finish();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "c1");
        assert_eq!(calls[0].name, "web_search");
        assert_eq!(calls[0].arguments, "{\"query\":\"rust\"}");
        assert_eq!(out.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(out.completion_tokens, Some(42));
    }

    #[test]
    fn whole_calls_without_index_and_openrouter_reasoning() {
        let mut s = ThinkSplitter::default();
        let mut calls = ToolCallAccumulator::default();
        let mut out = StreamOutcome::default();
        let e = json!({"choices":[{"delta":{"reasoning":"hmm","tool_calls":[
            {"id":"a","function":{"name":"one","arguments":"{}"}},
            {"id":"b","function":{"name":"two","arguments":{"x":1}}}
        ]}}]});
        let pieces = apply_event(&e, &mut s, &mut calls, &mut out).unwrap();
        assert_eq!(pieces, vec![Delta::Reasoning("hmm".into())]);
        let calls = calls.finish();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].name, "two");
        assert_eq!(calls[1].arguments, "{\"x\":1}");
    }

    #[test]
    fn streamed_errors_are_reported() {
        let mut s = ThinkSplitter::default();
        let mut calls = ToolCallAccumulator::default();
        let mut out = StreamOutcome::default();
        let e = json!({"error":{"message":"overloaded"}});
        assert_eq!(
            apply_event(&e, &mut s, &mut calls, &mut out).unwrap_err(),
            "overloaded"
        );
    }

    fn endpoint(flavor: Flavor) -> Endpoint {
        Endpoint {
            url: "http://127.0.0.1:1/v1/chat/completions".into(),
            bearer: None,
            label: "Test".into(),
            flavor,
            model: if flavor == Flavor::LlamaServer {
                String::new()
            } else {
                "m".into()
            },
            require_done: flavor != Flavor::LlamaServer,
        }
    }

    #[test]
    fn bodies_follow_each_flavor() {
        let turns = vec![ChatTurn::User {
            text: "hi".into(),
            images: vec![],
        }];
        let tool = json!({"type":"function","function":{"name":"x","parameters":{}}});

        let opts = initial_options(
            Flavor::OpenAi,
            100,
            0.7,
            vec![tool.clone()],
            Effort::Level("high".into()),
        );
        let body = build_body(&endpoint(Flavor::OpenAi), &turns, &opts);
        assert_eq!(body["max_completion_tokens"], 100);
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("temperature").is_none());
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["tool_choice"], "auto");

        let opts = initial_options(
            Flavor::OpenRouter,
            100,
            0.7,
            vec![],
            Effort::Level("low".into()),
        );
        let body = build_body(&endpoint(Flavor::OpenRouter), &turns, &opts);
        assert_eq!(body["reasoning"]["effort"], "low");
        assert!(body.get("tools").is_none());

        let opts = initial_options(Flavor::LlamaServer, 100, 0.7, vec![], Effort::Off);
        let body = build_body(&endpoint(Flavor::LlamaServer), &turns, &opts);
        assert_eq!(body["reasoning_effort"], "none");
        assert!(body.get("model").is_none());
        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn refused_fields_are_dropped_one_at_a_time() {
        let tool = json!({"type":"function"});
        let mut turns = vec![ChatTurn::User {
            text: "hi".into(),
            images: vec!["data:image/png;base64,AA".into()],
        }];
        let mut opts = initial_options(
            Flavor::Generic,
            100,
            0.7,
            vec![tool],
            Effort::Level("high".into()),
        );
        assert_eq!(
            adjust_for_error(&mut opts, &mut turns, "Unknown parameter: reasoning_effort"),
            Some(Adjustment::DroppedEffort)
        );
        assert_eq!(
            adjust_for_error(&mut opts, &mut turns, "this model does not support tools"),
            Some(Adjustment::DroppedTools)
        );
        assert_eq!(
            adjust_for_error(&mut opts, &mut turns, "image input is not supported"),
            Some(Adjustment::DroppedImages)
        );
        assert_eq!(adjust_for_error(&mut opts, &mut turns, "bad request"), None);
    }

    #[test]
    fn assistant_tool_calls_and_results_round_trip_to_the_wire() {
        let call = ToolCall {
            id: "c1".into(),
            name: "web_search".into(),
            arguments: "{}".into(),
        };
        let a = turn_to_wire(
            &ChatTurn::Assistant {
                text: String::new(),
                tool_calls: vec![call],
            },
            false,
        );
        assert_eq!(a["tool_calls"][0]["function"]["name"], "web_search");
        assert_eq!(a["tool_calls"][0]["type"], "function");
        let t = turn_to_wire(
            &ChatTurn::Tool {
                call_id: "c1".into(),
                name: "web_search".into(),
                content: "ok".into(),
            },
            false,
        );
        assert_eq!(t["tool_call_id"], "c1");
        assert!(t.get("name").is_none());
        let t = turn_to_wire(
            &ChatTurn::Tool {
                call_id: "c1".into(),
                name: "web_search".into(),
                content: "ok".into(),
            },
            true,
        );
        assert_eq!(t["name"], "web_search");
    }

    /// A one-connection-per-answer HTTP server on loopback. Each answer is
    /// (status, body); the request bodies it saw are returned.
    fn mock_server(answers: Vec<(u16, String)>) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (status, body) in answers {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                // Read the head, then the body by Content-Length.
                let (head_end, length) = loop {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break (buf.len(), 0);
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                        let length = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        break (pos + 4, length);
                    }
                };
                while buf.len() < head_end + length {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let _ = tx.send(String::from_utf8_lossy(&buf[head_end..]).to_string());
                let kind = if status == 200 {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        (url, rx)
    }

    #[test]
    fn a_refused_field_is_dropped_and_the_stream_is_read() {
        let sse = [
            r#"data: {"choices":[{"delta":{"reasoning_content":"Think"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c","function":{"name":"web_search","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}"#,
            "data: [DONE]",
            "",
        ]
        .join("\n\n");
        let (url, seen) = mock_server(vec![
            (
                400,
                r#"{"error":{"message":"Unrecognized request argument: reasoning_effort"}}"#.into(),
            ),
            (200, sse),
        ]);
        let endpoint = Endpoint {
            url,
            ..endpoint(Flavor::Generic)
        };
        let client = reqwest::blocking::Client::new();
        let mut turns = vec![ChatTurn::User {
            text: "hi".into(),
            images: vec![],
        }];
        let mut opts = initial_options(
            Flavor::Generic,
            100,
            0.5,
            vec![json!({"type":"function"})],
            Effort::Level("high".into()),
        );
        let cancel = AtomicBool::new(false);
        let mut deltas = Vec::new();
        let (outcome, adjustments) = stream_with_retries(
            &client,
            &endpoint,
            &mut turns,
            &mut opts,
            &cancel,
            &mut |d| deltas.push(d),
        )
        .unwrap();
        assert_eq!(adjustments, vec![Adjustment::DroppedEffort]);
        assert_eq!(outcome.content, "Hi");
        assert_eq!(outcome.reasoning, "Think");
        assert_eq!(outcome.tool_calls.len(), 1);
        assert!(outcome.saw_done);
        assert_eq!(deltas.len(), 2);
        let first: Value = serde_json::from_str(&seen.recv().unwrap()).unwrap();
        let second: Value = serde_json::from_str(&seen.recv().unwrap()).unwrap();
        assert_eq!(first["reasoning_effort"], "high");
        assert!(second.get("reasoning_effort").is_none());
        assert!(second.get("tools").is_some());
    }

    #[test]
    fn refused_tools_are_reported_to_the_caller() {
        let (url, _seen) =
            mock_server(vec![(400, r#"{"error":"tools are not supported"}"#.into())]);
        let endpoint = Endpoint {
            url,
            ..endpoint(Flavor::Generic)
        };
        let client = reqwest::blocking::Client::new();
        let mut turns = vec![ChatTurn::User {
            text: "hi".into(),
            images: vec![],
        }];
        let mut opts = initial_options(
            Flavor::Generic,
            100,
            0.5,
            vec![json!({"type":"function"})],
            Effort::Auto,
        );
        let cancel = AtomicBool::new(false);
        let err = stream_with_retries(
            &client,
            &endpoint,
            &mut turns,
            &mut opts,
            &cancel,
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(err.code, TOOLS_UNSUPPORTED);
        assert!(opts.tools.is_empty());
    }

    #[test]
    fn a_cut_stream_from_a_provider_is_an_error() {
        let (url, _seen) = mock_server(vec![(
            200,
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hal\"}}]}\n\n".into(),
        )]);
        let endpoint = Endpoint {
            url,
            ..endpoint(Flavor::Generic)
        };
        let client = reqwest::blocking::Client::new();
        let mut turns = vec![ChatTurn::User {
            text: "hi".into(),
            images: vec![],
        }];
        let mut opts = initial_options(Flavor::Generic, 100, 0.5, vec![], Effort::Auto);
        let cancel = AtomicBool::new(false);
        let err = stream_with_retries(
            &client,
            &endpoint,
            &mut turns,
            &mut opts,
            &cancel,
            &mut |_| {},
        )
        .unwrap_err();
        assert!(err.message.contains("before the answer was complete"));
    }

    #[test]
    fn flavors_follow_the_host() {
        assert_eq!(
            Flavor::for_base_url("https://api.openai.com/v1"),
            Flavor::OpenAi
        );
        assert_eq!(
            Flavor::for_base_url("https://openrouter.ai/api/v1"),
            Flavor::OpenRouter
        );
        assert_eq!(
            Flavor::for_base_url("https://api.groq.com/openai/v1"),
            Flavor::Generic
        );
        assert_eq!(Effort::parse(Some("High")), Effort::Level("high".into()));
        assert_eq!(Effort::parse(Some("off")), Effort::Off);
        assert_eq!(Effort::parse(None), Effort::Auto);
    }
}
