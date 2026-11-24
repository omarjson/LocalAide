use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::domain::settings::AppConfigDto;
use tauri::State;

/// The part of the config the app shell needs at startup. A corrupt config
/// file is an error here, so the app can say so instead of showing setup.
#[tauri::command]
pub fn config_load(state: State<'_, AppState>) -> Result<AppConfigDto, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;
    Ok(AppConfigDto {
        selected_models: cfg.selected_models,
        setup_complete: cfg.setup_complete,
        has_online_providers: !cfg.cloud_providers.is_empty(),
    })
}

/// The setup wizard finished, or the user chose to set up later. Either way
/// setup must not appear again on the next launch.
#[tauri::command]
pub fn config_setup_complete(state: State<'_, AppState>) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.setup_complete = true;
    store.save(&cfg).map_err(AppError::internal)
}

/// Current engine context window setting (tokens).
#[tauri::command]
pub fn settings_context(state: State<'_, AppState>) -> Result<u32, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(store.load().map_err(AppError::internal)?.context_tokens)
}

/// Smallest and largest conversation memory the setting accepts, in tokens.
pub const MIN_CONTEXT_TOKENS: u32 = 512;
pub const MAX_CONTEXT_TOKENS: u32 = 1_048_576;
/// Slider maximum when no installed model says how much it can hold.
const FALLBACK_CONTEXT_MAX: u32 = 131_072;

/// Set the engine context window. Values outside 512..=1,048,576 are
/// clamped; the effective size is still capped per model by the catalog hint.
#[tauri::command]
pub fn settings_set_context(state: State<'_, AppState>, tokens: u32) -> Result<u32, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.context_tokens = tokens.clamp(MIN_CONTEXT_TOKENS, MAX_CONTEXT_TOKENS);
    store.save(&cfg).map_err(AppError::internal)?;
    Ok(cfg.context_tokens)
}

/// What Settings → Conversation memory needs: the saved value, how far the
/// slider may go (the model's own context window), and what is loaded now.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextInfo {
    /// The saved setting, in tokens.
    pub value: u32,
    pub min: u32,
    /// Slider maximum: the active model's native window, else the largest
    /// native window among installed chat models, else 131,072.
    pub max: u32,
    /// The model the maximum comes from.
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    /// The size the local engine is running with right now, if a model is loaded.
    pub loaded: Option<u32>,
    /// The model loaded in the local engine, which "Reload model" restarts.
    pub active_id: Option<String>,
}

#[tauri::command]
pub fn settings_context_info(state: State<'_, AppState>) -> Result<ContextInfo, AppError> {
    let value = {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        store.load().map_err(AppError::internal)?.context_tokens
    };
    let models = &state.model_service;
    let active_id = state.model_manager.active_model_id();
    let active = active_id
        .clone()
        .and_then(|id| models.get_model_entry(&id))
        .filter(|e| e.context_tokens.is_some());
    let source = active.or_else(|| {
        models
            .all_entries()
            .into_iter()
            .filter(|e| models.is_chat_model(&e.id) && models.is_installed(&e.id))
            .filter(|e| e.context_tokens.is_some())
            .max_by_key(|e| e.context_tokens)
    });
    let max = source
        .as_ref()
        .and_then(|e| e.context_tokens)
        .unwrap_or(FALLBACK_CONTEXT_MAX);
    let loaded = state
        .inference_engine
        .local()
        .endpoint()
        .map(|ep| ep.context_size.max(0) as u32);
    Ok(ContextInfo {
        value,
        min: MIN_CONTEXT_TOKENS,
        max: context_slider_max(max),
        model_id: source.as_ref().map(|e| e.id.clone()),
        model_name: source.map(|e| e.name),
        active_id: active_id.filter(|_| loaded.is_some()),
        loaded,
    })
}

/// Keep the slider maximum inside the accepted range.
fn context_slider_max(native: u32) -> u32 {
    native.clamp(MIN_CONTEXT_TOKENS * 4, MAX_CONTEXT_TOKENS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_max_follows_the_model_inside_the_range() {
        assert_eq!(context_slider_max(32_768), 32_768);
        assert_eq!(context_slider_max(262_144), 262_144);
        assert_eq!(context_slider_max(10_000_000), MAX_CONTEXT_TOKENS);
        assert_eq!(context_slider_max(100), MIN_CONTEXT_TOKENS * 4);
    }
}

/// Share of memory models may use, in percent (Settings → Memory for models).
#[tauri::command]
pub fn settings_model_memory(state: State<'_, AppState>) -> Result<u8, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(store
        .load()
        .map_err(AppError::internal)?
        .memory_percent_for_models())
}

/// Saves the memory share. Values outside 20..=90 are clamped. The next system
/// probe uses it, so the Models screen and setup flag models that do not fit.
#[tauri::command]
pub fn settings_set_model_memory(state: State<'_, AppState>, percent: u8) -> Result<u8, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.model_memory_percent = Some(percent);
    let applied = cfg.memory_percent_for_models();
    cfg.model_memory_percent = Some(applied);
    store.save(&cfg).map_err(AppError::internal)?;
    Ok(applied)
}

/// The UI theme the user last chose. None = the dark default.
#[tauri::command]
pub fn settings_theme(state: State<'_, AppState>) -> Result<Option<String>, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(store.load().map_err(AppError::internal)?.theme)
}

/// Persist the theme choice. Unknown values fall back to the dark default.
#[tauri::command]
pub fn settings_set_theme(
    state: State<'_, AppState>,
    theme: Option<String>,
) -> Result<(), AppError> {
    let value = theme.filter(|t| t == "dark" || t == "light");
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.theme = value;
    store.save(&cfg).map_err(AppError::internal)
}

/// Chat preferences the user last chose (system prompt, temperature,
/// sampling cap, persona), persisted across sessions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatPrefs {
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub persona: Option<String>,
}

#[tauri::command]
pub fn chat_prefs_get(state: State<'_, AppState>) -> Result<ChatPrefs, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;
    Ok(ChatPrefs {
        system_prompt: cfg.chat_system_prompt,
        temperature: cfg.chat_temperature,
        max_tokens: cfg.chat_max_tokens,
        persona: cfg.chat_persona,
    })
}

#[tauri::command]
pub fn chat_prefs_set(state: State<'_, AppState>, prefs: ChatPrefs) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.chat_system_prompt = prefs.system_prompt.filter(|s| !s.trim().is_empty());
    cfg.chat_temperature = prefs.temperature.map(|t| t.clamp(0.0, 2.0));
    cfg.chat_max_tokens = prefs.max_tokens.map(|t| t.clamp(64, 8192));
    cfg.chat_persona = prefs.persona.filter(|s| !s.trim().is_empty());
    store.save(&cfg).map_err(AppError::internal)?;

    // Honest local adaptation: count custom prompt reuse so the Skills
    // screen can suggest saving it. No profile, no network, no inference.
    if let Some(prompt) = &cfg.chat_system_prompt {
        state.skills.record_prompt_usage(prompt);
    }
    Ok(())
}
