use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::config_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Older config files also hold `presets` and `ram_budget_mb`. Nothing
    // used them, so they are no longer read; serde skips unknown fields.
    pub selected_models: Vec<String>,
    pub cloud_llm_enabled: bool,
    /// True once the setup wizard has finished or been skipped. The startup
    /// gate must not depend on `selected_models` alone: an online-only user
    /// has no local models and would otherwise see setup on every launch.
    #[serde(default)]
    pub setup_complete: bool,
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
    /// Tool permission rules, evaluated in order; the last matching rule wins.
    #[serde(default)]
    pub permissions: Vec<PermissionRule>,
    /// Optional shell hooks run around tool calls.
    #[serde(default)]
    pub hooks: HooksConfig,
    /// Provider failover order and per-provider API key pools.
    #[serde(default)]
    pub routing: RoutingConfig,
    /// Per-capability auto-approve (Settings → Permissions). Defaults to
    /// read-only: only reads run without asking. Unknown or missing config
    /// means ask, never auto-approve.
    #[serde(default)]
    pub auto_approve_capabilities: CapabilityAutoApprove,
    /// Shorter system prompt for local models (Settings → Performance).
    /// Off by default: the full prompt is sent unless the user turns this on.
    #[serde(default)]
    pub use_compact_prompt: bool,
}

/// What a matching permission rule says to do with a tool call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

/// The backend's view of a tool decision. `Ask` keeps the existing
/// Allow Once / Allow for this session / Deny approval flow in place.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecision {
    Allow,
    Ask,
    Deny,
}

/// One capability a tool call can belong to, for per-capability
/// auto-approve (N9). Serialized snake_case for config.json.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadFiles,
    EditFiles,
    SafeCommands,
    AllCommands,
    Browser,
    ExternalTools,
}

/// Which capabilities run without asking. Default is read-only: only
/// reads are auto-approved, everything else asks first. A missing field
/// in an old config file falls back to the same default, so an unknown
/// or missing config never auto-approves a change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CapabilityAutoApprove {
    #[serde(default = "default_true")]
    pub read_files: bool,
    #[serde(default)]
    pub edit_files: bool,
    #[serde(default)]
    pub safe_commands: bool,
    #[serde(default)]
    pub all_commands: bool,
    #[serde(default)]
    pub browser: bool,
    #[serde(default)]
    pub external_tools: bool,
}

impl Default for CapabilityAutoApprove {
    fn default() -> Self {
        Self {
            read_files: true,
            edit_files: false,
            safe_commands: false,
            all_commands: false,
            browser: false,
            external_tools: false,
        }
    }
}

impl CapabilityAutoApprove {
    /// Whether this capability runs without asking. "All commands"
    /// covers safe commands too, so a safe command is auto-approved
    /// when either toggle is on.
    pub fn approves(&self, capability: Capability) -> bool {
        match capability {
            Capability::ReadFiles => self.read_files,
            Capability::EditFiles => self.edit_files,
            Capability::SafeCommands => self.safe_commands || self.all_commands,
            Capability::AllCommands => self.all_commands,
            Capability::Browser => self.browser,
            Capability::ExternalTools => self.external_tools,
        }
    }
}

impl From<PermissionDecision> for ToolDecision {
    fn from(value: PermissionDecision) -> Self {
        match value {
            PermissionDecision::Allow => ToolDecision::Allow,
            PermissionDecision::Ask => ToolDecision::Ask,
            PermissionDecision::Deny => ToolDecision::Deny,
        }
    }
}

/// One policy rule. `action` and `resource` are glob patterns where `*` matches
/// any run of characters. Rules are evaluated in order and the last match wins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    pub action: String,
    pub resource: String,
    pub decision: PermissionDecision,
}

impl PermissionRule {
    /// True when this rule applies to the given tool action and resource.
    pub fn matches(&self, action: &str, resource: &str) -> bool {
        wildcard_match(&self.action, action) && wildcard_match(&self.resource, resource)
    }
}

