use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Older config files also hold `presets` and `ram_budget_mb`. Nothing
    // used them, so they are no longer read; serde skips unknown fields.
    pub selected_models: Vec<String>,
    pub cloud_llm_enabled: bool,
    /// Opt-in OpenAI-compatible providers. Empty unless the user adds one.
    #[serde(default)]
    pub cloud_providers: Vec<CloudProvider>,
    /// Master switch for online providers (Settings → Online providers).
    /// Off: no provider is offered or contacted for chat. None (older
    /// configs) = on when the user has added a provider, since adding one
    /// was the explicit opt-in before this switch existed.
    #[serde(default)]
    pub online_allowed: Option<bool>,
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
    /// Web search settings (Settings → Web search). API keys are stored in
    /// the local config file only, sent only to their own provider, and
    /// excluded from backups unless explicitly included.
    #[serde(default)]
    pub web_search: WebSearchConfig,
    /// UI theme: Some("dark") | Some("light"). None = dark (the default).
    #[serde(default)]
    pub theme: Option<String>,
    /// Share of this computer's memory, after a reserve for the system, that
    /// models may use, in percent (Settings → Memory for models). None = 70.
    #[serde(default)]
    pub model_memory_percent: Option<u8>,
    /// Link to an OpenCode server the user runs (Settings → Integrations).
    #[serde(default)]
    pub opencode: OpenCodeConfig,
    /// Developer mode, experimental flags and which modules are on
    /// (Settings → Developer).
    #[serde(default)]
    pub developer: DeveloperConfig,
    /// The model chosen in the chat's model picker for the active online
    /// provider. None = the provider's own default model.
    #[serde(default)]
    pub active_cloud_model: Option<String>,
    /// The embedding ("meaning") model library search uses. Started at
    /// launch when installed. None = keyword search until the user picks one.
    #[serde(default)]
    pub embedding_model: Option<String>,
    /// Voice input and read-aloud (Settings → Voice). Off by default.
    #[serde(default)]
    pub voice: VoiceConfig,
    /// Starred models in the picker, as "local:<id>" or "<provider id>:<model>".
    #[serde(default)]
    pub favorite_models: Vec<String>,
    /// MCP servers the user added (Settings → Tools and MCP). Their tools
    /// are offered to the model in chat while the server is switched on.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// One MCP server. A local server is a program LocalAide starts; a remote
/// one is an address it posts to. Header values and environment values may
/// hold tokens, so backups leave them out unless keys are included.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Run this server's tools without asking each time.
    #[serde(default)]
    pub auto_approve: bool,
    pub transport: McpTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransport {
    /// A program on this computer, spoken to over its standard input and output.
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: std::collections::BTreeMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    /// A server reached over Streamable HTTP.
    Http {
        url: String,
        #[serde(default)]
        headers: std::collections::BTreeMap<String, String>,
    },
}

fn default_true() -> bool {
    true
}

/// Default and limits for `AppConfig::model_memory_percent`.
pub const DEFAULT_MODEL_MEMORY_PERCENT: u8 = 70;
pub const MIN_MODEL_MEMORY_PERCENT: u8 = 20;
pub const MAX_MODEL_MEMORY_PERCENT: u8 = 90;

impl AppConfig {
    /// Whether online providers may be used at all.
    pub fn online_allowed(&self) -> bool {
        self.online_allowed
            .unwrap_or(!self.cloud_providers.is_empty())
    }

    /// The memory share for models, kept inside the allowed range.
    pub fn memory_percent_for_models(&self) -> u8 {
        self.model_memory_percent
            .unwrap_or(DEFAULT_MODEL_MEMORY_PERCENT)
            .clamp(MIN_MODEL_MEMORY_PERCENT, MAX_MODEL_MEMORY_PERCENT)
    }
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WebSearchConfig {
    /// Provider used when the user searches without picking one. None =
    /// Automatic (DuckDuckGo, then Exa, then Wikipedia; all keyless).
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub exa_api_key: Option<String>,
    #[serde(default)]
    pub firecrawl_api_key: Option<String>,
    /// A Firecrawl server on this computer, for example
    /// `http://localhost:3002`. None = Firecrawl's own service.
    #[serde(default)]
    pub firecrawl_base_url: Option<String>,
}

/// Where LocalAide finds the user's OpenCode server. None = not linked.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OpenCodeConfig {
    #[serde(default)]
    pub base_url: Option<String>,
    /// The server's OPENCODE_SERVER_PASSWORD, when it has one. Left out of
    /// backups unless keys are included.
    #[serde(default)]
    pub password: Option<String>,
}

