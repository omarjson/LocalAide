//! Community-model commands: search the Hugging Face hub, inspect a
//! repository's GGUF files, and register one as a downloadable model
//! (ADR-0010). Registration only records metadata — the download itself
//! runs through the same verified pipeline as the curated catalog, pinned
//! to the SHA-256 Hugging Face serves for the chosen file.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::models::catalog::MAX_COMPANIONS_PER_MODEL;
use crate::services::models::catalog::{CatalogCompanion, CatalogEntry};
use crate::services::models::custom::{community_id, unique_id};
use crate::services::models::hf::{HfClient, HfFile, HfRepo};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Public GGUF repositories matching a search, most-downloaded first.
#[tauri::command]
pub async fn hf_search(query: String, limit: Option<u32>) -> Result<Vec<HfRepo>, AppError> {
    let client = HfClient::new()?;
    client
        .search(
            query.trim(),
            limit.unwrap_or(crate::services::models::hf::DEFAULT_SEARCH_LIMIT),
        )
        .await
}

/// Every GGUF file in a repository, with the SHA-256 each download would be
/// verified against. Non-LFS files are omitted (they cannot be pinned).
#[tauri::command]
pub async fn hf_repo_files(repo: String) -> Result<Vec<HfFile>, AppError> {
    let client = HfClient::new()?;
    client.repo_files(repo.trim()).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfCustomAddResult {
    pub id: String,
}

/// Register one repository file as a community model. Split GGUFs carry
/// their sibling shards as companions, and an `mmproj` projector can ride
/// along for vision. No download happens here.
#[tauri::command]
pub async fn hf_custom_add(
    state: State<'_, AppState>,
    repo: String,
    path: String,
    kind: Option<String>,
    license: Option<String>,
    with_mmproj: bool,
) -> Result<HfCustomAddResult, AppError> {
    let repo = repo.trim().to_string();
    let path = path.trim().to_string();
    let kind = kind.unwrap_or_else(|| "chat".to_string());
    if kind != "chat" && kind != "code" && kind != "embedding" && kind != "reranker" {
        return Err(AppError::invalid_input(format!(
            "'{kind}' is not a known model kind"
        )));
    }

    let client = HfClient::new()?;
    let files = client.repo_files(&repo).await?;
    let primary = files
        .iter()
        .find(|f| f.path == path)
        .ok_or_else(|| {
            AppError::invalid_input(format!("'{path}' is not a pinned GGUF file in {repo}"))
        })?
        .clone();

    let (org, repo_name) = repo
        .split_once('/')
        .ok_or_else(|| AppError::invalid_input("repository must be org/name"))?;
    let filename = primary
        .path
        .rsplit('/')
        .next()
        .unwrap_or(&primary.path)
        .to_string();
    let id = unique_id(
        &state.model_service.all_entries(),
        community_id(org, repo_name, &filename),
    );

    // A split GGUF's primary keeps its upstream shard name so llama.cpp
    // discovers the remaining shards next to it by the -00001-of pattern.
    let is_shard_group = primary.path.contains("-00001-of-");
    let primary_dest = filename.clone();
    let mut companions: Vec<CatalogCompanion> = Vec::new();

    if is_shard_group {
        let prefix = &primary.path[..primary.path.find("-00001-of-").expect("checked above")];
        for f in &files {
            if f.path != primary.path && f.path.starts_with(prefix) && f.path.ends_with(".gguf") {
                if companions.len() >= MAX_COMPANIONS_PER_MODEL {
                    return Err(AppError::invalid_input(
                        "This split GGUF has more shards than the companion limit allows",
                    ));
                }
                let shard_dest = f.path.rsplit('/').next().unwrap_or(&f.path).to_string();
                companions.push(CatalogCompanion {
                    url: format!(
                        "https://huggingface.co/{repo}/resolve/main/{}",
                        f.path.replace(' ', "%20")
                    ),
                    sha256: f.sha256.clone(),
                    file_size: f.size,
                    dest: shard_dest,
                });
            }
        }
    }

    // Total size across shards, mirroring the curated catalog's convention.
    let shard_bytes: u64 = if is_shard_group {
        companions.iter().map(|c| c.file_size).sum()
    } else {
        0
    };
    let total_size = primary.size + shard_bytes;

    // Vision: attach the mmproj projector nearest the primary (same folder
    // first, else the smallest) named so the engine's sibling-file discovery
    // finds it.
    if with_mmproj && !is_mmproj_file(&primary.path) {
        let same_dir: Vec<&HfFile> = files
            .iter()
            .filter(|f| f.is_mmproj && parent_of(&f.path) == parent_of(&primary.path))
            .collect();
        let projector = match same_dir.iter().min_by_key(|f| f.size) {
            Some(f) => Some(*f),
            None => files.iter().filter(|f| f.is_mmproj).min_by_key(|f| f.size),
        };
        if let Some(p) = projector {
            let stem = primary_dest.strip_suffix(".gguf").unwrap_or(&primary_dest);
            companions.push(CatalogCompanion {
                url: format!(
                    "https://huggingface.co/{repo}/resolve/main/{}",
                    p.path.replace(' ', "%20")
                ),
                sha256: p.sha256.clone(),
                file_size: p.size,
                dest: format!("{stem}.mmproj.gguf"),
            });
        }
    }

    // Honest RAM estimate: weights plus a little headroom for KV cache.
    let ram = ((total_size as f64 / 1_000_000_000.0) * 1.15 + 0.5).clamp(0.2, 512.0) as f32;
    let quant = quant_from_filename(&filename);

    let entry = CatalogEntry {
        id: id.clone(),
        name: format!("{repo_name} · {quant}"),
        params: "community".to_string(),
        kind,
        license: license.unwrap_or_else(|| "unknown".to_string()),
        file_size: total_size,
        ram_estimate_gb: ram,
        url: format!(
            "https://huggingface.co/{repo}/resolve/main/{}",
            primary.path.replace(' ', "%20")
        ),
        sha256: primary.sha256.clone(),
        quant,
        description: format!(
            "Community model from huggingface.co/{repo}. The download is verified against the hash Hugging Face serves for this file; the weights and license are not vetted by LocalAide."
        ),
        context_tokens: None,
        dest: if is_shard_group {
            Some(primary_dest)
        } else {
            None
        },
        companions,
    };

    state.model_service.custom_models().add(entry)?;
    Ok(HfCustomAddResult { id })
}

fn is_mmproj_file(path: &str) -> bool {
    path.to_ascii_lowercase().contains("mmproj")
}

fn parent_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Pull a quantization label out of a GGUF filename (Q4_K_M, IQ2_XXS, MXFP4,
/// F16, …), falling back to "GGUF".
fn quant_from_filename(filename: &str) -> String {
    let upper = filename.to_ascii_uppercase();
    for token in [
        "MXFP4", "F16", "BF16", "F32", "Q8_0", "Q6_K", "Q5_K_M", "Q5_K_S", "Q4_K_M", "Q4_K_S",
        "Q4_0", "Q3_K_XL", "Q3_K_M", "Q3_K_S", "Q2_K_XL", "Q2_K", "IQ4_XS", "IQ3_XXS", "IQ2_XXS",
        "IQ1_S", "IQ1_M",
    ] {
        if upper.contains(token) {
            return token.to_string();
        }
    }
    "GGUF".to_string()
}

#[cfg(test)]
mod tests {
    use super::quant_from_filename;

    #[test]
    fn detects_quant_labels() {
        assert_eq!(quant_from_filename("model-Q4_K_M.gguf"), "Q4_K_M");
        assert_eq!(
            quant_from_filename("GLM-5.3-Flash-UD-IQ2_XXS-00001-of-00004.gguf"),
            "IQ2_XXS"
        );
        assert_eq!(quant_from_filename("gpt-oss-20b-MXFP4.gguf"), "MXFP4");
        assert_eq!(quant_from_filename("mystery.gguf"), "GGUF");
    }
}
