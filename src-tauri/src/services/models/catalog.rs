use crate::app::error::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Hard limits applied to any manifest entering the app. A fetched body is
/// rejected before parsing once it exceeds MAX_MANIFEST_BYTES, so a hostile
/// or broken server cannot stream unbounded data into memory.
pub const MAX_MANIFEST_BYTES: usize = 512 * 1024;
pub const MAX_MODELS: usize = 200;
const MAX_COMPANIONS_PER_MODEL: usize = 8;
const KNOWN_KINDS: [&str; 4] = ["chat", "code", "embedding", "reranker"];

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Catalog download URLs come from the model list, never from the user, and
/// must always be HTTPS — the models themselves are additionally pinned by
/// SHA-256, so a compromised host cannot swap weights silently.
fn require_https(url: &str, what: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| AppError::invalid_input(format!("Catalog {what} URL is not valid: {url}")))?;
    if parsed.scheme() != "https" {
        return Err(AppError::invalid_input(format!(
            "Catalog {what} URL must use HTTPS: {url}"
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::invalid_input(format!(
            "Catalog {what} URL must not embed credentials: {url}"
        )));
    }
    if parsed.host_str().is_none() {
        return Err(AppError::invalid_input(format!(
            "Catalog {what} URL has no host: {url}"
        )));
    }
    Ok(())
}

/// A file downloaded with a model, such as a vision projector (`mmproj`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogCompanion {
    pub url: String,
    pub sha256: String,
    pub file_size: u64,
    /// File name inside the models folder. Must be unique per model.
    pub dest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub params: String,
    /// One of: "chat", "code", "embedding", "reranker".
    pub kind: String,
    /// Short license label shown in the UI, for example "Apache-2.0".
    pub license: String,
    pub file_size: u64,
    pub ram_estimate_gb: f32,
    pub url: String,
    pub sha256: String,
    pub quant: String,
    pub description: String,
    /// Native context window in tokens, when the model has one worth
    /// exposing. `None` falls back to the app-wide context setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u32>,
    /// Primary file name inside the models folder. Defaults to `{id}.gguf`.
    /// Split GGUFs keep their upstream shard name so llama.cpp discovers the
    /// remaining shards next to it by the `-00001-of-000NN` pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    #[serde(default)]
    pub companions: Vec<CatalogCompanion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogManifest {
    pub version: u32,
    pub updated_at: u64,
    pub signature: String,
    pub models: Vec<CatalogEntry>,
}

impl CatalogManifest {
    /// Digest of the catalog entries. This detects accidental edits to the
    /// built-in list. It is not a publisher signature.
    pub fn compute_signature(models: &[CatalogEntry]) -> String {
        let mut hasher = Sha256::new();
        for m in models {
            hasher.update(m.id.as_bytes());
            hasher.update(m.sha256.as_bytes());
            hasher.update(m.file_size.to_le_bytes());
            for c in &m.companions {
                hasher.update(c.sha256.as_bytes());
                hasher.update(c.file_size.to_le_bytes());
            }
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

    /// Structural validation for any manifest entering the app, fetched or
    /// loaded from cache. `min_version` is the version of the currently
    /// active catalog: rejecting anything older makes catalog rollback
    /// impossible after an app update ships a newer list (see ADR-0008).
    pub fn validate(&self, min_version: u32) -> Result<(), AppError> {
        if self.version < min_version {
            return Err(AppError::invalid_input(format!(
                "Catalog version {} is older than the active version {min_version}; refusing rollback",
                self.version
            )));
        }
        if self.models.is_empty() {
            return Err(AppError::invalid_input("Catalog has no models"));
        }
        if self.models.len() > MAX_MODELS {
            return Err(AppError::invalid_input(format!(
                "Catalog lists {} models; the limit is {MAX_MODELS}",
                self.models.len()
            )));
        }

        let mut seen_ids = std::collections::HashSet::new();
        for m in &self.models {
            if m.id.is_empty() || !seen_ids.insert(m.id.as_str()) {
                return Err(AppError::invalid_input(format!(
                    "Catalog model id '{}' is empty or duplicated",
                    m.id
                )));
            }
            m.validate_entry()?;
        }
        Ok(())
    }

    /// Built-in catalog, version 8.
    ///
    /// Each entry was checked against the Hugging Face API (2026-11-13 for
    /// versions 1-6, 2026-11-14 for version 7). The repo is public and not
    /// gated. The file size and SHA-256 come from the file's LFS metadata.
    /// The license label is the one on the repo. Sources and the rejected
    /// list are in `docs/evidence/models-2026-11.md`.
    #[rustfmt::skip]
    pub fn default_manifest() -> Self {
        let models = vec![
            // Tiny: under 1 GB, and about 1B parameters
            entry(
                "qwen3.5-0.8b-q4", "Qwen3.5 0.8B", "0.8B", "chat", "Apache-2.0",
                532_517_120, 1.0,
                "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q4_K_M.gguf",
                "bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517",
                "Q4_K_M", "Smallest Qwen3.5. Runs on almost any computer. Accepts images.",
                vec![companion(
                    "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/mmproj-F16.gguf",
                    "56e4c6cfe73b0c82e3e82bc518d7591997e61d81f723fc41a586f4fa69ea2453",
                    204_987_232, "qwen3.5-0.8b-q4.mmproj.gguf",
                )],
            ),
            entry(
                "qwen3-0.6b-q4", "Qwen3 0.6B", "0.6B", "chat", "Apache-2.0",
                396_705_472, 1.0,
                "https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q4_K_M.gguf",
                "ac2d97712095a558e31573f62f466a3f9d93990898b0ec79d7c974c1780d524a",
                "Q4_K_M", "Compact Qwen3 for very small machines. Text only.", vec![],
            ),
            entry(
                "lfm2.5-230m-q4", "LFM2.5 230M", "230M", "chat", "LFM Open License",
                153_406_304, 0.5,
                "https://huggingface.co/LiquidAI/LFM2.5-230M-GGUF/resolve/main/LFM2.5-230M-Q4_K_M.gguf",
                "7bbd90384d3deffe4c646ec9643b212802d32d4ce417c90a1ec9282100650062",
                "Q4_K_M", "Tiniest Liquid model. Text only. Custom license: check it before shipping.", vec![],
            ),
            entry(
                "lfm2.5-350m-q4", "LFM2.5 350M", "350M", "chat", "LFM Open License",
                229_312_224, 0.7,
                "https://huggingface.co/LiquidAI/LFM2.5-350M-GGUF/resolve/main/LFM2.5-350M-Q4_K_M.gguf",
                "7e6f72643caafc9a68256686638c4d7916f2cec76d1df478d4c3ddcd95a6aed4",
                "Q4_K_M", "Small Liquid model. Text only. Custom license: check it before shipping.", vec![],
            ),
            entry(
                "minicpm5-1b-q4", "MiniCPM5 1B", "1B", "chat", "Apache-2.0",
                688_065_920, 1.5,
                "https://huggingface.co/openbmb/MiniCPM5-1B-GGUF/resolve/main/MiniCPM5-1B-Q4_K_M.gguf",
                "81b64d05a23b17b34c475f42b3e72fbde62d4b92cc34541f7a8031d0752deafa",
                "Q4_K_M", "OpenBMB 1B model for on-device chat.", vec![],
            ),
            entry(
                "lfm2.5-1.2b-q4", "LFM2.5 1.2B Instruct", "1.2B", "chat", "LFM Open License",
                730_895_168, 1.6,
                "https://huggingface.co/LiquidAI/LFM2.5-1.2B-Instruct-GGUF/resolve/main/LFM2.5-1.2B-Instruct-Q4_K_M.gguf",
                "b1b3de114215d9507409a662a501a631095a479a419584e8a2ded6304b19b4f5",
                "Q4_K_M", "Liquid 1.2B instruct model. Custom license: check it before shipping.", vec![],
            ),
            entry(
                "qwen3-1.7b-q4", "Qwen3 1.7B", "1.7B", "chat", "Apache-2.0",
                1_107_409_472, 2.0,
                "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf",
                "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
                "Q4_K_M", "Small chat model with optional thinking mode.", vec![],
            ),
            // Small: 2 to 4 B
            entry(
                "qwen3.5-2b-q4", "Qwen3.5 2B", "2B", "chat", "Apache-2.0",
                1_280_835_840, 2.0,
                "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf",
                "aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223",
                "Q4_K_M", "Everyday chat model. Accepts images.",
                vec![companion(
                    "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/mmproj-F16.gguf",
                    "7035e9cb8d7c6a9681d07eef9a364783e86ea4cd73faab2eabb4f43a101830c7",
                    668_227_264, "qwen3.5-2b-q4.mmproj.gguf",
                )],
            ),
            entry(
                "minicpm5-2b-q4", "MiniCPM5 2B", "2B", "chat", "Apache-2.0",
                1_561_318_368, 2.8,
                "https://huggingface.co/openbmb/MiniCPM5-2B-GGUF/resolve/main/MiniCPM5-2B-Q4_K_M.gguf",
                "ec2d5801640099e97d8d7e8003ad4d81f336e757811f03a26173dddf386602fd",
                "Q4_K_M", "OpenBMB 2B model for on-device chat.", vec![],
            ),
            entry(
                "lfm3-1.7b-q4", "LFM3 1.7B", "1.7B", "chat", "LFM Open License",
                1_099_627_808, 2.0,
                "https://huggingface.co/LiquidAI/LFM3-1.7B-GGUF/resolve/main/LFM3-1.7B-Q4_K_M.gguf",
                "3793e1e6e2266919d6e015287221b41100175a98eddaba92e111aed3b05c93c4",
                "Q4_K_M", "Liquid third-generation 1.7B model. Custom license: check it before shipping.", vec![],
            ),
            entry(
                "granite-4.0-h-micro-q4", "Granite 4.0 H Micro 3B", "3B", "chat", "Apache-2.0",
                1_942_564_800, 2.8,
                "https://huggingface.co/unsloth/granite-4.0-h-micro-GGUF/resolve/main/granite-4.0-h-micro-Q4_K_M.gguf",
                "1f21ba20ca249f6de1b7fcc436d95bba80a11d40060532bf6eb0c6a9e54e5d28",
                "Q4_K_M", "IBM Granite 4.0 hybrid 3B. Good at tool calls and structured output.", vec![],
            ),
            entry(
                "smollm3-3b-q4", "SmolLM3 3B", "3B", "chat", "Apache-2.0",
                1_915_306_528, 3.0,
                "https://huggingface.co/unsloth/SmolLM3-3B-GGUF/resolve/main/SmolLM3-3B-Q4_K_M.gguf",
                "4de907d2d388a5508fb7cb443a06effe14cce3518b0a78d3bdd9e74d9edce989",
                "Q4_K_M", "Hugging Face 3B model with long context and thinking mode.", vec![],
            ),
            entry(
                "qwen3-4b-2507-q4", "Qwen3 4B Instruct 2507", "4B", "chat", "Apache-2.0",
                2_497_281_120, 3.6,
                "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
                "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597",
                "Q4_K_M", "Qwen3 4B tuned for everyday answers. Text only.", vec![],
            ),
            entry(
                "qwen3.5-4b-q4", "Qwen3.5 4B", "4B", "chat", "Apache-2.0",
                2_740_937_888, 3.6,
                "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf",
                "00fe7986ff5f6b463e62455821146049db6f9313603938a70800d1fb69ef11a4",
                "Q4_K_M", "Recommended general assistant. Accepts images.",
                vec![companion(
                    "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/mmproj-F16.gguf",
                    "cd88edcf8d031894960bb0c9c5b9b7e1fea6ebee02b9f7ce925a00d12891f864",
                    672_423_616, "qwen3.5-4b-q4.mmproj.gguf",
                )],
            ),
            entry(
                "gemma4-e2b-qat-q4", "Gemma 4 E2B (QAT)", "2B effective", "chat", "Apache-2.0",
                3_349_516_256, 4.0,
                "https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/resolve/main/gemma-4-E2B_q4_0-it.gguf",
                "fa401b55b07ee70a54c6dae3903c783a6e65064312529ea57175cb5f8dec6634",
                "Q4_0 QAT", "Google Gemma 4 E2B, quantized during training for better quality at 4-bit. Accepts images.",
                vec![companion(
                    "https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/resolve/main/gemma-4-E2B-it-mmproj.gguf",
                    "021059cce659fe7f9170d5599761d7bbaf644b798dab9503aca30dc43e6beb14",
                    986_833_664, "gemma4-e2b-qat-q4.mmproj.gguf",
                )],
            ),
            entry(
                "granite-4.2-3b-q4", "Granite 4.2 3B", "3B", "chat", "Apache-2.0",
                2_244_011_552, 3.6,
                "https://huggingface.co/ibm-granite/granite-4.2-3b-GGUF/resolve/main/granite-4.2-3b-Q4_K_M.gguf",
                "e0406663965846ae22a403456eb826ccce5f450840491f71952f18a7cb78e7d5",
                "Q4_K_M", "IBM Granite 4.2 3B. Enterprise-oriented, strong at instructions.", vec![],
            ),
            entry(
                "spark-x2.6-4b-q4", "Spark-X2.6 4B", "4B", "chat", "Apache-2.0",
                3_543_441_312, 3.8,
                "https://huggingface.co/XHToken/Spark-X2.6-4B-GGUF/resolve/main/Spark-X2.6-4B-Q4_K_M.gguf",
                "91cb2811aebb47ac916aa619d27d4cdd7f4143106b7917accb8ead4cd7f5cbd9",
                "Q4_K_M", "October 2026 on-device agent model tuned for tool use and real-world tasks. Text only.", vec![],
            ),
            entry(
                "spark-x2.6-1.7b-q8", "Spark-X2.6 1.7B", "1.7B", "chat", "Apache-2.0",
                1_820_113_440, 2.2,
                "https://huggingface.co/XHToken/Spark-X2.6-1.7B-GGUF/resolve/main/Spark-X2.6-1.7B-Q8_0.gguf",
                "8d96279c95ae2be111576f3f599880598be0922efee15069b4d7e4dd96ef6ed4",
                "Q8_0", "October 2026, the smallest 2026-generation agent model. Runs on almost any machine. Text only.", vec![],
            ),
            // Desktop: 6 to 9 B
            entry(
                "gemma4-e4b-qat-q4", "Gemma 4 E4B (QAT)", "4B effective", "chat", "Apache-2.0",
                5_154_941_280, 6.0,
                "https://huggingface.co/google/gemma-4-E4B-it-qat-q4_0-gguf/resolve/main/gemma-4-E4B_q4_0-it.gguf",
                "676c35070db6dbe52f93e9c864ee0fba4eddea94b9c875d9cb10daff453fbaee",
                "Q4_0 QAT", "Google Gemma 4 E4B, quantized during training. Accepts images.",
                vec![companion(
                    "https://huggingface.co/google/gemma-4-E4B-it-qat-q4_0-gguf/resolve/main/gemma-4-E4B-it-mmproj.gguf",
                    "7498a37cb619e55f2fcf87eb931f56e99389ed6d432e4c5c66110694c0d65578",
                    991_552_256, "gemma4-e4b-qat-q4.mmproj.gguf",
                )],
            ),
            entry(
                "qwen3-8b-q4", "Qwen3 8B", "8B", "chat", "Apache-2.0",
                5_027_784_512, 7.0,
                "https://huggingface.co/unsloth/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf",
                "120307ba529eb2439d6c430d94104dabd578497bc7bfe7e322b5d9933b449bd4",
                "Q4_K_M", "Strong general chat model with thinking mode.", vec![],
            ),
            entry(
                "glm-5.3-flash-iq2xxs", "GLM-5.3 Flash (MoE)", "320B total, 18B active", "chat", "MIT",
                101_844_951_904, 97.0,
                "https://huggingface.co/unsloth/GLM-5.3-Flash-GGUF/resolve/main/UD-IQ2_XXS/GLM-5.3-Flash-UD-IQ2_XXS-00001-of-00004.gguf",
                "337870cefce5fb34b2aa3ade54a0fbf61676697f7f9a906670c20edde74561f4",
                "UD-IQ2_XXS", "Z.ai's frontier mixture-of-experts with vision input (August 2026). Ships as four split GGUF shards llama.cpp loads together; only 18B parameters run per token, so a workstation RAM or expert offload is expected.",
                vec![
                    companion(
                        "https://huggingface.co/unsloth/GLM-5.3-Flash-GGUF/resolve/main/UD-IQ2_XXS/GLM-5.3-Flash-UD-IQ2_XXS-00002-of-00004.gguf",
                        "d51a3d9ca05022d53e71753df75ee64ed5fd4e47655650ae023e9717b3ec158d",
                        49_896_298_080, "GLM-5.3-Flash-UD-IQ2_XXS-00002-of-00004.gguf",
                    ),
                    companion(
                        "https://huggingface.co/unsloth/GLM-5.3-Flash-GGUF/resolve/main/UD-IQ2_XXS/GLM-5.3-Flash-UD-IQ2_XXS-00003-of-00004.gguf",
                        "89180f13784dc1128586b98adf1ca876279881a20bc4f01747703ce352563372",
                        49_248_507_840, "GLM-5.3-Flash-UD-IQ2_XXS-00003-of-00004.gguf",
                    ),
                    companion(
                        "https://huggingface.co/unsloth/GLM-5.3-Flash-GGUF/resolve/main/UD-IQ2_XXS/GLM-5.3-Flash-UD-IQ2_XXS-00004-of-00004.gguf",
                        "9e0ef47daa1fdffce3cf2ac1594ce4634d176ad018be6e692729757f38d5fbf6",
                        2_690_716_000, "GLM-5.3-Flash-UD-IQ2_XXS-00004-of-00004.gguf",
                    ),
                    companion(
                        "https://huggingface.co/unsloth/GLM-5.3-Flash-GGUF/resolve/main/mmproj-F16.gguf",
                        "922b809664d6a89cc1e40bfeaa63618000edc6651baf4082ce0718a784adf947",
                        1_128_047_296, "glm-5.3-flash.mmproj.gguf",
                    ),
                ],
            ).with_dest("GLM-5.3-Flash-UD-IQ2_XXS-00001-of-00004.gguf"),
            entry(
                "qwen3.5-9b-q4", "Qwen3.5 9B", "9B", "chat", "Apache-2.0",
                5_680_522_464, 7.5,
                "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q4_K_M.gguf",
                "03b74727a860a56338e042c4420bb3f04b2fec5734175f4cb9fa853daf52b7e8",
                "Q4_K_M", "Strong general model with image input.",
                vec![companion(
                    "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/mmproj-F16.gguf",
                    "f70dc3509053962b0d0d3ee8a7eacebf5d60aa560cad78254ae8698516ae029f",
                    918_166_080, "qwen3.5-9b-q4.mmproj.gguf",
                )],
            ),
            entry(
                "granite-4.2-8b-q4", "Granite 4.2 8B", "8B", "chat", "Apache-2.0",
                5_347_917_952, 8.0,
                "https://huggingface.co/ibm-granite/granite-4.2-8b-GGUF/resolve/main/granite-4.2-8b-Q4_K_M.gguf",
                "16a9369d0805f80b7377d25d87f937a90c05dc04ad79173a52001e42c9aab311",
                "Q4_K_M", "IBM Granite 4.2 8B. Strong at tool use and structured output.", vec![],
            ),
            entry(
                "granite-4.0-h-tiny-q4", "Granite 4.0 H Tiny (MoE)", "7B total, 1B active", "chat", "Apache-2.0",
                4_254_815_392, 6.0,
                "https://huggingface.co/unsloth/granite-4.0-h-tiny-GGUF/resolve/main/granite-4.0-h-tiny-Q4_K_M.gguf",
                "d9baffe0c4c061e699ddeb8a51271f1e1bab29579a27ad3f213bf621a30b24ee",
                "Q4_K_M", "IBM Granite 4.0 mixture-of-experts. Fast, since only 1B parameters run per token.", vec![],
            ),
            // Desktop large: 12 B
            entry(
                "gemma4-12b-qat-q4", "Gemma 4 12B (QAT)", "12B", "chat", "Apache-2.0",
                6_975_879_296, 9.5,
                "https://huggingface.co/google/gemma-4-12b-it-qat-q4_0-gguf/resolve/main/gemma-4-12b-it-qat-q4_0.gguf",
                "93567e57a8fe10b23569b9d9ec38cd005deedf71e29477c421a4b83f418a538b",
                "Q4_0 QAT", "Google Gemma 4 12B, quantized during training. Accepts images.",
                vec![companion(
                    "https://huggingface.co/google/gemma-4-12b-it-qat-q4_0-gguf/resolve/main/mmproj-gemma-4-12b-it-qat-q4_0.gguf",
                    "cb018338a7538a9814d994bfe54644c71eb7ed54e31eae2f721e45fd3c260da7",
                    175_115_616, "gemma4-12b-qat-q4.mmproj.gguf",
                )],
            ),
            // Power: 16 GB and up, slow on CPU
            entry(
                "mistral-small-4-119b-mxfp4", "Mistral Small 4 (MoE)", "119B total, ~6B active", "chat", "Apache-2.0",
                71_812_829_184, 69.0,
                "https://huggingface.co/unsloth/Mistral-Small-4-119B-2603-GGUF/resolve/main/MXFP4_MOE/Mistral-Small-4-119B-2603-MXFP4_MOE-00001-of-00003.gguf",
                "a46511af19832459a6a967c5a259d5b824c808017d33683d281fa0144c690009",
                "MXFP4_MOE", "Mistral's 2026 flagship: 128 experts with native MXFP4 quantization, vision input, 256k context (March 2026). Ships as three split GGUF shards llama.cpp loads together.",
                vec![
                    companion(
                        "https://huggingface.co/unsloth/Mistral-Small-4-119B-2603-GGUF/resolve/main/MXFP4_MOE/Mistral-Small-4-119B-2603-MXFP4_MOE-00002-of-00003.gguf",
                        "89ab81ea411fff16829a2b79632225f844ca4b4386d5c5d25f1bc7d8d418a484",
                        48_912_374_208, "Mistral-Small-4-119B-2603-MXFP4_MOE-00002-of-00003.gguf",
                    ),
                    companion(
                        "https://huggingface.co/unsloth/Mistral-Small-4-119B-2603-GGUF/resolve/main/MXFP4_MOE/Mistral-Small-4-119B-2603-MXFP4_MOE-00003-of-00003.gguf",
                        "492313b6d897651cf917968d2e3b5ee66793a20cb35144752ffe7ca560679743",
                        22_892_579_360, "Mistral-Small-4-119B-2603-MXFP4_MOE-00003-of-00003.gguf",
                    ),
                    companion(
                        "https://huggingface.co/unsloth/Mistral-Small-4-119B-2603-GGUF/resolve/main/mmproj-F16.gguf",
                        "8afb53096537664e248a0b4a9240c25677bb0f8a8cb5a5aad3cd1d608776a4e7",
                        857_078_496, "mistral-small-4-119b.mmproj.gguf",
                    ),
                ],
            ).with_dest("Mistral-Small-4-119B-2603-MXFP4_MOE-00001-of-00003.gguf")
            .with_context(262_144),
            entry(
                "gemma4-26b-a4b-q4", "Gemma 4 26B-A4B (MoE)", "26B total, 4B active", "chat", "Apache-2.0",
                16_947_541_728, 20.0,
                "https://huggingface.co/unsloth/gemma-4-26B-A4B-it-GGUF/resolve/main/gemma-4-26B-A4B-it-UD-Q4_K_M.gguf",
                "f2c28b3dc4776931ac6f879e11f203dec637ea0f14267a86ec8f6165f63f293f",
                "UD-Q4_K_M", "Gemma 4 mixture-of-experts. Large file, but only 4B parameters run per token.", vec![],
            ),
            entry(
                "gemma4-31b-q4", "Gemma 4 31B (dense)", "31B", "chat", "Apache-2.0",
                18_323_733_440, 22.0,
                "https://huggingface.co/unsloth/gemma-4-31B-it-GGUF/resolve/main/gemma-4-31B-it-Q4_K_M.gguf",
                "38bd64c852c4b460434cc7162fa9bdcf242faf86502581a754cb72956bb17f84",
                "Q4_K_M", "Largest dense Gemma 4. Slow on CPU. Accepts images.",
                vec![companion(
                    "https://huggingface.co/unsloth/gemma-4-31B-it-GGUF/resolve/main/mmproj-F16.gguf",
                    "6edcca228213c28d3567a35d22f849eea52d8360875093851959adf5d2f270eb",
                    1_198_957_024, "gemma4-31b-q4.mmproj.gguf",
                )],
            ),
            entry(
                "qwen3.8-27b-q4", "Qwen3.8 27B", "27B", "chat", "Apache-2.0",
                16_564_259_744, 20.0,
                "https://huggingface.co/unsloth/Qwen3.8-27B-GGUF/resolve/main/Qwen3.8-27B-UD-Q4_K_M.gguf",
                "1bce504dfecd84153666928a41d8a66706f0158d3a317bc5196b72819d7dc529",
                "UD-Q4_K_M", "Dense Qwen3.8 model. Accepts images. Slow on CPU.",
                vec![companion(
                    "https://huggingface.co/unsloth/Qwen3.8-27B-GGUF/resolve/main/mmproj-F16.gguf",
                    "cbb841a9ee0636b2ec172f5bb8df2ea8dfeb01e90fe7c6126581d662a0b4e43e",
                    927_607_488, "qwen3.8-27b-q4.mmproj.gguf",
                )],
            ),
            entry(
                "qwen3.6-35b-a3b-q4", "Qwen3.6 35B-A3B (MoE)", "35B total, 3B active", "chat", "Apache-2.0",
                22_134_528_992, 24.0,
                "https://huggingface.co/unsloth/Qwen3.6-35B-A3B-GGUF/resolve/main/Qwen3.6-35B-A3B-UD-Q4_K_M.gguf",
                "ac0e2c1189e055faa36eff361580e79c5bd6f8e76bffb4ce547f167d53e31a61",
                "UD-Q4_K_M", "Qwen3.6 mixture-of-experts. Large file, and slow on 32 GB computers.", vec![],
            ),
            entry(
                "muse-glimmer-30b-q4", "Muse Glimmer 30B", "30B", "chat", "Apache-2.0",
                16_756_683_904, 20.0,
                "https://huggingface.co/meta-models/Muse-Glimmer-30B-GGUF/resolve/main/Muse-Glimmer-30B-KQuant-17GB-Q4_K_M.gguf",
                "4cc57c0f51040a226e5a72cc47b7613f7772950e460a665f7083de89f183f60e",
                "Q4_K_M", "Meta agentic model for local tool use. Needs a recent llama.cpp build.", vec![],
            ),
            // Coding
            entry(
                "qwen3-coder-30b-a3b-q4", "Qwen3 Coder 30B-A3B (MoE)", "30B total, 3B active", "code", "Apache-2.0",
                18_556_689_568, 22.0,
                "https://huggingface.co/unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF/resolve/main/Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf",
                "fadc3e5f8d42bf7e894a785b05082e47daee4df26680389817e2093056f088ad",
                "Q4_K_M", "Code-focused model for writing and reviewing code.", vec![],
            ),
            entry(
                "north-mini-code-q4", "North Mini Code 1.0", "30B total, 3B active", "code", "Apache-2.0",
                19_203_186_784, 22.0,
                "https://huggingface.co/unsloth/North-Mini-Code-1.0-GGUF/resolve/main/North-Mini-Code-1.0-UD-Q4_K_M.gguf",
                "d59f4cb3abec1bd3e5e185b99cdd811e3be3402cbc2bab8223081a460d5724bf",
                "UD-Q4_K_M", "Coding and terminal-task model. Needs llama.cpp v0.9.0 or newer (cohere2moe).", vec![],
            ),
            entry(
                "seed-coder-8b-q4", "Seed-Coder 8B Instruct", "8B", "code", "MIT",
                5_071_410_624, 7.0,
                "https://huggingface.co/unsloth/Seed-Coder-8B-Instruct-GGUF/resolve/main/Seed-Coder-8B-Instruct-Q4_K_M.gguf",
                "cb85a42e65b1efda94d0b341331f647db2dc46539f262d996136d65fc7c4d1fa",
                "Q4_K_M", "ByteDance code model for writing, editing and completing code. Fits 16 GB computers.", vec![],
            ),
            entry(
                "devstral-small-2-24b-q4", "Devstral Small 2 24B", "24B", "code", "Other (check model card)",
                14_334_446_752, 18.0,
                "https://huggingface.co/unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF/resolve/main/Devstral-Small-2-24B-Instruct-2512-Q4_K_M.gguf",
                "d14ba9edee1bb4c4996a726deb81e49ae81800a3216f0774634238c380aee496",
                "Q4_K_M", "Mistral agentic coding model for multi-file tasks. Runs on a 32 GB Mac. License tag is 'other'; read the card before shipping.", vec![],
            ),
            // Embedding and reranking
            entry(
                "embeddinggemma-300m-q8", "EmbeddingGemma 300M", "300M", "embedding", "Gemma Terms",
                333_590_944, 1.0,
                "https://huggingface.co/ggml-org/embeddinggemma-300M-GGUF/resolve/main/embeddinggemma-300M-Q8_0.gguf",
                "b5ce9d77a3fc4b3b39ccb5643c36777911cc4eb46a66962eadfa3f5f60490d63",
                "Q8_0", "Google embedding model. Small and fast for library search.", vec![],
            ),
            entry(
                "embeddinggemma-300m-qat-q4", "EmbeddingGemma 300M (QAT)", "300M", "embedding", "Gemma Terms",
                277_852_192, 0.8,
                "https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q4_0-GGUF/resolve/main/embeddinggemma-300M-qat-Q4_0.gguf",
                "50d28e22432a148f6f8a86eab3700f92add5d1f54baf7790675a2a4dadbccf26",
                "Q4_0 QAT", "Smallest EmbeddingGemma. Good for low-RAM machines.", vec![],
            ),
            entry(
                "qwen3-embedding-0.6b-q8", "Qwen3 Embedding 0.6B", "0.6B", "embedding", "Apache-2.0",
                639_150_592, 1.0,
                "https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf",
                "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439",
                "Q8_0", "Embedding model for library and memory search.", vec![],
            ),
            entry(
                "nomic-embed-v2-moe-q8", "Nomic Embed Text v2 MoE", "475M", "embedding", "Apache-2.0",
                512_225_120, 0.7,
                "https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe-GGUF/resolve/main/nomic-embed-text-v2-moe.Q8_0.gguf",
                "06e7a7e594a26985523c18383aba4aad39fe6e14f08ffc6ab5b554e1ccdc3cff",
                "Q8_0", "Official llama.cpp quantization. Multilingual mixture-of-experts embeddings with Matryoshka dimensions.", vec![],
            ),
            entry(
                "qwen3-embedding-4b-q4", "Qwen3 Embedding 4B", "4B", "embedding", "Apache-2.0",
                2_496_703_776, 3.6,
                "https://huggingface.co/Qwen/Qwen3-Embedding-4B-GGUF/resolve/main/Qwen3-Embedding-4B-Q4_K_M.gguf",
                "2b0cf8f17b4c723c27303015383c27ec4bf2d8314bb677d05e920dd70bb0f16b",
                "Q4_K_M", "Larger embedding model with better recall. Slower to index.", vec![],
            ),
            entry(
                "qwen3-reranker-0.6b-q8", "Qwen3 Reranker 0.6B", "0.6B", "reranker", "Apache-2.0",
                639_153_184, 1.0,
                "https://huggingface.co/ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF/resolve/main/qwen3-reranker-0.6b-q8_0.gguf",
                "22c9979ce4fbcdc5acdc310c6641c32797eff1aa980b8f7a2db8a8ea23429a48",
                "Q8_0", "Re-ranks search results for better top answers.", vec![],
            ),
        ];

        let signature = Self::compute_signature(&models);
        Self {
            version: 8,
            updated_at: 1795027200,
            signature,
            models,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn entry(
    id: &str,
    name: &str,
    params: &str,
    kind: &str,
    license: &str,
    file_size: u64,
    ram_estimate_gb: f32,
    url: &str,
    sha256: &str,
    quant: &str,
    description: &str,
    companions: Vec<CatalogCompanion>,
) -> CatalogEntry {
    CatalogEntry {
        id: id.into(),
        name: name.into(),
        params: params.into(),
        kind: kind.into(),
        license: license.into(),
        file_size,
        ram_estimate_gb,
        url: url.into(),
        sha256: sha256.into(),
        quant: quant.into(),
        description: description.into(),
        context_tokens: None,
        dest: None,
        companions,
    }
}

impl CatalogEntry {
    /// Attach a native context window hint to a freshly built entry.
    fn with_context(mut self, tokens: u32) -> Self {
        self.context_tokens = Some(tokens);
        self
    }

    /// Name the primary file explicitly — required for split GGUFs so the
    /// shard pattern survives on disk.
    fn with_dest(mut self, dest: &str) -> Self {
        self.dest = Some(dest.into());
        self
    }

    /// Per-entry structural checks, shared by manifest validation and by the
    /// community-model registry (whose entries never pass through a manifest).
    pub fn validate_entry(&self) -> Result<(), AppError> {
        if self.name.is_empty() || self.params.is_empty() || self.quant.is_empty() {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' has an empty name, params, or quant label",
                self.id
            )));
        }
        if !KNOWN_KINDS.contains(&self.kind.as_str()) {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' has unknown kind '{}'",
                self.id, self.kind
            )));
        }
        if !is_sha256_hex(&self.sha256) {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' has a malformed SHA-256",
                self.id
            )));
        }
        if self.file_size == 0 {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' reports zero file size",
                self.id
            )));
        }
        if !(0.1..=512.0).contains(&self.ram_estimate_gb) {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' has an implausible RAM estimate ({})",
                self.id, self.ram_estimate_gb
            )));
        }
        if let Some(ctx) = self.context_tokens
            && !(512..=1_048_576).contains(&ctx)
        {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' has an implausible context hint ({ctx})",
                self.id
            )));
        }
        if let Some(dest) = &self.dest
            && (dest.is_empty()
                || dest.contains('/')
                || dest.contains('\\')
                || dest.contains("..")
                || !dest.ends_with(".gguf"))
        {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' has an unsafe dest '{}': it must be a plain .gguf file name",
                self.id, dest
            )));
        }
        require_https(&self.url, "model download")?;
        if self.companions.len() > MAX_COMPANIONS_PER_MODEL {
            return Err(AppError::invalid_input(format!(
                "Catalog model '{}' lists {} companions; the limit is {MAX_COMPANIONS_PER_MODEL}",
                self.id,
                self.companions.len()
            )));
        }

        let mut seen_dests = std::collections::HashSet::new();
        for c in &self.companions {
            require_https(&c.url, "companion download")?;
            if !is_sha256_hex(&c.sha256) {
                return Err(AppError::invalid_input(format!(
                    "Companion of '{}' has a malformed SHA-256",
                    self.id
                )));
            }
            if c.file_size == 0 {
                return Err(AppError::invalid_input(format!(
                    "Companion of '{}' reports zero file size",
                    self.id
                )));
            }
            if c.dest.is_empty() || !seen_dests.insert(c.dest.as_str()) {
                return Err(AppError::invalid_input(format!(
                    "Companion of '{}' has an empty or duplicated destination name",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

fn companion(url: &str, sha256: &str, file_size: u64, dest: &str) -> CatalogCompanion {
    CatalogCompanion {
        url: url.into(),
        sha256: sha256.into(),
        file_size,
        dest: dest.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(id: &str) -> CatalogEntry {
        CatalogEntry {
            id: id.into(),
            name: "Test".into(),
            params: "1B".into(),
            kind: "chat".into(),
            license: "Apache-2.0".into(),
            file_size: 1_000_000,
            ram_estimate_gb: 1.0,
            url: "https://huggingface.co/example/test/resolve/main/test.gguf".into(),
            sha256: "aa".repeat(32),
            quant: "Q4_K_M".into(),
            description: "test".into(),
            companions: vec![],
            context_tokens: None,
            dest: None,
        }
    }

    fn sample_manifest(version: u32) -> CatalogManifest {
        CatalogManifest {
            version,
            updated_at: 1_794_499_200,
            signature: String::new(),
            models: vec![sample_entry("test-1b")],
        }
    }

    #[test]
    fn embedded_manifest_passes_validation() {
        let m = CatalogManifest::default_manifest();
        m.verify().unwrap();
        m.validate(m.version).unwrap();
    }

    #[test]
    fn validation_rejects_catalog_rollback() {
        let embedded_version = CatalogManifest::default_manifest().version;
        assert!(
            sample_manifest(embedded_version - 1)
                .validate(embedded_version)
                .is_err()
        );
        assert!(
            sample_manifest(embedded_version)
                .validate(embedded_version)
                .is_ok()
        );
        assert!(
            sample_manifest(embedded_version + 1)
                .validate(embedded_version)
                .is_ok()
        );
    }

    #[test]
    fn validation_rejects_bad_entries() {
        let embedded_version = CatalogManifest::default_manifest().version;

        // Duplicate model id
        let mut m = sample_manifest(embedded_version + 1);
        m.models.push(sample_entry("test-1b"));
        assert!(m.validate(0).is_err());

        // Plain-HTTP download URL
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].url = "http://huggingface.co/example/test.gguf".into();
        assert!(m.validate(0).is_err());

        // Credentials embedded in the URL
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].url = "https://user:pass@huggingface.co/example/test.gguf".into();
        assert!(m.validate(0).is_err());

        // Malformed SHA-256
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].sha256 = "zz".repeat(32);
        assert!(m.validate(0).is_err());

        // Zero file size
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].file_size = 0;
        assert!(m.validate(0).is_err());

        // Unknown kind
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].kind = "agent".into();
        assert!(m.validate(0).is_err());

        // Implausible RAM estimate
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].ram_estimate_gb = 900.0;
        assert!(m.validate(0).is_err());

        // Companion with a duplicate destination name
        let mut m = sample_manifest(embedded_version + 1);
        m.models[0].companions = vec![
            CatalogCompanion {
                url: m.models[0].url.clone(),
                sha256: m.models[0].sha256.clone(),
                file_size: 100,
                dest: "mmproj.gguf".into(),
            },
            CatalogCompanion {
                url: m.models[0].url.clone(),
                sha256: m.models[0].sha256.clone(),
                file_size: 100,
                dest: "mmproj.gguf".into(),
            },
        ];
        assert!(m.validate(0).is_err());
    }

    /// Parity guard: the JSON that `model_catalog_refresh` fetches from the
    /// project repository and the list compiled into the app must stay
    /// identical. Regenerate `catalog/models.json` whenever
    /// `default_manifest()` changes (version bump included), otherwise this
    /// test fails and refreshed installs would diverge from built-in ones.
    #[test]
    fn catalog_json_matches_embedded_manifest() {
        let raw = include_str!("../../../../catalog/models.json");
        let parsed: CatalogManifest =
            serde_json::from_str(raw).expect("catalog/models.json parses");
        assert_eq!(parsed, CatalogManifest::default_manifest());
        parsed.verify().unwrap();
        parsed.validate(parsed.version).unwrap();
    }
}
