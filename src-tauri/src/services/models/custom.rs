//! Community models the user added from the Hugging Face hub. They reuse the
//! catalog entry shape, so downloads, verification, deletion, and loading all
//! run through the exact same code paths as curated models — the only
//! difference is where the pin comes from: the SHA-256 Hugging Face itself
//! serves for the file (see ADR-0010 for what that does and does not prove).

use crate::app::error::AppError;
use crate::services::models::catalog::CatalogEntry;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Hard cap so a curious user cannot grow the registry unbounded; downloads
/// are capped by disk space anyway.
pub const MAX_CUSTOM_MODELS: usize = 50;
/// The registry file is a small JSON list, not a manifest stream; reject
/// absurd bodies before parsing.
const MAX_CUSTOM_BYTES: usize = 2 * 1024 * 1024;

pub struct CustomModels {
    path: PathBuf,
    entries: RwLock<Arc<Vec<CatalogEntry>>>,
}

impl CustomModels {
    /// Load from disk; a missing file is an empty registry, a corrupt one is
    /// ignored with a warning (fail-open: the curated catalog still works).
    pub fn load(path: PathBuf) -> Self {
        let entries = match Self::read_entries(&path) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Ignoring community-model registry: {e}");
                Vec::new()
            }
        };
        Self {
            path,
            entries: RwLock::new(Arc::new(entries)),
        }
    }

    fn read_entries(path: &Path) -> Result<Vec<CatalogEntry>, AppError> {
        let data = match std::fs::read_to_string(path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(AppError::storage(format!(
                    "Could not read community-model registry: {e}"
                )));
            }
        };
        if data.len() > MAX_CUSTOM_BYTES {
            return Err(AppError::invalid_input(
                "Community-model registry exceeds the size cap",
            ));
        }
        serde_json::from_str(&data)
            .map_err(|e| AppError::invalid_input(format!("Community-model registry is invalid: {e}")))
    }

    fn persist(&self, entries: &[CatalogEntry]) -> Result<(), AppError> {
        let json = serde_json::to_vec(entries)
            .map_err(|e| AppError::storage(format!("Could not serialize community models: {e}")))?;
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, json)
            .map_err(|e| AppError::storage(format!("Could not write community models: {e}")))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| AppError::storage(format!("Could not save community models: {e}")))?;
        Ok(())
    }

    pub fn all(&self) -> Arc<Vec<CatalogEntry>> {
        self.entries
            .read()
            .expect("community-model lock poisoned")
            .clone()
    }

    pub fn get(&self, id: &str) -> Option<CatalogEntry> {
        self.all().iter().find(|e| e.id == id).cloned()
    }

    /// Validate and persist a new community entry. Duplicate ids are rejected
    /// so a re-add always means "remove the old one first".
    pub fn add(&self, entry: CatalogEntry) -> Result<(), AppError> {
        entry.validate_entry()?;
        let mut next: Vec<CatalogEntry> = self.all().as_ref().clone();
        if next.iter().any(|e| e.id == entry.id) {
            return Err(AppError::invalid_input(format!(
                "A community model with id '{}' already exists",
                entry.id
            )));
        }
        if next.len() >= MAX_CUSTOM_MODELS {
            return Err(AppError::invalid_input(format!(
                "The community-model list is full ({MAX_CUSTOM_MODELS}); remove one first"
            )));
        }
        next.push(entry);
        self.persist(&next)?;
        *self.entries.write().expect("community-model lock poisoned") = Arc::new(next);
        Ok(())
    }

    /// Drop an entry from the registry. The downloaded files themselves are
    /// deleted by the `model_remove` command; this only forgets the entry.
    pub fn remove(&self, id: &str) -> Result<(), AppError> {
        let mut next: Vec<CatalogEntry> = self.all().as_ref().clone();
        let before = next.len();
        next.retain(|e| e.id != id);
        if next.len() == before {
            return Err(AppError::invalid_input(format!(
                "'{id}' is not a community model"
            )));
        }
        self.persist(&next)?;
        *self.entries.write().expect("community-model lock poisoned") = Arc::new(next);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.all().len()
    }
}

/// Slugify one path component for a community-model id: lowercase, keep
/// alphanumerics, collapse everything else into single dashes.
pub fn slugify_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Deterministic id for a community model: `hf-{org}-{repo}-{file-stem}`.
pub fn community_id(org: &str, repo: &str, filename: &str) -> String {
    let stem = filename.strip_suffix(".gguf").unwrap_or(filename);
    format!(
        "hf-{}-{}-{}",
        slugify_component(org),
        slugify_component(repo),
        slugify_component(stem)
    )
}

/// Ensure uniqueness against existing ids by appending `-2`, `-3`, …
pub fn unique_id(existing: &[CatalogEntry], mut base: String) -> String {
    let mut n = 1;
    while existing.iter().any(|e| e.id == base) {
        n += 1;
        base = format!("{base}-{n}");
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> CatalogEntry {
        CatalogEntry {
            id: id.into(),
            name: "Community test".into(),
            params: "4B".into(),
            kind: "chat".into(),
            license: "Apache-2.0".into(),
            file_size: 1_000,
            ram_estimate_gb: 1.5,
            url: "https://huggingface.co/org/repo/resolve/main/model.gguf".into(),
            sha256: "aa".repeat(32),
            quant: "Q4_K_M".into(),
            description: "test".into(),
            context_tokens: None,
            dest: None,
            companions: vec![],
        }
    }

    #[test]
    fn persists_and_roundtrips() {
        let dir = std::env::temp_dir().join(format!("la-custom-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom-models.json");

        let first = CustomModels::load(path.clone());
        first.add(sample("hf-org-repo-model")).unwrap();
        assert_eq!(first.len(), 1);

        let second = CustomModels::load(path.clone());
        assert!(second.get("hf-org-repo-model").is_some());
        second.remove("hf-org-repo-model").unwrap();
        assert_eq!(second.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_ids_rejected() {
        let dir = std::env::temp_dir().join(format!("la-custom-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reg = CustomModels::load(dir.join("c.json"));
        reg.add(sample("dup")).unwrap();
        assert!(reg.add(sample("dup")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_entries_rejected() {
        let dir = std::env::temp_dir().join(format!("la-custom-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reg = CustomModels::load(dir.join("c.json"));

        let mut http = sample("http-url");
        http.url = "http://huggingface.co/org/repo/resolve/main/model.gguf".into();
        assert!(reg.add(http).is_err());

        let mut traversal = sample("bad-dest");
        traversal.dest = Some("../escape.gguf".into());
        assert!(reg.add(traversal).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_are_slugged_and_unique() {
        assert_eq!(
            community_id("unsloth", "Qwen3.5-9B-GGUF", "Qwen3.5-9B-Q4_K_M.gguf"),
            "hf-unsloth-qwen3-5-9b-gguf-qwen3-5-9b-q4-k-m"
        );
        let existing = vec![sample("hf-a"), sample("hf-a-2")];
        assert_eq!(unique_id(&existing, "hf-a".into()), "hf-a-3");
        assert_eq!(unique_id(&existing, "hf-b".into()), "hf-b");
        assert_eq!(slugify_component("--Weird__Name--"), "weird-name");
    }

    #[test]
    fn remove_unknown_is_an_error() {
        let dir = std::env::temp_dir().join(format!("la-custom-unk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reg = CustomModels::load(dir.join("c.json"));
        assert!(reg.remove("nope").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
