use crate::app::error::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub params: String,
    pub file_size: u64,
    pub ram_estimate_gb: f32,
    pub url: String,
    pub sha256: String,
    pub quant: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogManifest {
    pub version: u32,
    pub updated_at: u64,
    pub signature: String,
    pub models: Vec<CatalogEntry>,
}

impl CatalogManifest {
    pub fn compute_signature(models: &[CatalogEntry]) -> String {
        let mut hasher = Sha256::new();
        for m in models {
            hasher.update(m.id.as_bytes());
            hasher.update(m.sha256.as_bytes());
            hasher.update(&m.file_size.to_le_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn verify(&self) -> Result<(), AppError> {
        let expected = Self::compute_signature(&self.models);
        if self.signature != expected {
            return Err(AppError::new(
                "CATALOG_UNVERIFIED",
                "Model catalog integrity signature mismatch",
            ));
        }
        Ok(())
    }

    pub fn default_manifest() -> Self {
        let models = vec![
            CatalogEntry {
                id: "qwen3.5-0.8b-q4".into(),
                name: "Qwen3.5-0.8B Q4_K_M".into(),
                params: "0.8B".into(),
                file_size: 531_248_128,
                ram_estimate_gb: 1.0,
                url: "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q4_K_M.gguf".into(),
                sha256: "b9c3f0a5d4e12e8790b83e6024e0310fb176e7ef42d2a138da64c05295c52b71".into(),
                quant: "Q4_K_M".into(),
                description: "Lightweight and fast, optimized for machines with 4-8 GB RAM.".into(),
            },
            CatalogEntry {
                id: "qwen3.5-2b-q4".into(),
                name: "Qwen3.5-2B Q4_K_M".into(),
                params: "2B".into(),
                file_size: 1_284_927_488,
                ram_estimate_gb: 1.6,
                url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf".into(),
                sha256: "f4a2e8c10957b98d30e5d4cb05739818ae9cb873204918e9c0174bb01258ef90".into(),
                quant: "Q4_K_M".into(),
                description: "Strong conversational quality and balanced reasoning.".into(),
            },
            CatalogEntry {
                id: "qwen3.5-4b-q4".into(),
                name: "Qwen3.5-4B Q4_K_M".into(),
                params: "4B".into(),
                file_size: 2_572_492_800,
                ram_estimate_gb: 2.8,
                url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf".into(),
                sha256: "318e9a2b84cf2901a884efc718840294b09e847cbb0293e8e239fbc923a10984".into(),
                quant: "Q4_K_M".into(),
                description: "High accuracy and multi-turn instruction following.".into(),
            },
            CatalogEntry {
                id: "qwen3.5-9b-q4".into(),
                name: "Qwen3.5-9B Q4_K_M".into(),
                params: "9B".into(),
                file_size: 5_928_401_920,
                ram_estimate_gb: 6.2,
                url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf".into(),
                sha256: "8e2049ba3f92b0c4716b92138a0e8c74fb9137492c10b71948efc918370129bc".into(),
                quant: "Q4_K_M".into(),
                description: "Advanced coding, mathematical logic, and complex synthesis.".into(),
            },
        ];

        let signature = Self::compute_signature(&models);
        Self {
            version: 1,
            updated_at: 1730318400,
            signature,
            models,
        }
    }
}
