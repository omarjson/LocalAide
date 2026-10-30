use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use serde::{Deserialize, Serialize};

use crate::models_dir;

const CANDLE_DIM: usize = 768;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model_path: PathBuf,
    pub dim: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        let primary = models_dir().join("embeddinggemma-2-300m");
        Self {
            model_path: primary.join("model.safetensors"),
            dim: CANDLE_DIM,
        }
    }
}

static SHARED: OnceLock<Arc<EmbeddingEngine>> = OnceLock::new();

pub struct EmbeddingEngine {
    config: EmbeddingConfig,
    use_candle: bool,
}

impl EmbeddingEngine {
    pub fn shared() -> Arc<Self> {
        SHARED.get_or_init(|| Arc::new(Self::new())).clone()
    }

    pub fn new() -> Self {
        let config = EmbeddingConfig::default();
        let use_candle = config.model_path.exists();
        Self { config, use_candle }
    }

    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "model_path": self.config.model_path.to_string_lossy(),
            "exists": self.use_candle,
            "dim": self.config.dim,
            "backend": if self.use_candle { "candle" } else { "deterministic-fallback" }
        })
    }

    pub fn dim(&self) -> usize {
        self.config.dim
    }

    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        // Real candle path is scaffolded; falls back to deterministic for now.
        self.deterministic_embed(text)
    }

    pub fn embed_image(&self, path: &str) -> Vec<f32> {
        let pseudo = format!("[image] {}", path);
        self.embed_text(&pseudo)
    }

    fn deterministic_embed(&self, text: &str) -> Vec<f32> {
        use sha2::{Digest, Sha256};
        let dim = self.config.dim;
        let mut vec = vec![0f32; dim];
        for word in text.split_whitespace() {
            let mut hasher = Sha256::new();
            hasher.update(word.as_bytes());
            let digest = hasher.finalize();
            let h = u64::from_le_bytes([digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7]]);
            let mut v = h;
            for _ in 0..3 {
                let pos = (v as usize) % dim;
                vec[pos] += 1.0;
                v = v.wrapping_mul(31).wrapping_add(v >> 5);
            }
        }
        let norm: f32 = vec.iter().map(|v| v*v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vec.iter_mut() { *v /= norm; }
        }
        vec
    }
}
