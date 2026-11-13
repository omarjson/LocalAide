//! Chat engine that runs llama.cpp's `llama-server` as a child process.
//!
//! The server listens on 127.0.0.1 only, on a free port, and requires a random
//! API key on every chat request. The app starts it when a model loads and
//! stops it when a different model loads or the app exits.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{EngineStatus, GenerationMetrics, InferenceEngine, InferenceToken, PromptMessage};
use crate::app::error::AppError;

/// Environment variable that overrides the server binary path. Used in development.
const ENV_BIN: &str = "LOCALAIDE_LLAMA_SERVER";
const BIN_NAME: &str = "llama-server";
const HEALTH_TIMEOUT: Duration = Duration::from_secs(180);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Whole-request timeout for chat. Generation can run for minutes on a large model.
const CHAT_TIMEOUT: Duration = Duration::from_secs(60 * 60);

struct Running {
    child: Child,
    port: u16,
    api_key: String,
    model_path: PathBuf,
    context_size: i32,
    /// True when the model has a vision projector, so it can read images.
    has_mmproj: bool,
}

pub struct SidecarEngine {
    running: Mutex<Option<Running>>,
    client: reqwest::blocking::Client,
}

impl SidecarEngine {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(CHAT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            running: Mutex::new(None),
            client,
        }
    }

    /// Kill the running server, if any, and wait for it to exit.
    fn stop(&self) {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut r) = guard.take() {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    fn health_ok(&self, port: u16) -> bool {
        self.client
            .get(format!("http://127.0.0.1:{port}/health"))
            .timeout(Duration::from_secs(2))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

impl Default for SidecarEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SidecarEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

impl InferenceEngine for SidecarEngine {
    fn load(&self, model_path: &Path, context_size: i32) -> Result<String, AppError> {
        self.stop();

        let bin = locate_binary()?;
        let port = free_port()?;
        let api_key = new_api_key();
        let model_id = model_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut cmd = Command::new(&bin);
        cmd.arg("-m")
            .arg(model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--api-key")
            .arg(&api_key)
            .arg("-c")
            .arg(context_size.to_string())
            .arg("--jinja")
            .stdin(Stdio::null());

        let mmproj = mmproj_for(model_path);
        if let Some(path) = &mmproj {
            cmd.arg("--mmproj").arg(path);
        }

        match open_log() {
            Some((out, err)) => {
                cmd.stdout(Stdio::from(out)).stderr(Stdio::from(err));
            }
            None => {
                cmd.stdout(Stdio::null()).stderr(Stdio::null());
            }
        }

        let child = cmd.spawn().map_err(|e| {
            AppError::model_load_failed(&model_id, &format!("failed to start llama-server: {e}"))
        })?;

        let mut running = Running {
            child,
            port,
            api_key,
            model_path: model_path.to_path_buf(),
            context_size,
            has_mmproj: mmproj.is_some(),
        };

        let deadline = Instant::now() + HEALTH_TIMEOUT;
        loop {
            if let Ok(Some(status)) = running.child.try_wait() {
                return Err(AppError::model_load_failed(
                    &model_id,
                    &format!(
                        "llama-server exited during startup ({status}). See logs/llama-server.log."
                    ),
                ));
            }
            if self.health_ok(running.port) {
                break;
            }
            if Instant::now() >= deadline {
                let _ = running.child.kill();
                let _ = running.child.wait();
                return Err(AppError::model_load_failed(
                    &model_id,
                    "llama-server did not become ready in time",
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }

        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(running);
        Ok(format!("Successfully loaded {}", model_path.display()))
    }

    fn unload(&self) -> Result<(), AppError> {
        self.stop();
        Ok(())
    }

    fn generate(
        &self,
        messages: &[PromptMessage],
        max_tokens: i32,
        temperature: f32,
        cancel_flag: Arc<AtomicBool>,
        mut on_token: Box<dyn FnMut(InferenceToken) + Send>,
    ) -> Result<GenerationMetrics, AppError> {
        let (port, api_key, has_mmproj) = {
            let guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
            let r = guard.as_ref().ok_or_else(|| {
                AppError::model_not_installed("No model loaded in inference engine")
            })?;
            (r.port, r.api_key.clone(), r.has_mmproj)
        };

        let has_images = messages.iter().any(|m| !m.images.is_empty());
        if has_images && !has_mmproj {
            return Err(AppError::invalid_input(
                "The loaded model cannot read images. Choose a model marked (vision).",
            ));
        }

        // The server applies the model's own chat template to the message list.
        let body = json!({
            "messages": messages.iter().map(wire_message).collect::<Vec<Value>>(),
            "max_tokens": max_tokens,
            "temperature": temperature,
            "top_p": 0.9,
            "stream": true
        });

        let start = Instant::now();
        let resp = self
            .client
            .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .map_err(|e| AppError::internal(format!("chat request failed: {e}")))?;

        if !resp.status().is_success() {
            let code = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(AppError::internal(format!(
                "llama-server returned {code}: {text}"
            )));
        }

        let mut total_tokens = 0usize;
        let mut ttft: Option<Duration> = None;
        let mut cancelled = false;

        for line in BufReader::new(resp).lines() {
            if cancel_flag.load(Ordering::Relaxed) {
                cancelled = true;
                break;
            }
            let line = line.map_err(|e| AppError::internal(format!("stream read failed: {e}")))?;
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data.trim() == "[DONE]" {
                break;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            let Some(text) = v["choices"][0]["delta"]["content"].as_str() else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            if ttft.is_none() {
                ttft = Some(start.elapsed());
            }
            on_token(InferenceToken {
                token: text.to_string(),
                index: total_tokens,
            });
            total_tokens += 1;
        }

        let elapsed = start.elapsed().as_secs_f64();
        let tokens_per_sec = if elapsed > 0.0 {
            total_tokens as f64 / elapsed
        } else {
            0.0
        };

        Ok(GenerationMetrics {
            total_tokens,
            ttft,
            tokens_per_sec,
            cancelled,
        })
    }

    fn status(&self) -> EngineStatus {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let exited = match guard.as_mut() {
            Some(r) => matches!(r.child.try_wait(), Ok(Some(_)) | Err(_)),
            None => true,
        };
        if exited {
            *guard = None;
        }
        match guard.as_ref() {
            Some(r) => EngineStatus {
                loaded: true,
                active_model: Some(r.model_path.to_string_lossy().to_string()),
                context_size: r.context_size,
            },
            None => EngineStatus {
                loaded: false,
                active_model: None,
                context_size: 2048,
            },
        }
    }
}

/// Find the `llama-server` binary: the env override first, then next to the app executable.
fn locate_binary() -> Result<PathBuf, AppError> {
    if let Ok(p) = std::env::var(ENV_BIN) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Ok(path);
        }
    }
    let exe = std::env::current_exe().map_err(|e| AppError::internal(e.to_string()))?;
    let dir = exe
        .parent()
        .ok_or_else(|| AppError::internal("cannot locate the app folder"))?;
    for name in [BIN_NAME.to_string(), format!("{BIN_NAME}.exe")] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(AppError::internal(format!(
        "llama-server not found. Put it next to the app or set {ENV_BIN}."
    )))
}

/// Pick a free loopback port. There is a small race before the server binds it.
fn free_port() -> Result<u16, AppError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|e| AppError::internal(format!("cannot pick a port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::internal(e.to_string()))?
        .port();
    drop(listener);
    Ok(port)
}

/// A 256-bit random key, hex encoded. Reads the OS random source where available.
fn new_api_key() -> String {
    let mut bytes = [0u8; 32];
    let from_os = File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok();
    if from_os {
        return bytes.iter().map(|b| format!("{b:02x}")).collect();
    }
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::with_capacity(64);
    for i in 0..4u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Vision projector that sits next to a model: `{id}.gguf` pairs with `{id}.mmproj.gguf`.
fn mmproj_for(model_path: &Path) -> Option<PathBuf> {
    let stem = model_path.file_stem()?.to_string_lossy().to_string();
    let candidate = model_path.with_file_name(format!("{stem}.mmproj.gguf"));
    candidate.is_file().then_some(candidate)
}

/// Open the server log in the app folder for stdout and stderr.
fn open_log() -> Option<(File, File)> {
    let dir = crate::app::paths::app_dir().join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("llama-server.log"))
        .ok()?;
    let second = file.try_clone().ok()?;
    Some((file, second))
}

/// Convert one chat turn to the OpenAI-style wire format. Images become content parts.
fn wire_message(m: &PromptMessage) -> Value {
    if m.images.is_empty() {
        return json!({ "role": m.role, "content": m.content });
    }
    let mut parts = vec![json!({ "type": "text", "text": m.content })];
    for img in &m.images {
        parts.push(json!({ "type": "image_url", "image_url": { "url": img } }));
    }
    json!({ "role": m.role, "content": parts })
}
