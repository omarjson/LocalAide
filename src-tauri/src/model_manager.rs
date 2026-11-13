use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::models_dir;
use crate::services::models::catalog::{CatalogEntry, CatalogManifest};
use crate::services::models::verify::compute_sha256_streaming;
use tauri::Emitter;

/// A downloadable companion file (config, tokenizer, sidecar json…).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub url: String,
    pub sha256: Option<String>,
    /// File name relative to the model directory.
    pub dest: String,
}

/// A model entry from the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub kind: ModelKind,
    pub params: String,
    pub file_size: u64,
    pub ram_estimate_gb: f32,
    pub url: String,
    pub sha256: Option<String>,
    pub quant: Option<String>,
    pub installed: bool,
    pub active: bool,
    /// Companion files downloaded alongside the primary file.
    #[serde(default)]
    pub files: Vec<ModelFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
pub enum ModelKind {
    LLM,
    Embedding,
    STT,
    TTS,
    Vision,
}

/// Preset selection computed by the Simple mode wizard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub fits_ram_gb: String,
    pub models: Vec<String>, // model ids
    pub total_ram_gb: f32,
}

impl Preset {
    pub fn tiny() -> Self {
        Self {
            id: "tiny".into(),
            name: "Tiny".into(),
            fits_ram_gb: "4 GB+".into(),
            total_ram_gb: 1.0,
            models: vec!["qwen3.5-0.8b-q4".into()],
        }
    }
    pub fn small() -> Self {
        Self {
            id: "small".into(),
            name: "Small".into(),
            fits_ram_gb: "8 GB+".into(),
            total_ram_gb: 4.0,
            models: vec!["qwen3.5-2b-q4".into(), "gemma4-e2b-qat-q4".into()],
        }
    }
    pub fn balanced() -> Self {
        Self {
            id: "balanced".into(),
            name: "Balanced".into(),
            fits_ram_gb: "16 GB+".into(),
            total_ram_gb: 9.5,
            models: vec!["qwen3.5-9b-q4".into(), "gemma4-e4b-qat-q4".into()],
        }
    }
    pub fn advanced() -> Self {
        Self {
            id: "advanced".into(),
            name: "Advanced".into(),
            fits_ram_gb: "32 GB".into(),
            total_ram_gb: 20.0,
            models: vec!["gemma4-26b-a4b-q4".into(), "qwen3.8-27b-q4".into()],
        }
    }
}

/// Built-in catalog. The single source is `services::models::catalog`.
pub fn default_catalog() -> Vec<ModelEntry> {
    CatalogManifest::default_manifest()
        .models
        .into_iter()
        .map(catalog_entry_to_model_entry)
        .collect()
}

fn catalog_entry_to_model_entry(e: CatalogEntry) -> ModelEntry {
    let kind = match e.kind.as_str() {
        "embedding" | "reranker" => ModelKind::Embedding,
        _ => ModelKind::LLM,
    };
    ModelEntry {
        id: e.id,
        name: e.name,
        kind,
        params: e.params,
        file_size: e.file_size,
        ram_estimate_gb: e.ram_estimate_gb,
        url: e.url,
        sha256: Some(e.sha256),
        quant: Some(e.quant),
        installed: false,
        active: false,
        files: e
            .companions
            .into_iter()
            .map(|c| ModelFile {
                url: c.url,
                sha256: Some(c.sha256),
                dest: c.dest,
            })
            .collect(),
    }
}

/// Download progress payload emitted via Tauri event.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub percent: f32,
}

/// Installed-model state persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledState {
    pub active_model: Option<String>,
    pub installed: HashMap<String, InstalledModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModel {
    pub id: String,
    pub path: String,
    pub sha256: Option<String>,
    pub installed_at: String,
}

impl InstalledState {
    pub fn new() -> Self {
        Self {
            active_model: None,
            installed: HashMap::new(),
        }
    }
}

