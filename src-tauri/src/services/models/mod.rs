pub mod catalog;
pub mod download;
pub mod verify;

use crate::app::error::AppError;
use catalog::{CatalogEntry, CatalogManifest};
use download::{DownloadProgress, ModelDownloader};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct ModelService {
    models_dir: PathBuf,
    manifest: CatalogManifest,
    downloader: ModelDownloader,
}

impl ModelService {
    pub fn new(models_dir: PathBuf) -> Result<Self, AppError> {
        let manifest = CatalogManifest::default_manifest();
        manifest.verify()?;

        if !models_dir.exists() {
            std::fs::create_dir_all(&models_dir).map_err(|e| {
                AppError::storage(format!(
                    "Failed to create models directory at {}: {e}",
                    models_dir.display()
                ))
            })?;
        }

        Ok(Self {
            models_dir,
            manifest,
            downloader: ModelDownloader::new(),
        })
    }

    pub fn manifest(&self) -> &CatalogManifest {
        &self.manifest
    }

    pub fn list_catalog(&self) -> &[CatalogEntry] {
        &self.manifest.models
    }

    pub fn get_model_entry(&self, id: &str) -> Option<&CatalogEntry> {
        self.manifest.models.iter().find(|m| m.id == id)
    }

    pub fn model_file_path(&self, id: &str) -> PathBuf {
        self.models_dir.join(format!("{id}.gguf"))
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.model_file_path(id).is_file()
    }

    pub async fn download_model<F>(
        &self,
        id: &str,
        cancel: Arc<AtomicBool>,
        progress_cb: F,
    ) -> Result<PathBuf, AppError>
    where
        F: FnMut(DownloadProgress) + Send + 'static,
    {
        let entry = self
            .get_model_entry(id)
            .ok_or_else(|| AppError::invalid_input(format!("Model '{id}' not found in catalog")))?;

        let dest = self.model_file_path(id);
        self.downloader
            .download_atomic(&entry.url, &dest, &entry.sha256, cancel, progress_cb)
            .await
    }
}
