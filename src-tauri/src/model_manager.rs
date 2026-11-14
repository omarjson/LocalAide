use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::models_dir;
use crate::services::models::catalog::{CatalogEntry, CatalogManifest};

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
            total_ram_gb: 1.8,
            models: vec![
                "qwen3.5-0.8b-q4".into(),
                "embeddinggemma-300m-qat-q4".into(),
            ],
        }
    }
    pub fn small() -> Self {
        Self {
            id: "small".into(),
            name: "Small".into(),
            fits_ram_gb: "8 GB+".into(),
            total_ram_gb: 6.8,
            models: vec![
                "qwen3.5-2b-q4".into(),
                "gemma4-e2b-qat-q4".into(),
                "embeddinggemma-300m-qat-q4".into(),
            ],
        }
    }
    pub fn balanced() -> Self {
        Self {
            id: "balanced".into(),
            name: "Balanced".into(),
            fits_ram_gb: "16 GB+".into(),
            total_ram_gb: 14.5,
            models: vec![
                "qwen3.5-9b-q4".into(),
                "gemma4-e4b-qat-q4".into(),
                "embeddinggemma-300m-q8".into(),
            ],
        }
    }
    pub fn advanced() -> Self {
        Self {
            id: "advanced".into(),
            name: "Advanced".into(),
            fits_ram_gb: "32 GB".into(),
            total_ram_gb: 21.2,
            models: vec![
                "gemma4-26b-a4b-q4".into(),
                "qwen3.8-27b-q4".into(),
                "bge-m3-560m-q8".into(),
            ],
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

impl Default for InstalledState {
    fn default() -> Self {
        Self::new()
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

    /// Resolve the destination path for a model's primary file.
    pub(crate) fn primary_path(&self, id: &str) -> PathBuf {
        self.model_path(id)
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
}