/// Model manager: catalog, install/download with resume, verify SHA256, activate/remove.
pub struct ModelManager {
    models_dir: PathBuf,
    catalog: Vec<ModelEntry>,
    /// Interior mutability so Tauri `State` (shared ref) can drive installs.
    state: Mutex<InstalledState>,
    /// Cancellation flags for in-flight downloads keyed by model id.
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ModelManager {
    pub fn new() -> Result<Self, String> {
        let dir = models_dir();
        Self::with_dir(dir)
    }

    pub fn with_dir(dir: PathBuf) -> Result<Self, String> {
        let _ = fs::create_dir_all(&dir);
        let catalog = default_catalog();
        let state = Self::load_state(&dir).unwrap_or_else(|e| {
            log::warn!(
                "Could not load model state from {}, initializing empty: {e}",
                dir.display()
            );
            InstalledState::new()
        });
        Ok(Self {
            models_dir: dir,
            catalog,
            state: Mutex::new(state),
            cancels: Mutex::new(HashMap::new()),
        })
    }

    fn state_path(dir: &Path) -> PathBuf {
        dir.join("state.json")
    }

    fn load_state(dir: &Path) -> Result<InstalledState, String> {
        let path = Self::state_path(dir);
        if !path.exists() {
            return Ok(InstalledState::new());
        }
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    fn save_state(&self) -> Result<(), String> {
        let path = Self::state_path(&self.models_dir);
        let state_guard = self
            .state
            .lock()
            .map_err(|e| format!("state mutex poisoned: {e}"))?;
        let data = serde_json::to_string_pretty(&*state_guard).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }

    pub(crate) fn model_path(&self, id: &str) -> PathBuf {
        self.models_dir.join(format!("{id}.gguf"))
    }

    /// Directory that holds model files. All models share the flat models folder.
    pub(crate) fn model_dir(&self, _id: &str) -> PathBuf {
        self.models_dir.clone()
    }

    /// Resolve the destination path for a model's primary file.
    pub(crate) fn primary_path(&self, id: &str) -> PathBuf {
        self.model_path(id)
    }

    pub(crate) fn companion_path(&self, _id: &str, f: &ModelFile) -> PathBuf {
        self.models_dir.join(&f.dest)
    }

    /// List catalog entries with installed/active flags merged from persisted state.
    pub fn list_models(&self) -> Vec<ModelEntry> {
        let state = match self.state.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!("state mutex poisoned in list_models: {e}");
                return vec![];
            }
        };
        self.catalog
            .iter()
            .map(|m| {
                let installed = state.installed.contains_key(&m.id);
                let active = state.active_model.as_deref() == Some(m.id.as_str());
                ModelEntry {
                    installed,
                    active,
                    ..m.clone()
                }
            })
            .collect()
    }

    /// Return the currently active model entry (installed and activated).
    pub fn active_model(&self) -> Option<ModelEntry> {
        self.list_models().into_iter().find(|m| m.active)
    }

