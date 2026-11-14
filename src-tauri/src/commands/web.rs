//! Web search commands. Every call is an explicit user action over public
//! search/crawl providers; API keys come from the local config file and
//! travel only to their own provider.

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
    pub supports: String,
    pub key_set: bool,
    pub is_default: bool,
}

fn load_web(state: &State<'_, AppState>) -> Result<(AppConfig, WebSearchKeys), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;
    let keys = WebSearchKeys {
        exa_api_key: cfg.web_search.exa_api_key.clone(),
        firecrawl_api_key: cfg.web_search.firecrawl_api_key.clone(),
    };
    Ok((cfg, keys))
}

fn default_provider(cfg: &AppConfig) -> String {
    cfg.web_search
        .provider
        .clone()
        .filter(|p| PROVIDER_TABLE.iter().any(|t| t.id == *p))
        .unwrap_or_else(|| "duckduckgo".into())
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
            supports: p.supports.to_string(),
            key_set: keys.for_provider(p.id).is_ok(),
            is_default: p.id == default,
        })
        .collect())
}

#[tauri::command]
pub fn web_search(
    state: State<'_, AppState>,
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

/// Read one page as text. Default: Firecrawl when its key is configured
/// (it renders markdown), otherwise the keyless Jina Reader.
#[tauri::command]
pub fn web_crawl(
    state: State<'_, AppState>,
    url: String,
    provider: Option<String>,
) -> Result<WebPage, AppError> {
    let (_, keys) = load_web(&state)?;
    let provider = provider.unwrap_or_else(|| {
        if keys
            .firecrawl_api_key
            .as_deref()
            .is_some_and(|k| !k.trim().is_empty())
        {
            "firecrawl".to_string()
        } else {
            "jina".to_string()
        }
    });
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
