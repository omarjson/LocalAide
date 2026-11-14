pub mod catalog;
pub mod custom;
pub mod download;
pub mod hf;
pub mod refresh;
pub mod verify;

use crate::app::error::AppError;
use catalog::{CatalogEntry, CatalogManifest};
use custom::CustomModels;
use download::{DownloadProgress, ModelDownloader};
use refresh::{CachedCatalog, CatalogRefresher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ModelService {
    models_dir: PathBuf,
    cache_path: PathBuf,
    /// The active catalog behind a lock so an explicit refresh can swap it
    /// at runtime; readers take an `Arc` snapshot.
    manifest: RwLock<Arc<CatalogManifest>>,
    /// Unix timestamp of the fetch that produced the active catalog, when it
    /// came from a refresh rather than the embedded list.
    fetched_at: RwLock<Option<u64>>,
    /// Community models added from the Hugging Face hub (ADR-0010). They are
    /// catalog-shaped and resolve through the same accessors as the manifest.
    custom: CustomModels,
    refresher: CatalogRefresher,
    downloader: ModelDownloader,
}

impl ModelService {
    pub fn new(models_dir: PathBuf, cache_path: PathBuf, custom_path: PathBuf) -> Result<Self, AppError> {
        let builtin = CatalogManifest::default_manifest();
        builtin.verify()?;

        // A previously refreshed catalog wins over the embedded list as long
        // as it still validates; a stale one (older than the version shipped
        // with this build) or a corrupt one falls back to the embedded list.
        let mut manifest = Arc::new(builtin);
        let mut fetched_at = None;
        match Self::load_cached_catalog(&cache_path, manifest.version) {
            Ok(Some(cached)) => {
                log::info!(
                    "Using refreshed model catalog v{} (fetched {ts})",
                    cached.manifest.version,
                    ts = cached.fetched_at
                );
                manifest = Arc::new(cached.manifest);
                fetched_at = Some(cached.fetched_at);
            }
            Ok(None) => {}
            Err(e) => log::warn!("Ignoring cached model catalog: {e}"),
        }

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
            cache_path,
            manifest: RwLock::new(manifest),
            fetched_at: RwLock::new(fetched_at),
            custom: CustomModels::load(custom_path),
            refresher: CatalogRefresher::new(),
            downloader: ModelDownloader::new(),
        })
    }

    /// A cache is valid only if it parses, its integrity signature matches,
    /// and its version is not older than the embedded one.
    fn load_cached_catalog(
        path: &Path,
        min_version: u32,
    ) -> Result<Option<CachedCatalog>, AppError> {
        let data = match std::fs::read_to_string(path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(AppError::storage(format!(
                    "Could not read catalog cache: {e}"
                )));
            }
        };
        if data.len() > catalog::MAX_MANIFEST_BYTES {
            return Err(AppError::invalid_input(
                "Catalog cache exceeds the size limit",
            ));
        }
        let cached: CachedCatalog = serde_json::from_str(&data).map_err(|e| {
            AppError::invalid_input(format!("Catalog cache is not a valid manifest: {e}"))
        })?;
        cached.manifest.verify()?;
        cached.manifest.validate(min_version)?;
        Ok(Some(cached))
    }

    /// Snapshot of the active catalog.
    pub fn manifest(&self) -> Arc<CatalogManifest> {
        self.manifest.read().expect("catalog lock poisoned").clone()
    }

    /// Snapshot of the active catalog (the entries live in `manifest.models`).
    pub fn list_catalog(&self) -> Arc<CatalogManifest> {
        self.manifest()
    }

    pub fn fetched_at(&self) -> Option<u64> {
        *self.fetched_at.read().expect("catalog lock poisoned")
    }

    pub fn get_model_entry(&self, id: &str) -> Option<CatalogEntry> {
        self.manifest()
            .models
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .or_else(|| self.custom.get(id))
    }

    /// Every model the app knows about: the active catalog plus the
    /// user's community additions.
    pub fn all_entries(&self) -> Vec<CatalogEntry> {
        let mut all = self.manifest().models.as_ref().clone();
        all.extend(self.custom.all().as_ref().clone());
        all
    }

    pub fn custom_models(&self) -> &CustomModels {
        &self.custom
    }

    pub fn model_file_path(&self, id: &str) -> PathBuf {
        let name = self
            .get_model_entry(id)
            .and_then(|e| e.dest)
            .unwrap_or_else(|| format!("{id}.gguf"));
        self.models_dir.join(name)
    }

    /// Folder where model and companion files are stored.
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Context size the engine should allocate for a model: the user's
    /// setting capped by the model's native context hint, when one exists.
    pub fn effective_context(&self, id: &str, setting: u32) -> i32 {
        let capped = setting.min(
            self.get_model_entry(id)
                .and_then(|e| e.context_tokens)
                .unwrap_or(u32::MAX),
        );
        capped as i32
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.model_file_path(id).is_file()
    }

    /// Fetch the manifest from the project repository, validate it against
    /// the currently active catalog, persist it, and swap it in.
    pub async fn refresh_catalog(&self) -> Result<Arc<CatalogManifest>, AppError> {
        let fetched = self.refresher.fetch().await?;
        let current = self.manifest();
        fetched.validate(current.version)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Persist first, then swap: a failed cache write leaves the active
        // catalog unchanged instead of promising a refresh that would not
        // survive a restart.
        self.write_cache(&fetched, now)?;
        *self.manifest.write().expect("catalog lock poisoned") = Arc::new(fetched);
        *self.fetched_at.write().expect("catalog lock poisoned") = Some(now);
        Ok(self.manifest())
    }

    /// Delete the cache and fall back to the list compiled into the app.
    pub fn reset_catalog(&self) -> Result<Arc<CatalogManifest>, AppError> {
        if let Err(e) = std::fs::remove_file(&self.cache_path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(AppError::storage(format!(
                "Could not delete catalog cache: {e}"
            )));
        }
        let builtin = CatalogManifest::default_manifest();
        builtin.verify()?;
        *self.manifest.write().expect("catalog lock poisoned") = Arc::new(builtin);
        *self.fetched_at.write().expect("catalog lock poisoned") = None;
        Ok(self.manifest())
    }

    fn write_cache(&self, manifest: &CatalogManifest, fetched_at: u64) -> Result<(), AppError> {
        let payload = CachedCatalog {
            manifest: manifest.clone(),
            fetched_at,
        };
        let data = serde_json::to_string_pretty(&payload)
            .map_err(|e| AppError::storage(format!("Could not serialize catalog cache: {e}")))?;
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::storage(format!("Failed to create catalog cache directory: {e}"))
            })?;
        }
        // Atomic write: temp file in the same directory, then rename over the
        // old cache so a crash cannot leave a half-written catalog behind.
        let tmp = self.cache_path.with_extension("tmp");
        std::fs::write(&tmp, data)
            .map_err(|e| AppError::storage(format!("Could not write catalog cache: {e}")))?;
        std::fs::rename(&tmp, &self.cache_path)
            .map_err(|e| AppError::storage(format!("Could not save catalog cache: {e}")))?;
        Ok(())
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

        // The primary file plus every companion (such as a vision
        // projector) downloads here; without the companions a vision model
        // would be unusable even though it is marked installed.
        let mut files: Vec<(String, String, PathBuf)> = vec![(
            entry.url.clone(),
            entry.sha256.clone(),
            self.model_file_path(id),
        )];
        for c in &entry.companions {
            files.push((
                c.url.clone(),
                c.sha256.clone(),
                self.models_dir.join(&c.dest),
            ));
        }

        // Each file owns an equal slice of the overall percent; the
        // per-file progress from the downloader is rescaled into its slice.
        let shared = Arc::new(std::sync::Mutex::new(progress_cb));
        let weight = 100.0 / files.len() as f32;
        let mut primary: Option<PathBuf> = None;
        for (idx, (url, sha, dest)) in files.iter().enumerate() {
            let base = idx as f32 * weight;
            let shared = Arc::clone(&shared);
            let cancel = Arc::clone(&cancel);
            let downloaded = self
                .downloader
                .download_atomic(url, dest, sha, cancel, move |mut p: DownloadProgress| {
                    p.percent = base + p.percent * weight / 100.0;
                    let mut cb = shared.lock().expect("download progress lock poisoned");
                    cb(p);
                })
                .await?;
            if idx == 0 {
                primary = Some(downloaded);
            }
        }
        Ok(primary.expect("the primary file is always first"))
    }
}
