use std::sync::Arc;

pub mod app;
pub mod commands;
pub mod config;
pub mod domain;
pub mod model_manager;
pub mod services;

use app::AppState;
use model_manager::ModelManager;
use services::inference::InferenceEngine;

pub use app::paths::{
    catalog_cache_path, config_path, custom_models_path, index_dir, memory_dir, models_dir,
};

pub fn run() {
    let model_manager = Arc::new(ModelManager::new().unwrap_or_else(|e| {
        log::error!("Failed to init model manager: {e}; falling back to temp directory");
        let fallback = std::env::temp_dir().join("localaide_models");
        ModelManager::with_dir(fallback)
            .unwrap_or_else(|_| panic!("Failed even temp model manager"))
    }));

    let library = Arc::new(crate::services::library::LibraryService::open_default(&index_dir()).unwrap_or_else(|e| {
        log::error!("Failed to init SQLite library database at standard path: {e}; falling back to temp dir");
        let fallback = std::env::temp_dir().join("localaide_library_fallback");
        crate::services::library::LibraryService::open_default(&fallback).unwrap_or_else(|_| {
            crate::services::library::LibraryService::open_in_memory().expect("In-memory SQLite must always succeed")
        })
    }));

    let memory = Arc::new(crate::services::memory::MemoryService::new(
        library.db(),
        library.clone(),
        memory_dir(),
    ));

    let model_service = Arc::new(
        crate::services::models::ModelService::new(
            models_dir(),
            catalog_cache_path(),
            custom_models_path(),
        )
        .unwrap_or_else(|e| {
            log::error!(
                "Failed to init ModelService at standard path: {e}; falling back to temp dir"
            );
            let fallback = std::env::temp_dir().join("localaide_models_fallback");
            crate::services::models::ModelService::new(
                fallback,
                catalog_cache_path(),
                custom_models_path(),
            )
            .expect("Fallback model service must initialize")
        }),
    );

    let jail = Arc::new(
        crate::services::tools::sandbox::WorkspaceJail::new(crate::app::paths::workspace_dir())
            .unwrap_or_else(|e| {
                log::error!("Failed to init WorkspaceJail: {e}; falling back to temp dir");
                let fallback = std::env::temp_dir().join("localaide_workspace_fallback");
                crate::services::tools::sandbox::WorkspaceJail::new(fallback)
                    .expect("Fallback workspace jail must initialize")
            }),
    );

    let tools = Arc::new(crate::services::tools::ToolService::new(jail, library.db()));

    // User-saved prompt skills (markdown files) plus the honest local usage
    // counters behind the "save this as a skill?" suggestions.
    let skills = Arc::new(crate::services::skills::SkillsService::new());

    // Local embedding engine for semantic and hybrid search. Starts only on
    // explicit user action (RAM budget respect); the library uses it for
    // ingest vectorization and query embedding when it is running.
    let embedding_engine = Arc::new(crate::services::library::embedding::EmbeddingEngine::new(
        model_service.clone(),
    ));
    library.set_embedding_engine(embedding_engine.clone());
    library.set_rerank_engine(rerank_engine.clone());

    // Local reranker engine for the search second stage; like the embedding
    // engine it starts only on explicit user action (RAM budget respect).
    let rerank_engine = Arc::new(crate::services::library::rerank::RerankEngine::new(
        model_service.clone(),
    ));

    let app_state = AppState::new(
        model_manager.clone(),
        model_service.clone(),
        embedding_engine.clone(),
        rerank_engine.clone(),
        library.clone(),
        memory.clone(),
        skills.clone(),
        tools.clone(),
    );

    // Restore the engine mode and previously active model from config. Cloud
    // mode (opt-in) wins if it was left enabled with a valid provider; the
    // local sidecar auto-loads its model in every other case, including when
    // the stored provider was deleted or has an invalid base URL.
    let mut restored_cloud = false;
    if let Ok(store) = app_state.config_store.lock()
        && let Ok(cfg) = store.load()
    {
        if cfg.cloud_llm_enabled {
            let active = cfg
                .active_cloud_provider
                .as_deref()
                .and_then(|id| cfg.cloud_providers.iter().find(|p| p.id == id));
            if let Some(p) = active {
                if crate::config::validate_base_url(&p.base_url).is_ok() {
                    app_state.inference_engine.set_cloud_target(
                        crate::services::inference::remote::RemoteTarget {
                            label: p.label.clone(),
                            base_url: p.base_url.clone(),
                            model: p.model.clone(),
                            api_key: p.api_key.clone(),
                        },
                    );
                    restored_cloud = true;
                    log::info!("Cloud mode restored: {label}", label = p.label);
                } else {
                    log::warn!(
                        "Cloud provider {label} has an invalid base URL; staying local",
                        label = p.label
                    );
                }
            } else {
                log::warn!("Cloud mode was on but no active provider exists; staying local");
            }
        }
        if !restored_cloud
            && let Some(active_id) = cfg.selected_models.last()
            && app_state.model_service.get_model_entry(active_id).is_some()
        {
            let path = app_state.model_service.model_file_path(active_id);
            if path.exists() {
                let context = app_state
                    .model_service
                    .effective_context(active_id, cfg.context_tokens);
                match app_state.inference_engine.load(&path, context) {
                    Ok(_) => log::info!("Auto-loaded model: {active_id}"),
                    Err(e) => log::warn!("Could not auto-load model {active_id}: {e}"),
                }
            }
        }
    }

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::system_probe,
            commands::model_list,
            commands::hf_search,
            commands::hf_repo_files,
            commands::hf_custom_add,
            commands::model_activate,
            commands::model_remove,
            commands::model_verify,
            commands::model_existing_size,
            commands::model_presets,
            commands::model_download,
            commands::model_download_cancel,
            commands::model_download_preset,
            commands::model_preset_cancel,
            commands::model_catalog_status,
            commands::model_catalog_refresh,
            commands::model_catalog_reset,
            commands::inference_load,
            commands::inference_unload,
            commands::chat_stream,
            commands::chat_cancel,
            commands::chat_thread_list,
            commands::chat_thread_save,
            commands::chat_thread_load,
            commands::chat_thread_delete,
            commands::chat_thread_export,
            commands::summarize,
            commands::index_file,
            commands::index_folder,
            commands::search_index,
            commands::list_index,
            commands::indexer_stats,
            commands::index_remove,
            commands::index_watch_start,
            commands::index_watch_stop,
            commands::memory_create,
            commands::memory_update,
            commands::memory_list,
            commands::memory_read,
            commands::memory_delete,
            commands::memory_search,
            commands::memory_forget_all,
            commands::tool_execute,
            commands::tool_audits_list,
            commands::tool_audits_clear,
            commands::config_load,
            commands::config_save,
            commands::config_compute_ram,
            commands::settings_context,
            commands::settings_set_context,
            commands::chat_prefs_get,
            commands::chat_prefs_set,
            commands::skills_list,
            commands::skills_read,
            commands::skills_create,
            commands::skills_update,
            commands::skills_delete,
            commands::skills_suggestions,
            commands::system_health,
            commands::update_check,
            commands::logs_tail,
            commands::backup_config_get,
            commands::backup_config_set,
            commands::backup_run,
            commands::backup_list,
            commands::backup_restore,
            commands::embedding_status,
            commands::embedding_start,
            commands::embedding_stop,
            commands::embedding_backfill,
            commands::rerank_status,
            commands::rerank_start,
            commands::rerank_stop,
            commands::cloud_config_get,
            commands::cloud_provider_save,
            commands::cloud_provider_delete,
            commands::cloud_mode_set,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
