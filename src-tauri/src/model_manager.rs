use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::models_dir;
use crate::services::models::catalog::is_safe_model_id;

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
                "nomic-embed-v2-moe-q8".into(),
            ],
        }
    }
}

/// Model state persisted to `models/state.json`. Older files also hold an
/// `installed` map that nothing ever filled; serde skips it when reading.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstalledState {
    pub active_model: Option<String>,
}

impl InstalledState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Remembers which model is active and removes model files. Downloads and the
/// installed check live in `services::models` (`ModelService`).
pub struct ModelManager {
    models_dir: PathBuf,
    /// Interior mutability so Tauri `State` (shared ref) can change it.
    state: Mutex<InstalledState>,
}

impl ModelManager {
    pub fn new() -> Result<Self, String> {
        let dir = models_dir();
        Self::with_dir(dir)
    }

    pub fn with_dir(dir: PathBuf) -> Result<Self, String> {
        let _ = fs::create_dir_all(&dir);
        let state = Self::load_state(&dir).unwrap_or_else(|e| {
            log::warn!(
                "Could not load model state from {}, initializing empty: {e}",
                dir.display()
            );
            InstalledState::new()
        });
        Ok(Self {
            models_dir: dir,
            state: Mutex::new(state),
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

    fn model_path(&self, id: &str) -> PathBuf {
        self.models_dir.join(format!("{id}.gguf"))
    }

    /// Id of the model the user last loaded, or `None`. Unlike a catalog
    /// lookup, this keeps community models that the catalog does not list.
    pub fn active_model_id(&self) -> Option<String> {
        match self.state.lock() {
            Ok(state) => state.active_model.clone(),
            Err(e) => {
                log::error!("state mutex poisoned in active_model_id: {e}");
                None
            }
        }
    }

    /// Forget the active model. The caller stops the engine.
    pub fn clear_active(&self) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| format!("state mutex poisoned: {e}"))?;
        state.active_model = None;
        drop(state);
        self.save_state()
    }

    /// Record a model as the active one. The caller checks that the file is on
    /// disk first. This only saves the choice.
    pub fn activate(&self, id: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| format!("state mutex poisoned: {e}"))?;
        state.active_model = Some(id.to_string());
        drop(state);
        self.save_state()?;
        Ok(())
    }

    /// Remove an installed model file and forget it if it was active.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        // The id becomes a folder path that is deleted recursively below. An id
        // of `..` would delete the whole app data folder, so reject it first.
        if !is_safe_model_id(id) {
            return Err(format!("invalid model id: {id}"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|e| format!("state mutex poisoned: {e}"))?;
        if state.active_model.as_deref() == Some(id) {
            state.active_model = None;
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_refuses_ids_that_leave_the_models_folder() {
        let root = std::env::temp_dir().join(format!("la-model-remove-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();
        fs::write(root.join("keep.txt"), "data").unwrap();

        let manager = ModelManager::with_dir(models).unwrap();
        assert!(manager.remove("..").is_err());
        assert!(manager.remove("../").is_err());
        assert!(root.join("keep.txt").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn activate_records_the_choice_and_clear_active_forgets_it() {
        let root = std::env::temp_dir().join(format!("la-model-active-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let manager = ModelManager::with_dir(root.join("models")).unwrap();
        assert!(manager.active_model_id().is_none());
        manager.activate("community-model").unwrap();
        assert_eq!(
            manager.active_model_id().as_deref(),
            Some("community-model")
        );
        manager.clear_active().unwrap();
        assert!(manager.active_model_id().is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_older_state_file_still_loads() {
        let root = std::env::temp_dir().join(format!("la-model-state-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();
        fs::write(
            models.join("state.json"),
            r#"{"active_model":"qwen3.5-2b-q4","installed":{}}"#,
        )
        .unwrap();

        let manager = ModelManager::with_dir(models).unwrap();
        assert_eq!(manager.active_model_id().as_deref(), Some("qwen3.5-2b-q4"));

        let _ = fs::remove_dir_all(&root);
    }
}
