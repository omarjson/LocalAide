use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub selected_models: Vec<String>,
    pub presets: Vec<UserPreset>,
    pub ram_budget_mb: u64,
    pub cloud_llm_enabled: bool,
    /// Opt-in OpenAI-compatible providers. Empty unless the user adds one.
    #[serde(default)]
    pub cloud_providers: Vec<CloudProvider>,
    /// Provider used when `cloud_llm_enabled` is true. None = local engine.
    #[serde(default)]
    pub active_cloud_provider: Option<String>,
    /// Context window the local engine is allowed to allocate. Capped per
    /// model by the catalog's native context hint when one exists.
    #[serde(default = "default_context_tokens")]
    pub context_tokens: u32,
    /// Chat preferences the user last chose, restored on the next session.
    /// All fields are optional so old config files keep loading.
    #[serde(default)]
    pub chat_system_prompt: Option<String>,
    #[serde(default)]
    pub chat_temperature: Option<f32>,
    #[serde(default)]
    pub chat_max_tokens: Option<u32>,
    #[serde(default)]
    pub chat_persona: Option<String>,
    /// Encrypted local backup settings (ADR-0009). The passphrase is never
    /// stored, so backups are user-triggered, not scheduled.
    #[serde(default)]
    pub backup: BackupConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BackupConfig {
    /// Where archives are written. None = ~/.aiassistant/backups.
    #[serde(default)]
    pub target_dir: Option<String>,
    /// Include provider API keys in archives (default off).
    #[serde(default)]
    pub include_api_keys: bool,
    #[serde(default)]
    pub last_backup_at: Option<u64>,
}

fn default_context_tokens() -> u32 {
    8192
}

/// One user-configured OpenAI-compatible provider (OpenAI, OpenRouter,
/// Together, a localhost proxy such as Ollama or LM Studio, and so on).
///
/// The API key is stored in the app config file on this machine only. It is
/// sent as a bearer token to `base_url` and nowhere else.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloudProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreset {
    pub id: String,
    pub name: String,
    pub model_ids: Vec<String>,
    pub total_ram_gb: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            selected_models: vec![],
            presets: vec![],
            ram_budget_mb: 4096,
            cloud_llm_enabled: false,
            cloud_providers: vec![],
            active_cloud_provider: None,
            context_tokens: default_context_tokens(),
            chat_system_prompt: None,
            chat_temperature: None,
            chat_max_tokens: None,
            chat_persona: None,
            backup: BackupConfig::default(),
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigStore {
    pub fn new() -> Self {
        Self {
            path: config_path(),
        }
    }

    pub fn load(&self) -> Result<AppConfig, String> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }
        let data = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    }

    pub fn save(&self, cfg: &AppConfig) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
        fs::write(&self.path, data).map_err(|e| e.to_string())
    }

    pub fn compute_total_ram_gb(
        &self,
        model_ids: &[String],
        catalog: &[crate::model_manager::ModelEntry],
    ) -> f32 {
        model_ids
            .iter()
            .filter_map(|id| {
                catalog
                    .iter()
                    .find(|m| &m.id == id)
                    .map(|m| m.ram_estimate_gb)
            })
            .sum()
    }
}

/// Require HTTPS unless the URL points at the user's own machine, where plain
/// HTTP proxies (Ollama, LM Studio, llama-server itself) are legitimate.
pub fn validate_base_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| {
        "The base URL is not a valid URL. Use https://provider.example/v1.".to_string()
    })?;
    // Credentials in the URL would leak into provider error text (reqwest
    // echoes the request URL); the API key belongs in the header only.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Do not put a username or password in the base URL.".to_string());
    }
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = parsed.host_str().unwrap_or_default();
            let loopback = matches!(
                host,
                "127.0.0.1" | "localhost" | "::1" | "[::1]" | "0.0.0.0"
            ) || host.starts_with("127.");
            if loopback {
                Ok(())
            } else {
                Err("Plain HTTP is only allowed for addresses on this machine (127.0.0.1 or localhost). Use HTTPS for any remote provider.".to_string())
            }
        }
        _ => Err(
            "The base URL must start with https:// (or http:// for a proxy on this machine)."
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_cloud_providers() {
        let mut cfg = AppConfig::default();
        cfg.cloud_providers.push(CloudProvider {
            id: "p1".into(),
            label: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            api_key: "sk-test".into(),
        });
        cfg.active_cloud_provider = Some("p1".into());
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cloud_providers.len(), 1);
        assert_eq!(back.cloud_providers[0].api_key, "sk-test");
        assert_eq!(back.active_cloud_provider.as_deref(), Some("p1"));
    }

    #[test]
    fn config_from_older_version_defaults_new_fields() {
        let old =
            r#"{"selected_models":[],"presets":[],"ram_budget_mb":4096,"cloud_llm_enabled":false}"#;
        let cfg: AppConfig = serde_json::from_str(old).unwrap();
        assert!(cfg.cloud_providers.is_empty());
        assert!(cfg.active_cloud_provider.is_none());
    }

    #[test]
    fn https_urls_are_accepted() {
        assert!(validate_base_url("https://api.openai.com/v1").is_ok());
    }

    #[test]
    fn plain_http_is_loopback_only() {
        assert!(validate_base_url("http://127.0.0.1:11434/v1").is_ok());
        assert!(validate_base_url("http://localhost:1234/v1").is_ok());
        assert!(validate_base_url("http://api.example.com/v1").is_err());
        assert!(validate_base_url("ftp://api.example.com").is_err());
        assert!(validate_base_url("not a url").is_err());
    }

    #[test]
    fn userinfo_in_urls_is_rejected() {
        assert!(validate_base_url("https://user:pass@api.example.com/v1").is_err());
        assert!(validate_base_url("https://user:pass@127.0.0.1:11434/v1").is_err());
    }
}
