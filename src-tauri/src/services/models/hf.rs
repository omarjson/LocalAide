//! A thin client for the public Hugging Face API, used by the community
//! model browser (ADR-0010). Read-only: search and file listings only —
//! downloads go through the same verified pipeline as the curated catalog.
//!
//! The base URL is a constructor parameter so tests can point the client at
//! a loopback server; parsing lives in pure functions that are tested
//! without any network.

use crate::app::error::AppError;
use serde::{Deserialize, Serialize};

/// Cap on any single API response body, so a hostile or broken endpoint
/// cannot stream unbounded data into memory.
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_BASE_URL: &str = "https://huggingface.co";
/// Broad but bounded result windows for search and file listings.
pub const DEFAULT_SEARCH_LIMIT: u32 = 20;
pub const MAX_SEARCH_LIMIT: u32 = 50;

pub struct HfClient {
    client: reqwest::Client,
    base_url: String,
}

impl HfClient {
    pub fn new() -> Result<Self, AppError> {
        Self::with_base_url(DEFAULT_BASE_URL.to_string())
    }

    pub fn with_base_url(base_url: String) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::internal(e.to_string()))?;
        Ok(Self { client, base_url })
    }

    /// GET an API path relative to the base URL, capped at MAX_RESPONSE_BYTES.
    async fn get_capped(&self, path: &str) -> Result<String, AppError> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .header("User-Agent", "LocalAide-community-models")
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Hugging Face request failed: {e}")))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN
        {
            return Err(AppError::new(
                "HF_GATED",
                "This repository is gated on Hugging Face and requires a login, which LocalAide does not store. Pick an open repository instead.",
            ));
        }
        if !status.is_success() {
            return Err(AppError::internal(format!(
                "Hugging Face API returned HTTP {status}"
            )));
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| AppError::internal(format!("Hugging Face stream failed: {e}")))?;
            body.extend_from_slice(&chunk);
            if body.len() > MAX_RESPONSE_BYTES {
                return Err(AppError::invalid_input(
                    "Hugging Face API response exceeds the size cap",
                ));
            }
        }
        String::from_utf8(body)
            .map_err(|_| AppError::invalid_input("Hugging Face API returned invalid UTF-8"))
    }

    /// Search public GGUF repositories, most-downloaded first.
    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<HfRepo>, AppError> {
        let limit = limit.clamp(1, MAX_SEARCH_LIMIT);
        let q = urlencode(query);
        let path = format!(
            "/api/models?search={q}&filter=gguf&sort=downloads&direction=-1&limit={limit}"
        );
        let body = self.get_capped(&path).await?;
        parse_repos(&body)
    }

    /// List every GGUF file in a repository's main revision.
    pub async fn repo_files(&self, repo: &str) -> Result<Vec<HfFile>, AppError> {
        validate_repo(repo)?;
        let body = self
            .get_capped(&format!("/api/models/{repo}/tree/main?recursive=true"))
            .await?;
        parse_files(&body)
    }
}