/// Glob-style match: `*` matches zero or more characters, all other characters
/// match literally. Pure and allocation-light enough for a handful of rules.
pub fn wildcard_match(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = value.chars().collect();
    let (mut pi, mut si) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut retry = 0usize;
    while si < s.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            retry = si;
            pi += 1;
        } else if pi < p.len() && p[pi] == s[si] {
            pi += 1;
            si += 1;
        } else if let Some(star_idx) = star {
            pi = star_idx + 1;
            retry += 1;
            si = retry;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// The last rule that matches, if any. Later entries override earlier ones.
pub fn matching_rule<'a>(
    rules: &'a [PermissionRule],
    action: &str,
    resource: &str,
) -> Option<&'a PermissionRule> {
    rules.iter().rev().find(|r| r.matches(action, resource))
}

/// Optional shell hooks around a tool call. Commands run from the workspace
/// root with a timeout; a non-zero pre-tool exit blocks the call.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HooksConfig {
    /// Run before a tool call. A non-zero exit (or timeout) blocks it.
    #[serde(default)]
    pub pre_tool: Option<String>,
    /// Run after a tool call. Its result is informational, never a block.
    #[serde(default)]
    pub post_tool: Option<String>,
    /// Seconds before a hook is killed. Defaults to 10 when absent or zero.
    #[serde(default)]
    pub timeout_secs: u64,
}

impl HooksConfig {
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(if self.timeout_secs == 0 {
            10
        } else {
            self.timeout_secs
        })
    }
}

/// Provider failover order and per-provider key pools. Both are optional, so
/// configs written before this feature keep their old behaviour.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RoutingConfig {
    /// Provider ids to try first, in order. Empty = enabled config order.
    #[serde(default)]
    pub provider_priority: Vec<String>,
    /// Extra keys per provider id, tried after the primary key on 401/429.
    #[serde(default)]
    pub key_pools: BTreeMap<String, Vec<String>>,
}

/// A cursor over one provider's keys. The router rotates to the next key when
/// the provider answers 401 (bad key) or 429 (rate limited).
#[derive(Debug, Clone, Default)]
pub struct CredentialPool {
    keys: Vec<String>,
    index: usize,
}

impl CredentialPool {
    pub fn new(keys: Vec<String>) -> Self {
        Self { keys, index: 0 }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The key to use for the next request, if any.
    pub fn current(&self) -> Option<&str> {
        self.keys.get(self.index).map(String::as_str)
    }

    /// Move to the next key after a 401/429. Returns true only when a
    /// rotation actually happened (two or more keys and a retryable status).
    pub fn on_auth_failure(&mut self, status: u16) -> bool {
        if (status == 401 || status == 429) && self.keys.len() > 1 {
            self.index = (self.index + 1) % self.keys.len();
            true
        } else {
            false
        }
    }
}

/// One model's price, USD per 1M tokens. Cache reads are usually cheaper;
/// None = the provider does not discount cached input.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ModelPrice {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub cache_read: Option<f64>,
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
    /// When non-empty, only these tool names are exposed to the model.
    #[serde(default)]
    pub enabled_tools: Vec<String>,
    /// These tool names are never exposed, even if listed in `enabled_tools`.
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    pub transport: McpTransport,
    /// OAuth 2.1 / PKCE sign-in settings for an HTTP server. Optional so
    /// config files written before this feature keep loading unchanged; the
    /// access token itself never lives here, only public client metadata.
    #[serde(default)]
    pub oauth: Option<McpOAuthConfig>,
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

/// OAuth 2.1 / PKCE sign-in settings for an MCP server (ADR-0012). Stored on
/// `McpServerConfig` as an optional field, so older config files and every
/// existing struct literal keep working. The redirect is a loopback address
/// the user copies the code from during sign-in; no secret is stored here,
/// only public client metadata. Missing fields default to empty so a client
/// that omits them cannot break config parsing; `mcp_server_save` validates
/// them before anything is persisted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct McpOAuthConfig {
    /// The MCP server id this sign-in belongs to.
    pub server_id: String,
    /// Public OAuth client id issued by the authorization server.
    pub client_id: String,
    /// Authorization endpoint the user's browser is opened to.
    pub authorize_url: String,
    /// Token endpoint used for the code exchange.
    pub token_url: String,
    /// Loopback redirect URI registered with the authorization server.
    pub redirect_uri: String,
    /// Optional scopes to request.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Optional resource/audience hint some authorization servers require.
    #[serde(default)]
    pub audience: Option<String>,
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

