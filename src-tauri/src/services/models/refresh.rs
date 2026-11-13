use super::catalog::{CatalogManifest, MAX_MANIFEST_BYTES};
use crate::app::error::AppError;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Where refreshed catalogs come from: this project's own repository, over
/// HTTPS, at a URL compiled into the app. There is no user-configurable
/// catalog source — the manifest download URLs it contains are separately
/// validated and the model files themselves are pinned by SHA-256 (ADR-0008).
pub const CATALOG_URL: &str =
    "https://raw.githubusercontent.com/omarjson/LocalAide/main/catalog/models.json";

/// A refreshed catalog persisted next to the app config, so it survives
/// restarts. Invalid caches are ignored at startup and fall back to the
/// embedded list.
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedCatalog {
    pub manifest: CatalogManifest,
    pub fetched_at: u64,
}

pub struct CatalogRefresher {
    client: reqwest::Client,
}

impl Default for CatalogRefresher {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogRefresher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// Fetch and parse the manifest, verifying its integrity signature.
    /// Structural validation happens against the currently active catalog
    /// afterwards (anti-rollback needs the active version as the baseline).
    pub async fn fetch(&self) -> Result<CatalogManifest, AppError> {
        let response = self
            .client
            .get(CATALOG_URL)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Catalog fetch failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::internal(format!(
                "Catalog fetch returned HTTP {status} from the repository"
            )));
        }

        // Stream with a hard cap so a server without a content-length header
        // cannot push unbounded data into memory.
        let mut body: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::internal(format!("Catalog stream error: {e}")))?;
            if body.len() + chunk.len() > MAX_MANIFEST_BYTES {
                return Err(AppError::invalid_input(format!(
                    "Catalog exceeds the size limit of {} KB",
                    MAX_MANIFEST_BYTES / 1024
                )));
            }
            body.extend_from_slice(&chunk);
        }

        let text = String::from_utf8(body)
            .map_err(|_| AppError::invalid_input("Catalog body is not valid UTF-8"))?;
        let manifest: CatalogManifest = serde_json::from_str(&text).map_err(|e| {
            AppError::invalid_input(format!("Catalog is not a valid manifest: {e}"))
        })?;
        manifest.verify()?;
        Ok(manifest)
    }
}