/// Very small percent-encoding for query strings — enough for search terms.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Repository ids are `org/name` with a constrained alphabet; anything else
/// is refused before it can reach the API or a download URL.
pub fn validate_repo(repo: &str) -> Result<(), AppError> {
    let ok = !repo.is_empty()
        && repo.len() <= 120
        && repo.contains('/')
        && repo.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
        })
        && !repo.starts_with('/')
        && !repo.ends_with('/')
        && !repo.contains("..");
    if ok {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "'{repo}' is not a valid Hugging Face repository id (expected org/name)"
        )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfRepo {
    pub id: String,
    pub downloads: u64,
    pub likes: u64,
    pub gated: bool,
    /// License tag on the repo (for example "apache-2.0"), when present.
    pub license: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfFile {
    /// Path inside the repository, for example "Q4_K_M/model.gguf".
    pub path: String,
    pub size: u64,
    /// SHA-256 of the LFS object — the pin every community download is
    /// verified against.
    pub sha256: String,
    /// True for vision projector files (`mmproj`), which download as
    /// companions alongside the chosen GGUF.
    pub is_mmproj: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfFileEntry {
    path: String,
    size: u64,
    #[serde(default)]
    lfs: Option<HfLfsMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfLfsMeta {
    #[serde(rename = "oid")]
    sha256: String,
    #[serde(default)]
    size: u64,
}

/// The API returns `gated` as `false`/`true`/`"auto"`/`"manual"` — only the
/// string forms actually gate a download.
fn deserialize_gated<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(matches!(
        v,
        serde_json::Value::String(ref s) if s == "auto" || s == "manual"
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HfRepoEntry {
    id: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    likes: u64,
    #[serde(default, deserialize_with = "deserialize_gated")]
    gated: bool,
    #[serde(default)]
    tags: Vec<String>,
}

/// Parse a search response. Pure function so tests need no network.
pub fn parse_repos(body: &str) -> Result<Vec<HfRepo>, AppError> {
    let entries: Vec<HfRepoEntry> = serde_json::from_str(body)
        .map_err(|e| AppError::invalid_input(format!("Hugging Face search response is invalid: {e}")))?;
    Ok(entries
        .into_iter()
        .map(|e| HfRepo {
            license: e
                .tags
                .iter()
                .find(|t| t.starts_with("license:"))
                .map(|t| t.trim_start_matches("license:").to_string()),
            gated: e.gated,
            id: e.id,
            downloads: e.downloads,
            likes: e.likes,
        })
        .collect())
}

/// Parse a repo tree response into the GGUF files we can offer. Pure function.
pub fn parse_files(body: &str) -> Result<Vec<HfFile>, AppError> {
    let entries: Vec<HfFileEntry> = serde_json::from_str(body)
        .map_err(|e| AppError::invalid_input(format!("Hugging Face tree response is invalid: {e}")))?;
    Ok(entries
        .into_iter()
        .filter(|f| f.path.to_ascii_lowercase().ends_with(".gguf"))
        .filter_map(|f| {
            // Non-LFS GGUF files cannot be pinned, so they are not offered.
            let lfs = f.lfs?;
            Some(HfFile {
                is_mmproj: f.path.to_ascii_lowercase().contains("mmproj"),
                sha256: lfs.sha256,
                size: if lfs.size > 0 { lfs.size } else { f.size },
                path: f.path,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_results() {
        let body = r#"[
            {"id":"unsloth/Qwen3.5-9B-GGUF","downloads":1234,"likes":56,"gated":false,
             "tags":["gguf","license:apache-2.0"]},
            {"id":"org/gated-repo","downloads":10,"likes":1,"gated":"manual","tags":[]}
        ]"#;
        let repos = parse_repos(body).unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].license.as_deref(), Some("apache-2.0"));
        assert!(!repos[0].gated);
        assert!(repos[1].gated);
        assert!(repos[1].license.is_none());
    }

    #[test]
    fn parses_tree_and_detects_mmproj() {
        let body = r#"[
            {"path":"Q4_K_M/model-Q4_K_M.gguf","size":100,
             "lfs":{"oid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":100}},
            {"path":"mmproj-F16.gguf","size":9,
             "lfs":{"oid":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":9}},
            {"path":"README.md","size":5},
            {"path":"not-pinned.gguf","size":7}
        ]"#;
        let files = parse_files(body).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].path.contains("model-Q4_K_M.gguf"));
        assert!(!files[0].is_mmproj);
        assert!(files[1].is_mmproj);
        assert_eq!(files[0].size, 100);
    }

    #[test]
    fn repos_are_validated() {
        assert!(validate_repo("unsloth/Qwen3.5-9B-GGUF").is_ok());
        assert!(validate_repo("no-slash").is_err());
        assert!(validate_repo("/leading").is_err());
        assert!(validate_repo("a/../b").is_err());
        assert!(validate_repo("").is_err());
    }

    #[test]
    fn urlencode_encodes_spaces() {
        assert_eq!(urlencode("qwen 3.5"), "qwen%203.5");
    }
}