    /// The permission rule that applies to this tool call, if any.
    pub fn permission_for<'a>(
        &'a self,
        action: &str,
        resource: &str,
    ) -> Option<&'a PermissionRule> {
        matching_rule(&self.permissions, action, resource)
    }

    /// Decide a tool call from the configured rules. With no matching rule the
    /// decision is `Ask`, preserving the existing approval flow.
    pub fn decide_tool_call(&self, action: &str, resource: &str) -> ToolDecision {
        self.permission_for(action, resource)
            .map(|rule| ToolDecision::from(rule.decision))
            .unwrap_or(ToolDecision::Ask)
    }

    /// Provider ids in failover order. Empty priority list falls back to the
    /// config order of enabled providers, so older configs behave as before.
    pub fn provider_order(&self) -> Vec<String> {
        if !self.routing.provider_priority.is_empty() {
            let ordered: Vec<String> = self
                .routing
                .provider_priority
                .iter()
                .filter(|id| {
                    self.cloud_providers
                        .iter()
                        .any(|p| &p.id == *id && p.enabled)
                })
                .cloned()
                .collect();
            if !ordered.is_empty() {
                return ordered;
            }
            // An explicit priority list is honoured even before the matching
            // providers have been loaded into the config.
            return self.routing.provider_priority.clone();
        }
        self.cloud_providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.id.clone())
            .collect()
    }

    /// All keys for one provider: the primary key first, then the pool.
    pub fn credentials_for(&self, provider_id: &str) -> CredentialPool {
        let mut keys = self
            .routing
            .key_pools
            .get(provider_id)
            .cloned()
            .unwrap_or_default();
        if let Some(provider) = self.cloud_providers.iter().find(|p| p.id == provider_id)
            && !provider.api_key.trim().is_empty()
            && !keys.contains(&provider.api_key)
        {
            keys.insert(0, provider.api_key.clone());
        }
        CredentialPool::new(keys)
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

impl DeveloperConfig {
    /// A module that has no stored entry is on. Used by the chat prompt
    /// assembly to skip optional work (project context files, recall, ...)
    /// when that module has been switched off in Settings → Developer.
    pub fn module_on(&self, id: &str) -> bool {
        self.modules.get(id).copied().unwrap_or(true)
    }
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
    /// The last connection check (Test connection or Load models).
    #[serde(default)]
    pub check: Option<ProviderCheck>,
}

/// The result of the last time LocalAide reached a provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderCheck {
    pub ok: bool,
    /// Seconds since 1970.
    pub at: u64,
    pub model_count: usize,
    /// Plain-words reason when it failed.
    #[serde(default)]
    pub error: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            selected_models: vec![],
            cloud_llm_enabled: false,
            setup_complete: false,
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
            permissions: Vec::new(),
            hooks: HooksConfig::default(),
            routing: RoutingConfig::default(),
            auto_approve_capabilities: CapabilityAutoApprove::default(),
            use_compact_prompt: false,
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

/// Pure dirty check for config writes. True when `next` differs from the
/// last serialized snapshot; `None` means nothing has been written yet.
pub fn snapshot_is_dirty(last_written: Option<&str>, next: &str) -> bool {
    last_written != Some(next)
}

/// Merge provider entries that are the same provider added twice: same
/// address and same name. The Kilo preset, added once per setup run, came
/// back as three "Kilo Code" rows in the model menu, each with its own
/// model list. The first entry keeps its id, so the active provider,
/// favourites and keychain references stay valid; a later entry contributes
/// only models the first one lacks, and the first non-empty key wins. Two
/// routers on the same address keep separate entries when their names
/// differ (9Router vs OmniRoute). Returns true when anything merged.
pub fn dedupe_cloud_providers(cfg: &mut AppConfig) -> bool {
    let mut kept: Vec<CloudProvider> = Vec::new();
    let mut dropped: Vec<(String, String)> = Vec::new();
    for provider in cfg.cloud_providers.drain(..) {
        let key = (
            provider
                .base_url
                .trim()
                .trim_end_matches('/')
                .to_ascii_lowercase(),
            provider.label.trim().to_ascii_lowercase(),
        );
        if let Some(existing) = kept.iter_mut().find(|p: &&mut CloudProvider| {
            (
                p.base_url.trim().trim_end_matches('/').to_ascii_lowercase(),
                p.label.trim().to_ascii_lowercase(),
            ) == key
        }) {
            for model in provider.models {
                if !existing.models.iter().any(|m| m.id == model.id) {
                    existing.models.push(model);
                }
            }
            if existing.api_key.trim().is_empty() {
                existing.api_key = provider.api_key.clone();
            }
            existing.enabled = existing.enabled || provider.enabled;
            dropped.push((provider.id, existing.id.clone()));
        } else {
            kept.push(provider);
        }
    }
    if dropped.is_empty() {
        cfg.cloud_providers = kept;
        return false;
    }
    for (old, kept_id) in &dropped {
        if cfg.active_cloud_provider.as_deref() == Some(old.as_str()) {
            cfg.active_cloud_provider = Some(kept_id.clone());
        }
        for fav in cfg.favorite_models.iter_mut() {
            if let Some(model) = fav.strip_prefix(&format!("{old}:")) {
                *fav = format!("{kept_id}:{model}");
            }
        }
    }
    cfg.cloud_providers = kept;
    true
}

pub struct ConfigStore {
    path: PathBuf,
    /// Serialized snapshot last written to disk. `save` skips the atomic
    /// rename when the snapshot is unchanged, so debounced settings changes
    /// do not rewrite an identical config file on every keystroke.
    last_written: Mutex<Option<String>>,
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigStore {
    pub fn new() -> Self {
        Self::at(config_path())
    }

    /// A store pointed at an explicit path, for isolated tests and migrations.
    pub fn at(path: PathBuf) -> Self {
        Self {
            path,
            last_written: Mutex::new(None),
        }
    }

    pub fn load(&self) -> Result<AppConfig, String> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }
        let data = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        let mut cfg: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        // Hydrate references from the OS keychain. Keys written by older
        // versions are migrated on the next save (below), which keeps load
        // cheap and never loses a key if the keychain is unavailable.
        let mut needs_migration = false;
        for provider in &mut cfg.cloud_providers {
            if crate::services::secrets::is_reference(&provider.api_key) {
                match crate::services::secrets::load(&provider.id) {
                    Ok(Some(key)) => provider.api_key = key,
                    Ok(None) => {
                        log::warn!("No OS keychain entry for provider {}", provider.id);
                        provider.api_key.clear();
                    }
                    Err(e) => {
                        log::warn!("Could not read provider key from OS keychain: {e}");
                        provider.api_key.clear();
                    }
                }
            } else if !provider.api_key.is_empty() {
                needs_migration = true;
            }
        }
        // The same provider added twice (same address and name) shows up
        // twice in the picker. Merge on load, so an existing config heals
        // itself the next time the app starts.
        let before: Vec<String> = cfg.cloud_providers.iter().map(|p| p.id.clone()).collect();
        if dedupe_cloud_providers(&mut cfg) {
            needs_migration = true;
            for id in before {
                if !cfg.cloud_providers.iter().any(|p| p.id == id) {
                    let _ = crate::services::secrets::delete(&id);
                }
            }
        }
        if needs_migration {
            let _ = self.save(&cfg);
        }
        Ok(cfg)
    }

    /// Writes the config atomically. API keys are moved to the OS keychain
    /// and replaced with `keyring:` markers, so the file no longer holds
    /// plain-text secrets. If the keychain is unreachable the key stays in
    /// the file, which is still owner-only on Unix.
    pub fn save(&self, cfg: &AppConfig) -> Result<(), String> {
        let mut on_disk = cfg.clone();
        for provider in &mut on_disk.cloud_providers {
            if crate::services::secrets::is_reference(&provider.api_key) {
                continue;
            }
            if provider.api_key.is_empty() {
                let _ = crate::services::secrets::delete(&provider.id);
                continue;
            }
            match crate::services::secrets::save(&provider.id, &provider.api_key) {
                Ok(()) => provider.api_key = crate::services::secrets::reference(&provider.id),
                Err(e) => {
                    log::warn!("OS keychain unavailable, keeping provider key in config: {e}")
                }
            }
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_string_pretty(&on_disk).map_err(|e| e.to_string())?;
        // Skip a redundant write. The cache is only an optimisation: it never
        // changes what a changed config serializes to, and the first save in
        // a fresh store always writes.
        let mut last = self.last_written.lock().unwrap_or_else(|e| e.into_inner());
        if !snapshot_is_dirty(last.as_deref(), &data) {
            return Ok(());
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, data.as_bytes()).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())?;
        *last = Some(data);
        Ok(())
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
            check: None,
        });
        cfg.active_cloud_provider = Some("p1".into());
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cloud_providers.len(), 1);
        assert_eq!(back.cloud_providers[0].api_key, "sk-test");
        assert_eq!(back.active_cloud_provider.as_deref(), Some("p1"));
    }

    #[test]
    fn duplicate_providers_merge_on_the_same_address_and_name() {
        let provider = |id: &str, model: &str| CloudProvider {
            id: id.into(),
            label: "Kilo Code (free, no key)".into(),
            base_url: "https://api.kilo.ai/api/openrouter".into(),
            model: model.into(),
            api_key: String::new(),
            enabled: true,
            models: vec![crate::services::inference::provider_models::ProviderModel {
                id: model.into(),
                name: None,
                owned_by: None,
                context_length: None,
                free: true,
                reasoning: None,
                vision: None,
                tools: None,
                prompt_price_per_million: None,
                completion_price_per_million: None,
                cache_price_per_million: None,
            }],
            check: None,
        };
        let mut cfg = AppConfig::default();
        cfg.cloud_providers.push(provider("cp-1", "kilo-auto/free"));
        cfg.cloud_providers
            .push(provider("cp-2", "other-model/free"));
        cfg.cloud_providers.push(provider("cp-3", "kilo-auto/free"));
        cfg.active_cloud_provider = Some("cp-2".into());
        cfg.favorite_models.push("cp-2:other-model/free".into());
        assert!(dedupe_cloud_providers(&mut cfg));
        assert_eq!(cfg.cloud_providers.len(), 1);
        assert_eq!(cfg.cloud_providers[0].id, "cp-1");
        assert_eq!(cfg.cloud_providers[0].models.len(), 2);
        // The active provider and the favourite follow the kept entry.
        assert_eq!(cfg.active_cloud_provider.as_deref(), Some("cp-1"));
        assert_eq!(cfg.favorite_models, vec!["cp-1:other-model/free"]);
        assert!(!dedupe_cloud_providers(&mut cfg));
    }

    #[test]
    fn same_address_different_names_stay_separate() {
        let provider = |id: &str, label: &str| CloudProvider {
            id: id.into(),
            label: label.into(),
            base_url: "http://localhost:20128/v1".into(),
            model: String::new(),
            api_key: String::new(),
            enabled: true,
            models: Vec::new(),
            check: None,
        };
        let mut cfg = AppConfig::default();
        cfg.cloud_providers
            .push(provider("cp-1", "9Router (on this computer)"));
        cfg.cloud_providers
            .push(provider("cp-2", "OmniRoute (on this computer)"));
        assert!(!dedupe_cloud_providers(&mut cfg));
        assert_eq!(cfg.cloud_providers.len(), 2);
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
    fn capability_auto_approve_defaults_to_read_only() {
        // A fresh config and an old config file with no such key agree:
        // only reads run without asking.
        let fresh = CapabilityAutoApprove::default();
        assert!(fresh.approves(Capability::ReadFiles));
        assert!(!fresh.approves(Capability::EditFiles));
        assert!(!fresh.approves(Capability::SafeCommands));
        assert!(!fresh.approves(Capability::AllCommands));
        assert!(!fresh.approves(Capability::Browser));
        assert!(!fresh.approves(Capability::ExternalTools));
        let old: AppConfig =
            serde_json::from_str(r#"{"selected_models":[],"cloud_llm_enabled":false}"#).unwrap();
        assert_eq!(
            old.auto_approve_capabilities,
            CapabilityAutoApprove::default()
        );
        assert!(!old.use_compact_prompt, "compact prompt defaults off");
        // "All commands" covers safe commands too.
        let caps = CapabilityAutoApprove {
            all_commands: true,
            ..CapabilityAutoApprove::default()
        };
        assert!(caps.approves(Capability::SafeCommands));
        assert!(caps.approves(Capability::AllCommands));
        let caps = CapabilityAutoApprove {
            safe_commands: true,
            ..CapabilityAutoApprove::default()
        };
        assert!(caps.approves(Capability::SafeCommands));
        assert!(!caps.approves(Capability::AllCommands));
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
    fn dirty_check_skips_identical_snapshots() {
        assert!(snapshot_is_dirty(None, "{}"));
        assert!(!snapshot_is_dirty(Some("{}"), "{}"));
        assert!(snapshot_is_dirty(Some("previous"), "next"));
    }

    #[test]
    fn save_replaces_the_file_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("la-config-save-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = ConfigStore::at(dir.join("config.json"));
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

    fn rule(action: &str, resource: &str, decision: PermissionDecision) -> PermissionRule {
        PermissionRule {
            action: action.to_string(),
            resource: resource.to_string(),
            decision,
        }
    }

    #[test]
    fn wildcards_match_exactly_as_expected() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("write_*", "write_file"));
        assert!(!wildcard_match("write_*", "write"));
        assert!(!wildcard_match("write_*", "read_file"));
        assert!(wildcard_match("notes/*", "notes/a/b.txt"));
        assert!(!wildcard_match("notes/*", "other/a.txt"));
        assert!(wildcard_match("a*c", "abc"));
        assert!(wildcard_match("a*c", "ac"));
        assert!(!wildcard_match("a*c", "ab"));
    }

    #[test]
    fn last_matching_rule_wins() {
        let rules = vec![
            rule("*", "*", PermissionDecision::Deny),
            rule("read_*", "*", PermissionDecision::Allow),
            rule("read_*", "secret/*", PermissionDecision::Deny),
        ];
        let cfg = AppConfig {
            permissions: rules,
            ..AppConfig::default()
        };
        assert_eq!(
            cfg.decide_tool_call("read_file", "notes/a.txt"),
            ToolDecision::Allow
        );
        assert_eq!(
            cfg.decide_tool_call("read_file", "secret/key.txt"),
            ToolDecision::Deny,
            "the later, more specific deny overrides the earlier allow"
        );
        assert_eq!(
            cfg.decide_tool_call("write_file", "notes/a.txt"),
            ToolDecision::Deny
        );
    }

    #[test]
    fn no_rule_means_ask() {
        let cfg = AppConfig::default();
        assert_eq!(
            cfg.decide_tool_call("write_file", "anything"),
            ToolDecision::Ask
        );
    }

    #[test]
    fn credential_pools_rotate_only_on_401_and_429() {
        let mut pool = CredentialPool::new(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(pool.current(), Some("a"));
        assert!(!pool.on_auth_failure(500));
        assert_eq!(pool.current(), Some("a"));
        assert!(pool.on_auth_failure(401));
        assert_eq!(pool.current(), Some("b"));
        assert!(pool.on_auth_failure(429));
        assert_eq!(pool.current(), Some("c"));
        assert!(pool.on_auth_failure(401));
        assert_eq!(pool.current(), Some("a"), "rotation wraps around");

        let mut single = CredentialPool::new(vec!["only".into()]);
        assert!(!single.on_auth_failure(401));
        assert_eq!(single.current(), Some("only"));
        assert!(CredentialPool::new(vec![]).is_empty());
    }

    #[test]
    fn routing_pool_and_mcp_gates_round_trip() {
        let mut cfg = AppConfig::default();
        cfg.routing.provider_priority = vec!["p1".into(), "p2".into()];
        cfg.routing
            .key_pools
            .insert("p1".into(), vec!["k1".into(), "k2".into()]);
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.routing.provider_priority, vec!["p1", "p2"]);
        assert_eq!(back.routing.key_pools["p1"], vec!["k1", "k2"]);
        assert!(back.permissions.is_empty());
        assert!(back.hooks.pre_tool.is_none());
        assert!(back.mcp_servers.is_empty());

        let old = r#"{"selected_models":[],"cloud_llm_enabled":false,"mcp_servers":[{"id":"m1","name":"Docs","enabled":true,"auto_approve":false,"transport":{"type":"stdio","command":"npx"}}]}"#;
        let legacy: AppConfig = serde_json::from_str(old).unwrap();
        assert!(legacy.mcp_servers[0].enabled_tools.is_empty());
        assert!(legacy.mcp_servers[0].disabled_tools.is_empty());
    }

    #[test]
    fn an_unlisted_module_is_on_and_a_listed_one_can_be_off() {
        let dev = DeveloperConfig::default();
        assert!(dev.module_on("context_files"), "missing means on");
        let mut dev = DeveloperConfig::default();
        dev.modules.insert("context_files".into(), false);
        assert!(!dev.module_on("context_files"));
        assert!(dev.module_on("memory"), "other modules are unaffected");
    }
}
