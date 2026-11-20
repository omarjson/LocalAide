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
    /// Models being downloaded now. A second download of the same model
    /// would write the same `.part` file from two places at once.
    in_flight: std::sync::Mutex<std::collections::HashSet<String>>,
}

/// Removes a model from `in_flight` when its download ends, however it ends.
struct InFlight<'a> {
    set: &'a std::sync::Mutex<std::collections::HashSet<String>>,
    id: String,
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.set
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

impl ModelService {
    pub fn new(
        models_dir: PathBuf,
        cache_path: PathBuf,
        custom_path: PathBuf,
    ) -> Result<Self, AppError> {
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
            in_flight: std::sync::Mutex::new(std::collections::HashSet::new()),
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
        self.manifest
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn fetched_at(&self) -> Option<u64> {
        *self.fetched_at.read().unwrap_or_else(|e| e.into_inner())
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
        let mut all = self.manifest().models.clone();
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

    /// True for models that can hold a chat. Embedding and reranker models
    /// only score text, so they never serve as the chat engine.
    pub fn is_chat_model(&self, id: &str) -> bool {
        self.get_model_entry(id)
            .is_some_and(|e| matches!(e.kind.as_str(), "chat" | "code"))
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
        *self.manifest.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(fetched);
        *self.fetched_at.write().unwrap_or_else(|e| e.into_inner()) = Some(now);
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
        *self.manifest.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(builtin);
        *self.fetched_at.write().unwrap_or_else(|e| e.into_inner()) = None;
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

    /// Every file a model needs: the main file, then its companions.
    fn model_files(&self, entry: &CatalogEntry) -> Vec<(String, String, u64, PathBuf)> {
        let mut files = vec![(
            entry.url.clone(),
            entry.sha256.clone(),
            entry.file_size,
            self.model_file_path(&entry.id),
        )];
        for c in &entry.companions {
            files.push((
                c.url.clone(),
                c.sha256.clone(),
                c.file_size,
                self.models_dir.join(&c.dest),
            ));
        }
        files
    }

    /// (bytes on disk so far, total bytes) over the model and its companions.
    /// Finished files count in full, unfinished ones by their `.part` file.
    pub fn download_state(&self, id: &str) -> (u64, u64) {
        let Some(entry) = self.get_model_entry(id) else {
            return (0, 0);
        };
        let mut have = 0;
        let mut total = 0;
        for (_, _, size, dest) in self.model_files(&entry) {
            total += size;
            have += if dest.is_file() {
                size
            } else {
                std::fs::metadata(dest.with_extension("part"))
                    .map(|m| m.len().min(size))
                    .unwrap_or(0)
            };
        }
        (have, total)
    }

    /// Delete the model's partial files (Cancel). Finished files stay.
    pub fn remove_partials(&self, id: &str) {
        let Some(entry) = self.get_model_entry(id) else {
            return;
        };
        for (_, _, _, dest) in self.model_files(&entry) {
            let _ = std::fs::remove_file(dest.with_extension("part"));
        }
    }

    /// True while this model is downloading.
    pub fn is_downloading(&self, id: &str) -> bool {
        self.in_flight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
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

        // One download per model: a second one would write the same
        // `.part` file at the same time and fail its checksum.
        {
            let mut set = self.in_flight.lock().unwrap_or_else(|e| e.into_inner());
            if !set.insert(id.to_string()) {
                return Err(AppError::new(
                    "DOWNLOAD_BUSY",
                    "This model is still downloading or still stopping. Wait a moment and try again.",
                ));
            }
        }
        let _guard = InFlight {
            set: &self.in_flight,
            id: id.to_string(),
        };

        // The primary file plus every companion (such as a vision
        // projector) downloads here; without the companions a vision model
        // would be unusable even though it is marked installed.
        let files = self.model_files(&entry);
        let total: u64 = files.iter().map(|f| f.2).sum();

        // Progress covers all files: bytes of earlier files are added to the
        // current file's bytes, and the percent is of the whole model.
        let shared = Arc::new(std::sync::Mutex::new(progress_cb));
        let mut done_before = 0u64;
        let mut primary: Option<PathBuf> = None;
        for (idx, (url, sha, size, dest)) in files.iter().enumerate() {
            let shared = Arc::clone(&shared);
            let cancel = Arc::clone(&cancel);
            let before = done_before;
            let downloaded = self
                .downloader
                .download_atomic(
                    url,
                    dest,
                    *size,
                    sha,
                    cancel,
                    move |mut p: DownloadProgress| {
                        p.bytes_downloaded = before + p.bytes_downloaded.min(p.bytes_total);
                        p.bytes_total = total.max(p.bytes_downloaded);
                        p.percent = if total > 0 {
                            (p.bytes_downloaded as f64 / total as f64 * 100.0) as f32
                        } else {
                            0.0
                        };
                        let mut cb = shared.lock().unwrap_or_else(|e| e.into_inner());
                        cb(p);
                    },
                )
                .await?;
            done_before += *size;
            if idx == 0 {
                primary = Some(downloaded);
            }
        }
        Ok(primary.expect("the primary file is always first"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_service(tag: &str) -> (ModelService, PathBuf) {
        // Tagged per test: cargo test runs tests in parallel threads.
        let dir =
            std::env::temp_dir().join(format!("la-modelservice-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let service = ModelService::new(
            dir.join("models"),
            dir.join("catalog-cache.json"),
            dir.join("custom-models.json"),
        )
        .expect("service must initialize");
        (service, dir)
    }

    fn community_entry(id: &str, dest: Option<String>) -> CatalogEntry {
        CatalogEntry {
            id: id.into(),
            name: "Community model".into(),
            params: "community".into(),
            kind: "chat".into(),
            license: "MIT".into(),
            file_size: 1_000,
            ram_estimate_gb: 1.0,
            url: "https://huggingface.co/org/repo/resolve/main/model.gguf".into(),
            sha256: "ab".repeat(32),
            quant: "Q4_K_M".into(),
            description: "test".into(),
            context_tokens: Some(131_072),
            dest,
            companions: vec![],
        }
    }

    #[test]
    fn custom_entries_resolve_through_the_standard_accessors() {
        let (service, dir) = temp_service("resolve");
        // A plain community model: dest falls back to {id}.gguf.
        service
            .custom_models()
            .add(community_entry("hf-org-repo-model", None))
            .unwrap();
        assert!(
            service.get_model_entry("hf-org-repo-model").is_some(),
            "community entries must resolve like catalog entries"
        );
        assert_eq!(
            service.model_file_path("hf-org-repo-model"),
            dir.join("models").join("hf-org-repo-model.gguf")
        );
        assert_eq!(
            service.effective_context("hf-org-repo-model", 8192),
            8192,
            "the entry's native context hint caps the setting"
        );
        assert_eq!(
            service.effective_context("hf-org-repo-model", 999_999),
            131_072
        );

        // A split GGUF keeps its shard-pattern dest for llama.cpp discovery.
        service
            .custom_models()
            .add(community_entry(
                "hf-org-repo-shard",
                Some("model-00001-of-00003.gguf".into()),
            ))
            .unwrap();
        assert_eq!(
            service.model_file_path("hf-org-repo-shard"),
            dir.join("models").join("model-00001-of-00003.gguf")
        );
        assert!(!service.is_installed("hf-org-repo-shard"));

        service.custom_models().remove("hf-org-repo-model").unwrap();
        assert!(service.get_model_entry("hf-org-repo-model").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
