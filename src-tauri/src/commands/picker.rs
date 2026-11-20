//! The chat's model picker: every model on this computer and every model of
//! the online providers the user switched on, with what each can do.

use std::collections::HashSet;

use tauri::State;

use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::commands::cloud::{apply_engine_mode, load_config, save_config};
use crate::config::{AppConfig, CloudProvider, is_local_host};
use crate::domain::chat::ChatTargetRequest;
use crate::domain::cloud::{ModelChoice, ModelChoices, ModelGroup};
use crate::services::inference::capabilities::{self, Capabilities};
use crate::services::inference::provider_models::ProviderModel;

/// The picker key of a local model.
pub fn local_key(id: &str) -> String {
    format!("local:{id}")
}

/// The picker key of a provider's model.
pub fn remote_key(provider_id: &str, model: &str) -> String {
    format!("{provider_id}:{model}")
}

/// What a provider's model can do: the provider's own word first, then the
/// name.
pub fn remote_caps(m: &ProviderModel) -> Capabilities {
    let id = m.id.to_ascii_lowercase();
    let mut caps = capabilities::guess(&id, false);
    if let Some(r) = m.reasoning {
        caps.reasoning = r;
        caps.efforts = if r {
            capabilities::effort_levels(&id, false)
        } else {
            Vec::new()
        };
    }
    if let Some(v) = m.vision {
        caps.vision = v;
    }
    if let Some(t) = m.tools {
        caps.tools = t;
    }
    if m.context_length.is_some() {
        caps.context = m.context_length;
    }
    caps
}

/// The provider's saved list, plus its default model and the chosen model
/// when the list does not have them.
fn provider_models(p: &CloudProvider, chosen: Option<&str>) -> Vec<ProviderModel> {
    let mut list = p.models.clone();
    for extra in [Some(p.model.as_str()), chosen].into_iter().flatten() {
        let extra = extra.trim();
        if !extra.is_empty() && !list.iter().any(|m| m.id == extra) {
            list.insert(
                0,
                ProviderModel {
                    id: extra.to_string(),
                    name: None,
                    owned_by: None,
                    context_length: None,
                    free: false,
                    reasoning: None,
                    vision: None,
                    tools: None,
                },
            );
        }
    }
    list
}

/// What the model the user chose can do. Used by chat to size the answer
/// and pick the thinking setting.
pub fn caps_for_target(
    state: &AppState,
    cfg: &AppConfig,
    target: &ChatTargetRequest,
) -> Capabilities {
    match target.provider_id.as_deref() {
        None => local_caps(state, cfg, &target.model, true),
        Some(pid) => cfg
            .cloud_providers
            .iter()
            .find(|p| p.id == pid)
            .map(|p| {
                provider_models(p, Some(&target.model))
                    .iter()
                    .find(|m| m.id == target.model)
                    .map(remote_caps)
                    .unwrap_or_else(|| capabilities::guess(&target.model, false))
            })
            .unwrap_or_else(|| capabilities::guess(&target.model, false)),
    }
}

fn local_caps(state: &AppState, cfg: &AppConfig, id: &str, ask_server: bool) -> Capabilities {
    let entry = state.model_service.get_model_entry(id);
    let name = entry.as_ref().map(|e| e.name.clone()).unwrap_or_default();
    let mut caps = capabilities::guess(&format!("{id} {name}"), true);
    if let Some(e) = &entry {
        caps.vision = e.companions.iter().any(|c| c.dest.contains("mmproj"));
    }
    caps.tools = true;
    caps.context = Some(
        state
            .model_service
            .effective_context(id, cfg.context_tokens)
            .max(0) as u64,
    );
    // The loaded model's own template is the best source.
    let loaded = state
        .inference_engine
        .local()
        .endpoint()
        .is_some_and(|ep| ep.model_path == state.model_service.model_file_path(id));
    if ask_server
        && loaded
        && let Some(props) = state.inference_engine.local().props()
    {
        caps = capabilities::with_props(caps, &props, &format!("{id} {name}"));
    }
    caps
}

fn build_choices(state: &AppState, cfg: &AppConfig) -> ModelChoices {
    let favorites: HashSet<&str> = cfg.favorite_models.iter().map(String::as_str).collect();
    let local_ep = state.inference_engine.local().endpoint();
    let loaded_path = local_ep.as_ref().map(|ep| ep.model_path.clone());
    let active_local = state.model_manager.active_model_id();

    let online = cfg.online_allowed();
    let cloud_active = (cfg.cloud_llm_enabled && online)
        .then_some(cfg.active_cloud_provider.as_deref())
        .flatten()
        .and_then(|id| cfg.cloud_providers.iter().find(|p| p.id == id && p.enabled));
    let active_key = match cloud_active {
        Some(p) => Some(remote_key(
            &p.id,
            cfg.active_cloud_model
                .as_deref()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or(&p.model),
        )),
        None => active_local.as_deref().map(local_key),
    };

    let mut models = Vec::new();
    for entry in state.model_service.all_entries() {
        if !matches!(entry.kind.as_str(), "chat" | "code")
            || !state.model_service.is_installed(&entry.id)
        {
            continue;
        }
        let path = state.model_service.model_file_path(&entry.id);
        let is_loaded = loaded_path.as_ref() == Some(&path);
        let caps = local_caps(state, cfg, &entry.id, is_loaded);
        let key = local_key(&entry.id);
        models.push(ModelChoice {
            active: active_key.as_deref() == Some(key.as_str()),
            favorite: favorites.contains(key.as_str()),
            key,
            provider_id: None,
            provider_label: "On this computer".into(),
            model: entry.id.clone(),
            name: entry.name.clone(),
            local: true,
            reasoning: caps.reasoning,
            vision: caps.vision,
            tools: caps.tools,
            context: caps.context,
            efforts: caps.efforts,
            free: true,
            size_bytes: Some(entry.file_size),
        });
    }

    let mut groups = Vec::new();
    for p in &cfg.cloud_providers {
        let local_router = url::Url::parse(&p.base_url).is_ok_and(|u| is_local_host(&u));
        groups.push(ModelGroup {
            provider_id: p.id.clone(),
            label: p.label.clone(),
            enabled: p.enabled,
            local_router,
            needs_refresh: p.models.is_empty(),
        });
        if !p.enabled || !online {
            continue;
        }
        let chosen = (cfg.active_cloud_provider.as_deref() == Some(p.id.as_str()))
            .then_some(cfg.active_cloud_model.as_deref())
            .flatten();
        for m in provider_models(p, chosen) {
            let caps = remote_caps(&m);
            let key = remote_key(&p.id, &m.id);
            models.push(ModelChoice {
                active: active_key.as_deref() == Some(key.as_str()),
                favorite: favorites.contains(key.as_str()),
                key,
                provider_id: Some(p.id.clone()),
                provider_label: p.label.clone(),
                name: m.name.clone().unwrap_or_else(|| m.id.clone()),
                model: m.id,
                local: local_router,
                reasoning: caps.reasoning,
                vision: caps.vision,
                tools: caps.tools,
                context: caps.context,
                efforts: caps.efforts,
                free: m.free || local_router,
                size_bytes: None,
            });
        }
    }

    ModelChoices {
        models,
        groups,
        active_key,
        local_loaded: local_ep.is_some(),
        online_allowed: online,
    }
}