    /// Mark a model active. Deactivates any previously active model.
    pub fn activate(&self, id: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| format!("state mutex poisoned: {e}"))?;
        if !state.installed.contains_key(id) {
            return Err(format!("model {id} not installed"));
        }
        state.active_model = Some(id.to_string());
        drop(state);
        self.save_state()?;
        Ok(())
    }

    /// Remove an installed model file and its state entry.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| format!("state mutex poisoned: {e}"))?;
        if state.active_model.as_deref() == Some(id) {
            state.active_model = None;
        }
        state.installed.remove(id);
        drop(state);
        let path = self.model_path(id);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        // Multi-file models: remove the whole directory.
        let dir = self.models_dir.join(id);
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        self.save_state()?;
        Ok(())
    }

    /// Check whether a model file is already present on disk.
    pub fn is_installed(&self, id: &str) -> bool {
        match self.state.lock() {
            Ok(state) => state.installed.contains_key(id),
            Err(e) => {
                log::error!("state mutex poisoned in is_installed: {e}");
                false
            }
        }
    }

    /// Compute local file size of an in-progress download for resume support.
    pub fn existing_size(&self, id: &str) -> u64 {
        let path = self.model_path(id);
        if path.exists() {
            return path.metadata().map(|m| m.len()).unwrap_or(0);
        }
        // Multi-file layout: report size of the primary file.
        let p = self.primary_path(id);
        p.metadata().map(|m| m.len()).unwrap_or(0)
    }

    /// Request cancellation of an in-flight download.
    pub fn cancel_download(&self, id: &str) {
        let cancels = match self.cancels.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!("cancels mutex poisoned in cancel_download: {e}");
                return;
            }
        };
        if let Some(flag) = cancels.get(id) {
            flag.store(true, Ordering::SeqCst);
        }
    }

    /// Whether a download is currently running for this model.
    pub fn is_downloading(&self, id: &str) -> bool {
        match self.cancels.lock() {
            Ok(cancels) => cancels.contains_key(id),
            Err(e) => {
                log::error!("cancels mutex poisoned in is_downloading: {e}");
                false
            }
        }
    }

    fn entry(&self, id: &str) -> Result<ModelEntry, String> {
        self.catalog
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| format!("model {id} not in catalog"))
    }

    /// Download (and install) a model with streaming progress, resume, cancel
    /// and SHA256 verification. Progress is emitted as `model_download_progress`.
    pub async fn download(&self, app: tauri::AppHandle, id: String) -> Result<(), String> {
        let entry = self.entry(&id)?;

        // Mark in-flight and install a fresh cancellation flag.
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut cancels = self
                .cancels
                .lock()
                .map_err(|e| format!("cancels mutex poisoned: {e}"))?;
            cancels.insert(id.clone(), cancel.clone());
        }

        let result = self.download_inner(&app, &entry, &cancel).await;

        {
            let mut cancels = self
                .cancels
                .lock()
                .map_err(|e| format!("cancels mutex poisoned: {e}"))?;
            cancels.remove(&id);
        }

        match result {
            Ok(digest) => {
                let started = now_iso();
                {
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|e| format!("state mutex poisoned: {e}"))?;
                    state.installed.insert(
                        id.clone(),
                        InstalledModel {
                            id: id.clone(),
                            path: self.primary_path(&id).to_string_lossy().to_string(),
                            sha256: Some(digest),
                            installed_at: started,
                        },
                    );
                }
                self.save_state()?;
                let _ = app.emit(
                    "model_download_done",
                    serde_json::json!({ "id": id, "ok": true }),
                );
                Ok(())
            }
            Err(e) => {
                let _ = app.emit(
                    "model_download_done",
                    serde_json::json!({ "id": id, "ok": false, "error": e }),
                );
                Err(e)
            }
        }
    }

    async fn download_inner(
        &self,
        app: &tauri::AppHandle,
        entry: &ModelEntry,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60 * 60))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        // (url, expected sha256, destination path)
        let mut jobs: Vec<(String, Option<String>, PathBuf)> = vec![(
            entry.url.clone(),
            entry.sha256.clone(),
            self.primary_path(&entry.id),
        )];
        for f in &entry.files {
            jobs.push((
                f.url.clone(),
                f.sha256.clone(),
                self.companion_path(&entry.id, f),
            ));
        }

        let mut final_digest = String::new();
        for (url, expected, dest) in jobs {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            // Resume: keep whatever bytes we already have on disk and request the
            // remainder with a Range header.
            let mut offset = dest.metadata().map(|m| m.len()).unwrap_or(0);

            let mut attempt = client.get(&url);
            if offset > 0 {
                attempt = attempt.header("Range", format!("bytes={offset}-"));
            }

            let mut res = attempt.send().await.map_err(|e| e.to_string())?;
            let status = res.status().as_u16();
            if !(200..300).contains(&status) {
                if status == 416 || status == 200 {
                    // 416: nothing left to fetch. 200: server ignored Range, so we
                    // restart the file from scratch.
                    if status == 416 {
                        let _ = fs::remove_file(&dest);
                    }
                    offset = 0;
                    res = client.get(&url).send().await.map_err(|e| e.to_string())?;
                    if !(200..300).contains(&res.status().as_u16()) {
                        return Err(format!("download failed: HTTP {}", res.status()));
                    }
                } else {
                    return Err(format!("download failed: HTTP {status}"));
                }
            } else if offset > 0 && status == 200 {
                offset = 0;
            }

            final_digest = self
                .write_stream(app, entry, res, &dest, offset, expected, cancel)
                .await?;
        }

        Ok(final_digest)
    }

    #[allow(clippy::too_many_arguments)]
    async fn write_stream(
        &self,
        app: &tauri::AppHandle,
        entry: &ModelEntry,
        res: reqwest::Response,
        dest: &Path,
        offset: u64,
        expected: Option<String>,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        use futures::StreamExt;
        use std::io::{Seek, Write};

        let file_name = dest
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.id.clone());
        let total = res.content_length().map(|l| l + offset);

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dest)
            .map_err(|e| e.to_string())?;
        file.seek(std::io::SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;

        let mut downloaded = offset;
        let mut stream = res.bytes_stream();
        let mut last_emit = std::time::Instant::now();

        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                drop(file);
                let _ = fs::remove_file(dest);
                return Err("download cancelled".into());
            }
            let chunk = chunk.map_err(|e| e.to_string())?;
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;

            if last_emit.elapsed() >= Duration::from_millis(100) {
                let percent = match total {
                    Some(t) if t > 0 => (downloaded as f32 / t as f32) * 100.0,
                    _ => 0.0,
                };
                let _ = app.emit(
                    "model_download_progress",
                    serde_json::json!({
                        "id": entry.id,
                        "file": file_name,
                        "bytes_downloaded": downloaded,
                        "bytes_total": total.unwrap_or(0),
                        "percent": percent,
                    }),
                );
                last_emit = std::time::Instant::now();
            }
        }

        file.flush().map_err(|e| e.to_string())?;
        drop(file);

        // Hash the finished file in streamed chunks so large models never load into RAM.
        let digest = compute_sha256_streaming(dest).map_err(|e| e.to_string())?;
        if let Some(expected) = expected {
            let expected = expected.trim();
            if !expected.eq_ignore_ascii_case(&digest) {
                let _ = fs::remove_file(dest);
                return Err(format!(
                    "SHA256 mismatch for {file_name}: expected {expected}, got {digest}"
                ));
            }
        }

        Ok(digest)
    }
}

/// Minimal ISO-8601 UTC timestamp (no chrono dependency).
fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}