/// Developer mode. `flags` turn experimental features on; `modules` turn
/// whole parts of the app off (a missing entry means on).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DeveloperConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub flags: std::collections::BTreeMap<String, bool>,
    #[serde(default)]
    pub modules: std::collections::BTreeMap<String, bool>,
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
    /// Shown in the chat's model picker. Several providers can be on at once.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// The provider's model list from the last "Load models", kept so the
    /// picker can show it without asking the provider again.
    #[serde(default)]
    pub models: Vec<crate::services::inference::provider_models::ProviderModel>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            selected_models: vec![],
            cloud_llm_enabled: false,
            cloud_providers: vec![],
            online_allowed: None,
            active_cloud_provider: None,
            context_tokens: default_context_tokens(),
            chat_system_prompt: None,
            chat_temperature: None,
            chat_max_tokens: None,
            chat_persona: None,
            backup: BackupConfig::default(),
            web_search: WebSearchConfig::default(),
            theme: None,
            model_memory_percent: None,
            opencode: OpenCodeConfig::default(),
            developer: DeveloperConfig::default(),
            active_cloud_model: None,
            embedding_model: None,
            voice: VoiceConfig::default(),
            favorite_models: Vec::new(),
            mcp_servers: Vec::new(),
        }
    }
}

/// Settings → Voice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoiceConfig {
    /// The microphone button in chat. Off until the user turns it on.
    #[serde(default)]
    pub enabled: bool,
    /// The transcription model: a local catalog id or "provider:model".
    #[serde(default = "default_voice_model")]
    pub model: String,
    /// Block every network use for voice: only local models run.
    #[serde(default = "default_true")]
    pub local_only: bool,
    /// Tidy the transcript with the local chat model before it is sent.
    #[serde(default)]
    pub cleanup: bool,
    /// Send the message as soon as the recording is transcribed.
    #[serde(default = "default_true")]
    pub auto_send: bool,
    /// Read answers aloud with the operating system's voice.
    #[serde(default)]
    pub read_aloud: bool,
    /// The system voice's name; None = the system default.
    #[serde(default)]
    pub voice_name: Option<String>,
    /// Speaking speed, 0.5 to 2.
    #[serde(default = "default_rate")]
    pub rate: f32,
    /// Keys for online transcription providers, by provider id. Stored in
    /// this file like the chat provider keys (0600, atomic write).
    #[serde(default)]
    pub keys: std::collections::BTreeMap<String, String>,
}

fn default_voice_model() -> String {
    "stt-whisper-tiny".into()
}
fn default_rate() -> f32 {
    1.0
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: default_voice_model(),
            local_only: true,
            cleanup: false,
            auto_send: true,
            read_aloud: false,
            voice_name: None,
            rate: 1.0,
            keys: Default::default(),
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

    /// Writes the config atomically. The new file is written to a temp file in
    /// full, then renamed over the real one, so a crash mid-write keeps the old
    /// config. The file holds API keys, so it is owner-only on Unix.
    pub fn save(&self, cfg: &AppConfig) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, data).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

/// True when the URL's host is this machine: the name `localhost`, or a
/// loopback IP address. Comparing the parsed host means `127.evil.com` and
/// `0.0.0.0` do not pass.
pub fn is_local_host(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
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
            if is_local_host(&parsed) {
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
            enabled: true,
            models: Vec::new(),
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
        assert_eq!(
            cfg.memory_percent_for_models(),
            DEFAULT_MODEL_MEMORY_PERCENT
        );
    }

    #[test]
    fn memory_share_stays_in_range() {
        let with = |percent: u8| AppConfig {
            model_memory_percent: Some(percent),
            ..AppConfig::default()
        };
        assert_eq!(
            with(5).memory_percent_for_models(),
            MIN_MODEL_MEMORY_PERCENT
        );
        assert_eq!(
            with(100).memory_percent_for_models(),
            MAX_MODEL_MEMORY_PERCENT
        );
        assert_eq!(with(55).memory_percent_for_models(), 55);
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
    fn loopback_check_uses_the_parsed_host() {
        assert!(validate_base_url("http://[::1]:11434/v1").is_ok());
        assert!(validate_base_url("http://127.0.0.2:8080/v1").is_ok());
        assert!(validate_base_url("http://127.evil.com:8080/v1").is_err());
        assert!(validate_base_url("http://127.0.0.1.nip.io/v1").is_err());
        assert!(validate_base_url("http://0.0.0.0:8080/v1").is_err());
        assert!(validate_base_url("http://localhost.evil.com/v1").is_err());
    }

    #[test]
    fn save_replaces_the_file_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("la-config-save-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = ConfigStore {
            path: dir.join("config.json"),
        };
        let mut cfg = AppConfig::default();
        store.save(&cfg).unwrap();
        cfg.context_tokens = 4096;
        store.save(&cfg).unwrap();
        assert_eq!(store.load().unwrap().context_tokens, 4096);
        assert!(!dir.join("config.json.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn userinfo_in_urls_is_rejected() {
        assert!(validate_base_url("https://user:pass@api.example.com/v1").is_err());
        assert!(validate_base_url("https://user:pass@127.0.0.1:11434/v1").is_err());
    }
}