/// Every model the picker can offer.
#[tauri::command]
pub async fn chat_models(state: State<'_, AppState>) -> Result<ModelChoices, AppError> {
    // Asking the local server about its model is a short HTTP call, so this
    // runs off the window's thread.
    with_app_state(&state, |owned| {
        let cfg = load_config(&owned)?;
        Ok(build_choices(&owned, &cfg))
    })
    .await
}

/// Make a model the one chat uses. A local model loads (this can take a
/// minute); an online model is used from the next message on.
#[tauri::command]
pub async fn chat_target_set(
    state: State<'_, AppState>,
    req: ChatTargetRequest,
) -> Result<ModelChoices, AppError> {
    match req.provider_id.as_deref() {
        None => {
            let path = state.model_service.model_file_path(&req.model);
            let already = state
                .inference_engine
                .local()
                .endpoint()
                .is_some_and(|ep| ep.model_path == path);
            if already {
                state.inference_engine.set_local();
                if let Err(e) = state.model_manager.activate(&req.model) {
                    log::warn!("Could not save the active model {}: {e}", req.model);
                }
            } else {
                crate::commands::chat::load_and_activate(&state, req.model.clone()).await?;
            }
            let mut cfg = load_config(&state)?;
            cfg.cloud_llm_enabled = false;
            save_config(&state, &cfg)?;
        }
        Some(pid) => {
            let mut cfg = load_config(&state)?;
            if !cfg.online_allowed() {
                return Err(AppError::invalid_input(
                    "Online providers are switched off. Switch them on in Settings → Online providers.",
                ));
            }
            let Some(provider) = cfg.cloud_providers.iter_mut().find(|p| p.id == pid) else {
                return Err(AppError::invalid_input("That provider no longer exists."));
            };
            if req.model.trim().is_empty() {
                return Err(AppError::invalid_input("Choose a model."));
            }
            // Choosing one of its models in the picker shows the provider.
            provider.enabled = true;
            cfg.cloud_llm_enabled = true;
            cfg.active_cloud_provider = Some(pid.to_string());
            cfg.active_cloud_model = Some(req.model.trim().to_string());
            save_config(&state, &cfg)?;
            apply_engine_mode(&state, &cfg);
        }
    }
    with_app_state(&state, |owned| {
        let cfg = load_config(&owned)?;
        Ok(build_choices(&owned, &cfg))
    })
    .await
}

/// Star or unstar a model in the picker. Returns the starred keys.
#[tauri::command]
pub fn chat_favorite_toggle(
    state: State<'_, AppState>,
    key: String,
) -> Result<Vec<String>, AppError> {
    let key = key.trim().to_string();
    if key.is_empty() || key.len() > 300 {
        return Err(AppError::invalid_input("That model key is not valid."));
    }
    let mut cfg = load_config(&state)?;
    if let Some(pos) = cfg.favorite_models.iter().position(|k| *k == key) {
        cfg.favorite_models.remove(pos);
    } else {
        cfg.favorite_models.push(key);
    }
    save_config(&state, &cfg)?;
    Ok(cfg.favorite_models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_metadata_beats_the_name() {
        let m = ProviderModel {
            id: "plain-model".into(),
            name: None,
            owned_by: None,
            context_length: Some(32768),
            free: false,
            reasoning: Some(true),
            vision: Some(true),
            tools: Some(false),
        };
        let caps = remote_caps(&m);
        assert!(caps.reasoning && caps.vision && !caps.tools);
        assert_eq!(caps.context, Some(32768));
        assert!(!caps.efforts.is_empty());
        assert_eq!(remote_key("p1", "a/b:free"), "p1:a/b:free");
        assert_eq!(local_key("qwen"), "local:qwen");
    }

    #[test]
    fn the_default_and_chosen_models_are_always_listed() {
        let p = CloudProvider {
            id: "p".into(),
            label: "P".into(),
            base_url: "https://x.example/v1".into(),
            model: "default-m".into(),
            api_key: String::new(),
            enabled: true,
            models: vec![],
            check: None,
        };
        let list = provider_models(&p, Some("chosen-m"));
        let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"default-m") && ids.contains(&"chosen-m"));
    }
}
