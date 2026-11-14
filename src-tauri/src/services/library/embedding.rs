//! Local embedding engine. A second `llama-server` instance, started with
//! `--embedding`, serves `/v1/embeddings` for the installed embedding model
//! the user picks. It powers semantic and hybrid library search and memory
//! recall. Nothing here talks to the network beyond 127.0.0.1.

use crate::app::error::AppError;
use crate::services::inference::sidecar::{free_port, locate_binary, new_api_key, open_log};
use crate::services::models::ModelService;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Texts per `/v1/embeddings` call. Chunks are ~1500 characters, so a batch
/// stays well inside the embedding model's context.
const BATCH_SIZE: usize = 16;
const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const EMBED_TIMEOUT: Duration = Duration::from_secs(120);
/// Context for the embedding server itself: chunk text plus prompt overhead.
const EMBED_CONTEXT: i32 = 4096;

struct Running {
    child: Child,
    port: u16,
    api_key: String,
    model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingState {
    pub ready: bool,
    pub model_id: Option<String>,
    pub dimension: Option<usize>,
}

pub struct EmbeddingEngine {
    models: std::sync::Arc<ModelService>,
    running: Mutex<Option<Running>>,
    client: reqwest::blocking::Client,
    dimension: Mutex<Option<usize>>,
}

impl Drop for EmbeddingEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

impl EmbeddingEngine {
    pub fn new(models: std::sync::Arc<ModelService>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(EMBED_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            models,
            running: Mutex::new(None),
            client,
            dimension: Mutex::new(None),
        }
    }

    /// Kill the running embedding server, if any, and wait for it to exit.
    pub fn stop(&self) {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut r) = guard.take() {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    /// Snapshot of the engine state for the UI and status commands.
    pub fn state(&self) -> EmbeddingState {
        let guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        let dimension = *self.dimension.lock().unwrap_or_else(|e| e.into_inner());
        EmbeddingState {
            ready: guard.is_some(),
            model_id: guard.as_ref().map(|r| r.model_id.clone()),
            dimension,
        }
    }

    /// Start the server on the requested embedding model, or on the
    /// smallest installed one when no id is given. Restarting with a
    /// different model is allowed; the stored vectors of the old model are
    /// ignored by search (dimension mismatch scores zero) until a backfill.
    pub fn start(&self, preferred: Option<&str>) -> Result<EmbeddingState, AppError> {
        self.stop();

        let bin = locate_binary()?;
        let port = free_port()?;
        let api_key = new_api_key();
        let (model_id, model_path) = self.resolve_model(preferred)?;

        let mut cmd = Command::new(&bin);
        cmd.arg("-m")
            .arg(&model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--api-key")
            .arg(&api_key)
            .arg("-c")
            .arg(EMBED_CONTEXT.to_string())
            .arg("--embedding")
            .stdin(Stdio::null());

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
            model_id: model_id.clone(),
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

        *self.dimension.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.running.lock().unwrap_or_else(|e| e.into_inner()) = Some(running);
        Ok(self.state())
    }

    fn health_ok(&self, port: u16) -> bool {
        self.client
            .get(format!("http://127.0.0.1:{port}/health"))
            .timeout(Duration::from_secs(2))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Smallest installed embedding model, or the requested one. Prefers
    /// models by RAM so low-memory machines get the lightest engine.
    fn resolve_model(&self, preferred: Option<&str>) -> Result<(String, PathBuf), AppError> {
        let catalog = self.models.manifest();
        let mut candidates: Vec<&crate::services::models::catalog::CatalogEntry> = catalog
            .models
            .iter()
            .filter(|e| e.kind == "embedding")
            .collect();
        // The requested model first, then everything else by RAM estimate.
        candidates.sort_by(|a, b| {
            let a_preferred = preferred == Some(a.id.as_str());
            let b_preferred = preferred == Some(b.id.as_str());
            if a_preferred != b_preferred {
                return if a_preferred {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            }
            a.ram_estimate_gb
                .partial_cmp(&b.ram_estimate_gb)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for entry in candidates {
            let path = self.models.model_file_path(&entry.id);
            if path.is_file() {
                return Ok((entry.id.clone(), path));
            }
        }
        Err(AppError::new(
            "EMBEDDING_NO_MODEL",
            "No embedding model is installed. Download one from the Models screen first.",
        ))
    }

    /// True when the server is up and ready to embed.
    pub fn is_running(&self) -> bool {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Embed a batch of texts. Returns one vector per input, in input order.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        let (port, api_key, model_id) = {
            let guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
            let r = guard.as_ref().ok_or_else(|| {
                AppError::new("EMBEDDING_NOT_READY", "The embedding engine is not running")
            })?;
            (r.port, r.api_key.clone(), r.model_id.clone())
        };

        let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        for batch in texts.chunks(BATCH_SIZE) {
            let response = self
                .client
                .post(format!("http://127.0.0.1:{port}/v1/embeddings"))
                .bearer_auth(&api_key)
                .json(&serde_json::json!({ "input": batch, "model": model_id }))
                .send()
                .map_err(|e| AppError::internal(format!("Embedding request failed: {e}")))?;
            let status = response.status();
            if !status.is_success() {
                return Err(AppError::internal(format!(
                    "Embedding endpoint returned HTTP {status}"
                )));
            }
            let parsed: EmbeddingsResponse = response
                .json()
                .map_err(|e| AppError::internal(format!("Embedding response was invalid: {e}")))?;
            let mut data = parsed.data;
            data.sort_by_key(|item| item.index);
            if data.len() != batch.len() {
                return Err(AppError::internal(format!(
                    "Embedding endpoint returned {} vectors for {} texts",
                    data.len(),
                    batch.len()
                )));
            }
            for item in data {
                out.push(item.embedding);
            }
        }

        let mut dim = self.dimension.lock().unwrap_or_else(|e| e.into_inner());
        if dim.is_none() {
            *dim = out.first().map(|v| v.len());
        }
        Ok(out)
    }

    /// Embed a single text (a search query or memory-recall probe).
    pub fn embed_query(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let mut vectors = self.embed(std::slice::from_ref(&text.to_string()))?;
        vectors
            .pop()
            .ok_or_else(|| AppError::internal("Embedding endpoint returned no vector"))
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingItem {
    index: usize,
    embedding: Vec<f32>,
}
