//! Web search commands. Every call is an explicit user action over public
//! search/crawl providers; API keys come from the local config file and
//! travel only to their own provider.

use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::config::AppConfig;
use crate::services::websearch::{PROVIDER_TABLE, WebHit, WebPage, WebSearchKeys};
use serde::Serialize;
use tauri::State;

/// The provider table plus local state: which key slots are filled and
/// which provider the user picked as default.
#[derive(Debug, Clone, Serialize)]
pub struct WebProviderStatus {
    pub id: String,
    pub label: String,
    pub needs_key: bool,
    /// A key can be added to raise the free limits.
    pub takes_key: bool,
    pub supports: String,
    pub note: String,
    /// Firecrawl only: the user's own server address, when set.
    pub base_url: Option<String>,
    pub key_set: bool,
    pub is_default: bool,
}

/// The configured keys and default provider, for chat's web context.
pub(crate) fn web_settings(state: &AppState) -> Result<(String, WebSearchKeys), AppError> {
    let (cfg, keys) = load_web(state)?;
    Ok((default_provider(&cfg), keys))
}

fn load_web(state: &AppState) -> Result<(AppConfig, WebSearchKeys), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;
    let keys = WebSearchKeys {
        exa_api_key: cfg.web_search.exa_api_key.clone(),
        firecrawl_api_key: cfg.web_search.firecrawl_api_key.clone(),
        firecrawl_base_url: cfg.web_search.firecrawl_base_url.clone(),
    };
    Ok((cfg, keys))
}

fn default_provider(cfg: &AppConfig) -> String {
    cfg.web_search
        .provider
        .clone()
        .filter(|p| PROVIDER_TABLE.iter().any(|t| t.id == *p))
        .unwrap_or_else(|| "auto".into())
}

#[tauri::command]
pub fn web_providers(state: State<'_, AppState>) -> Result<Vec<WebProviderStatus>, AppError> {
    let (cfg, keys) = load_web(&state)?;
    let default = default_provider(&cfg);
    Ok(PROVIDER_TABLE
        .iter()
        .map(|p| WebProviderStatus {
            id: p.id.to_string(),
            label: p.label.to_string(),
            needs_key: p.needs_key,
            takes_key: p.takes_key,
            supports: p.supports.to_string(),
            note: p.note.to_string(),
            base_url: (p.id == "firecrawl")
                .then(|| keys.firecrawl_base_url.clone())
                .flatten(),
            key_set: keys.key(p.id).is_some(),
            is_default: p.id == default,
        })
        .collect())
}

#[tauri::command]
pub async fn web_search(
    state: State<'_, AppState>,
    query: String,
    provider: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<WebHit>, AppError> {
    with_app_state(&state, move |state| {
        web_search_work(state, query, provider, limit)
    })
    .await
}

/// Network searches block for seconds, so this runs off the UI thread.
fn web_search_work(
    state: AppState,
    query: String,
    provider: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<WebHit>, AppError> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Err(AppError::invalid_input("Enter something to search for."));
    }
    let (cfg, keys) = load_web(&state)?;
    let provider = provider.unwrap_or_else(|| default_provider(&cfg));
    state
        .web
        .search(&provider, &query, limit.unwrap_or(8), &keys)
}

/// Read one page as text. Default ("auto"): Firecrawl when the user added
/// its key or runs their own Firecrawl, otherwise the Jina Reader with Exa
/// as the fallback.
#[tauri::command]
pub async fn web_crawl(
    state: State<'_, AppState>,
    url: String,
    provider: Option<String>,
) -> Result<WebPage, AppError> {
    with_app_state(&state, move |state| web_crawl_work(state, url, provider)).await
}

/// Fetching a page blocks for seconds, so this runs off the UI thread.
fn web_crawl_work(
    state: AppState,
    url: String,
    provider: Option<String>,
) -> Result<WebPage, AppError> {
    let (_, keys) = load_web(&state)?;
    let provider = provider.unwrap_or_else(|| "auto".to_string());
    state.web.read_page(&url, &provider, &keys)
}

/// Store or clear a provider API key. An empty or whitespace-only key
/// clears the slot; the value is trimmed and bounded.
#[tauri::command]
pub fn web_set_key(
    state: State<'_, AppState>,
    provider: String,
    key: Option<String>,
) -> Result<(), AppError> {
    let value = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    if let Some(k) = &value
        && k.len() > 500
    {
        return Err(AppError::invalid_input("That API key is too long."));
    }
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    let slot = match provider.as_str() {
        "exa" => &mut cfg.web_search.exa_api_key,
        "firecrawl" => &mut cfg.web_search.firecrawl_api_key,
        _ => {
            return Err(AppError::new(
                "WEB_UNKNOWN_PROVIDER",
                format!("'{provider}' does not take an API key."),
            ));
        }
    };
    *slot = value;
    store.save(&cfg).map_err(AppError::internal)
}

/// Pick the provider used when the user searches without choosing one.
#[tauri::command]
pub fn web_set_default(state: State<'_, AppState>, provider: String) -> Result<(), AppError> {
    if !PROVIDER_TABLE.iter().any(|p| p.id == provider) {
        return Err(AppError::new(
            "WEB_UNKNOWN_PROVIDER",
            format!("'{provider}' is not a web search provider this build knows."),
        ));
    }
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.web_search.provider = Some(provider);
    store.save(&cfg).map_err(AppError::internal)
}

/// Point Firecrawl at a server the user runs, or clear it (None or empty)
/// to use Firecrawl's own service. Plain http is allowed for this computer
/// only, like other local servers.
#[tauri::command]
pub fn web_set_firecrawl_url(
    state: State<'_, AppState>,
    url: Option<String>,
) -> Result<(), AppError> {
    let value = url
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty());
    if let Some(u) = &value {
        crate::config::validate_base_url(u).map_err(AppError::invalid_input)?;
    }
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.web_search.firecrawl_base_url = value;
    store.save(&cfg).map_err(AppError::internal)
}
