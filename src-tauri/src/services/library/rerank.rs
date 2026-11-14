//! Local reranker engine. A third `llama-server` instance, started with
//! `--rerank`, serves `/v1/rerank` for the installed reranker model. It
//! re-orders the top search candidates by true query-document relevance,
//! a second stage on top of the fused keyword+vector ranking. Nothing here
//! talks to the network beyond 127.0.0.1.

use crate::app::error::AppError;
use crate::services::inference::sidecar::{free_port, locate_binary, new_api_key, open_log};
use crate::services::models::ModelService;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How many top search results the reranker sees as candidates. Deep enough
/// to fix fusion-order mistakes, shallow enough to stay fast.
pub const RERANK_CANDIDATES: usize = 20;
const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const RERANK_TIMEOUT: Duration = Duration::from_secs(120);
/// Reranker input is one query plus short passage snippets, well inside 4k.
const RERANK_CONTEXT: i32 = 4096;

struct Running {
    child: Child,
    port: u16,
    api_key: String,
    model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankState {
    pub ready: bool,
    pub model_id: Option<String>,
}

/// One candidate's relevance score from the reranker, by input index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankHit {
    pub index: usize,
    pub score: f32,
}

pub struct RerankEngine {
    models: std::sync::Arc<ModelService>,
    running: Mutex<Option<Running>>,
    client: reqwest::blocking::Client,
}

impl Drop for RerankEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

impl RerankEngine {
    pub fn new(models: std::sync::Arc<ModelService>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(RERANK_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            models,
            running: Mutex::new(None),
            client,
        }
    }

    /// Kill the running reranker server, if any, and wait for it to exit.
    pub fn stop(&self) {
        let mut guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut r) = guard.take() {
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    /// Snapshot of the engine state for the UI and status commands.
    pub fn state(&self) -> RerankState {
        let guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
        RerankState {
            ready: guard.is_some(),
            model_id: guard.as_ref().map(|r| r.model_id.clone()),
        }
    }

    /// True when the server is up and ready to rerank.
    pub fn is_running(&self) -> bool {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Start the server on the requested reranker model, or on the smallest
    /// installed one when no id is given.
    pub fn start(&self, preferred: Option<&str>) -> Result<RerankState, AppError> {
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
            .arg(RERANK_CONTEXT.to_string())
            .arg("--rerank")
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

    /// Smallest installed reranker model, or the requested one, by RAM
    /// estimate. Community entries resolve through `all_entries` too.
    fn resolve_model(&self, preferred: Option<&str>) -> Result<(String, PathBuf), AppError> {
        let mut candidates: Vec<crate::services::models::catalog::CatalogEntry> = self
            .models
            .all_entries()
            .into_iter()
            .filter(|e| e.kind == "reranker")
            .collect();
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
            "RERANK_NO_MODEL",
            "No reranker model is installed. Download one from the Models screen first.",
        ))
    }

    /// Score each document against the query. Returns hits sorted by score,
    /// descending. One hit per input document, index-preserving.
    pub fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<RerankHit>, AppError> {
        let (port, api_key, model_id) = {
            let guard = self.running.lock().unwrap_or_else(|e| e.into_inner());
            let r = guard.as_ref().ok_or_else(|| {
                AppError::new("RERANK_NOT_READY", "The reranker engine is not running")
            })?;
            (r.port, r.api_key.clone(), r.model_id.clone())
        };

        let response = self
            .client
            .post(format!("http://127.0.0.1:{port}/v1/rerank"))
            .bearer_auth(&api_key)
            .json(&serde_json::json!({
                "model": model_id,
                "query": query,
                "documents": documents
            }))
            .send()
            .map_err(|e| AppError::internal(format!("Rerank request failed: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::internal(format!(
                "Rerank endpoint returned HTTP {status}"
            )));
        }
        let body = response
            .text()
            .map_err(|e| AppError::internal(format!("Rerank response was unreadable: {e}")))?;
        let mut hits = parse_rerank_response(&body)?;
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(hits)
    }
}

/// Parse a `/v1/rerank` response body into hits. Out-of-range or duplicate
/// indices are dropped by the caller via [`reorder`].
pub fn parse_rerank_response(body: &str) -> Result<Vec<RerankHit>, AppError> {
    let parsed: RerankResponse = serde_json::from_str(body)
        .map_err(|e| AppError::internal(format!("Rerank response was invalid: {e}")))?;
    Ok(parsed
        .results
        .into_iter()
        .map(|r| RerankHit {
            index: r.index,
            score: r.score,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct RerankResponse {
    results: Vec<RerankResultItem>,
}

#[derive(Debug, Deserialize)]
struct RerankResultItem {
    index: usize,
    score: f32,
}

/// Reorder `docs` by rerank hits (highest score first), keeping any document
/// the reranker did not score in its original position at the tail.
pub fn reorder<T: Clone>(docs: &[T], hits: &[RerankHit]) -> Vec<T> {
    let mut out = Vec::with_capacity(docs.len());
    let mut seen = vec![false; docs.len()];
    for hit in hits {
        if hit.index < docs.len() && !seen[hit.index] {
            seen[hit.index] = true;
            out.push(docs[hit.index].clone());
        }
    }
    for (i, doc) in docs.iter().enumerate() {
        if !seen[i] {
            out.push(doc.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rerank_response() {
        let hits = parse_rerank_response(
            r#"{"results":[{"index":2,"score":0.94},{"index":0,"score":0.51}]}"#,
        )
        .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].index, 2);
        assert_eq!(hits[0].score, 0.94);
        assert_eq!(hits[1].index, 0);
    }

    #[test]
    fn rejects_invalid_rerank_response() {
        assert!(parse_rerank_response("not json").is_err());
        assert!(parse_rerank_response(r#"{"results":[{"index":"x","score":1.0}]}"#).is_err());
    }

    #[test]
    fn reorder_puts_scored_first_and_keeps_unscored_order() {
        let docs = vec!["a", "b", "c", "d"];
        let hits = vec![
            RerankHit { index: 3, score: 0.9 },
            RerankHit { index: 0, score: 0.5 },
        ];
        let out = reorder(&docs, &hits);
        assert_eq!(out, vec!["d", "a", "b", "c"]);
    }

    #[test]
    fn reorder_drops_out_of_range_and_duplicate_indices() {
        let docs = vec!["a", "b"];
        let hits = vec![
            RerankHit { index: 5, score: 1.0 },
            RerankHit { index: 1, score: 0.8 },
            RerankHit { index: 1, score: 0.7 },
        ];
        let out = reorder(&docs, &hits);
        assert_eq!(out, vec!["b", "a"]);
    }

    #[test]
    fn reorder_with_no_hits_is_identity() {
        let docs = vec!["a", "b"];
        assert_eq!(reorder(&docs, &[]), docs);
    }
}
