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
    })
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

/// Set the engine context window. Values outside 512..=131072 are clamped;
/// the effective size is still capped per model by the catalog hint.
#[tauri::command]
pub fn settings_set_context(state: State<'_, AppState>, tokens: u32) -> Result<u32, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.context_tokens = tokens.clamp(512, 131_072);
    store.save(&cfg).map_err(AppError::internal)?;
    Ok(cfg.context_tokens)
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
