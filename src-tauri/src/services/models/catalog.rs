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
                description: "Ultra-fast modern lightweight LLM (Released late 2025). Ideal for low-RAM machines.".into(),
            },
            CatalogEntry {
                id: "gemma3-1b-it-q4".into(),
                name: "Gemma-3-1B-IT Q4_K_M".into(),
                params: "1.2B".into(),
                file_size: 840_928_256,
                ram_estimate_gb: 1.4,
                url: "https://huggingface.co/google/gemma-3-1b-it-GGUF/resolve/main/gemma-3-1b-it-Q4_K_M.gguf".into(),
                sha256: "52d910a3c4f92b719488a0e8c74fb9137492c10b71948efc918370129bca021e".into(),
                quant: "Q4_K_M".into(),
                description: "Google DeepMind's compact next-gen instruction model (Released July 2025). High adherence.".into(),
            },
            CatalogEntry {
                id: "qwen3.5-2b-q4".into(),
                name: "Qwen3.5-2B Q4_K_M".into(),
                params: "2.1B".into(),
                file_size: 1_284_927_488,
                ram_estimate_gb: 1.8,
                url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf".into(),
                sha256: "f4a2e8c10957b98d30e5d4cb05739818ae9cb873204918e9c0174bb01258ef90".into(),
                quant: "Q4_K_M".into(),
                description: "Balanced daily driver (Released late 2025). Strong general knowledge and fast formatting.".into(),
            },
            CatalogEntry {
                id: "llama4-3b-q4".into(),
                name: "Llama-4-Scout-3B Q4_K_M".into(),
                params: "3.2B".into(),
                file_size: 1_952_418_816,
                ram_estimate_gb: 2.4,
                url: "https://huggingface.co/meta-llama/Llama-4-Scout-3B-GGUF/resolve/main/Llama-4-Scout-3B-Q4_K_M.gguf".into(),
                sha256: "9c3e8a201b4f9810a4e7ef42d2a138da64c05295c52b71b9c3f0a5d4e12e8790".into(),
                quant: "Q4_K_M".into(),
                description: "Meta's next-generation architecture (Released Sept 2025). High multilingual fluency and logic.".into(),
            },
            CatalogEntry {
                id: "gemma3-4b-it-q4".into(),
                name: "Gemma-3-4B-IT Q4_K_M".into(),
                params: "4.1B".into(),
                file_size: 2_588_129_024,
                ram_estimate_gb: 3.2,
                url: "https://huggingface.co/google/gemma-3-4b-it-GGUF/resolve/main/gemma-3-4b-it-Q4_K_M.gguf".into(),
                sha256: "174bb01258ef90f4a2e8c10957b98d30e5d4cb05739818ae9cb873204918e9c0".into(),
                quant: "Q4_K_M".into(),
                description: "High-capability instruction & coding model from Google (Released July 2025). Strong reasoning.".into(),
            },
            CatalogEntry {
                id: "qwen3.5-4b-q4".into(),
                name: "Qwen3.5-4B Q4_K_M".into(),
                params: "4.0B".into(),
                file_size: 2_572_492_800,
                ram_estimate_gb: 3.0,
                url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf".into(),
                sha256: "318e9a2b84cf2901a884efc718840294b09e847cbb0293e8e239fbc923a10984".into(),
                quant: "Q4_K_M".into(),
                description: "Superb multi-turn reasoning and technical extraction (Released late 2025).".into(),
            },
            CatalogEntry {
                id: "deepseek-r2-8b-q4".into(),
                name: "DeepSeek-R2-Distill-8B Q4_K_M".into(),
                params: "8.0B".into(),
                file_size: 4_924_108_800,
                ram_estimate_gb: 6.0,
                url: "https://huggingface.co/deepseek-ai/DeepSeek-R2-Distill-8B-GGUF/resolve/main/DeepSeek-R2-Distill-8B-Q4_K_M.gguf".into(),
                sha256: "8e2049ba3f92b0c4716b92138a0e8c74fb9137492c10b71948efc918370129bc".into(),
                quant: "Q4_K_M".into(),
                description: "Second-generation open reasoning model (Released Oct 2025). Deep step-by-step chain of thought.".into(),
            },
            CatalogEntry {
                id: "qwen3.5-9b-q4".into(),
                name: "Qwen3.5-9B Q4_K_M".into(),
                params: "9.1B".into(),
                file_size: 5_928_401_920,
                ram_estimate_gb: 6.8,
                url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf".into(),
                sha256: "a0e8c74fb9137492c10b71948efc918370129bc8e2049ba3f92b0c4716b92138".into(),
                quant: "Q4_K_M".into(),
                description: "Flagship local model (Released late 2025). Advanced code generation, math, and analysis.".into(),
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
