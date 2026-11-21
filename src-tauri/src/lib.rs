use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::{Emitter, Manager};

pub mod app;
pub mod commands;
pub mod config;
pub mod domain;
pub mod model_manager;
pub mod services;

use app::AppState;
use app::lifecycle;
use model_manager::ModelManager;
use services::inference::InferenceEngine;

pub use app::paths::{
    catalog_cache_path, config_path, custom_models_path, index_dir, memory_dir, models_dir,
};

pub fn run() {
    // A second copy would share the database and start a second set of servers.
    if !lifecycle::claim_single_instance() {
        return;
    }

    let model_manager = Arc::new(ModelManager::new().unwrap_or_else(|e| {
        lifecycle::fatal_startup_error(&format!("The model folder could not be opened: {e}"))
    }));

    let library = Arc::new(
        crate::services::library::LibraryService::open_default(&index_dir()).unwrap_or_else(|e| {
            lifecycle::fatal_startup_error(&format!(
                "The document database could not be opened: {e}"
            ))
        }),
    );

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
            lifecycle::fatal_startup_error(&format!("The model service could not start: {e}"))
        }),
    );

    let jail = Arc::new(
        crate::services::tools::sandbox::WorkspaceJail::new(crate::app::paths::workspace_dir())
            .unwrap_or_else(|e| {
                lifecycle::fatal_startup_error(&format!(
                    "The workspace folder could not be opened: {e}"
                ))
            }),
    );

    let tools = Arc::new(crate::services::tools::ToolService::new(jail, library.db()));
    crate::services::cloud_audit::install(library.db());

    // User-saved prompt skills (markdown files) plus the honest local usage
    // counters behind the "save this as a skill?" suggestions.
    let skills = Arc::new(crate::services::skills::SkillsService::new());

    // The embedding and reranker sidecar engines are built inside AppState;
    // the library gets them injected here so search can use both.
    let app_state = AppState::new(
        model_manager.clone(),
        model_service.clone(),
        library.clone(),
        memory.clone(),
        skills.clone(),
        tools.clone(),
    );
    library.set_embedding_engine(app_state.embedding_engine.clone());
    library.set_rerank_engine(app_state.rerank_engine.clone());

    // Restore the engine mode from config. Cloud mode (opt-in) wins if it was
    // left enabled with a valid provider. Otherwise the saved local model loads
    // in the background once the window is open.
    let mut restored_cloud = false;
    let mut pending_load: Option<(String, PathBuf, i32)> = None;
    if let Ok(store) = app_state.config_store.lock()
        && let Ok(cfg) = store.load()
    {
        if cfg.cloud_llm_enabled && cfg.online_allowed() {
            let active = cfg
                .active_cloud_provider
                .as_deref()
                .and_then(|id| cfg.cloud_providers.iter().find(|p| p.id == id && p.enabled));
            if let Some(p) = active {
                if crate::config::validate_base_url(&p.base_url).is_ok() {
                    app_state.inference_engine.set_cloud_target(
                        crate::commands::cloud::remote_target(p, cfg.active_cloud_model.as_deref()),
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
        if !restored_cloud {
            // Prefer the model the user last loaded. Otherwise use the newest chat
            // model in the selection, since embedding models never chat.
            let startup_model = app_state
                .model_manager
                .active_model_id()
                .into_iter()
                .chain(cfg.selected_models.iter().rev().cloned())
                .find(|id| {
                    app_state.model_service.is_chat_model(id)
                        && app_state.model_service.is_installed(id)
                });
            if let Some(active_id) = startup_model {
                let path = app_state.model_service.model_file_path(&active_id);
                let context = app_state
                    .model_service
                    .effective_context(&active_id, cfg.context_tokens);
                pending_load = Some((active_id, path, context));
            }
        }
    }

    // Handles for the background load and the exit handler. AppState holds only
    // shared handles, so cloning it is cheap.
    crate::commands::library_status::start_embedding_at_launch(app_state.clone());
    let startup_state = app_state.clone();
    let shutdown_state = app_state.clone();
    let engine_loading = app_state.engine_loading.clone();
    let drop_state = app_state.clone();

    // Logs go to a rotated file in the app folder in every build, so a user can
    // send that file when something goes wrong.
    let log_dir = app::paths::app_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_plugin = tauri_plugin_log::Builder::new()
        .clear_targets()
        .level(log::LevelFilter::Info)
        .max_file_size(5_000_000)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
        .targets([
            tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
                path: log_dir,
                file_name: Some("localaide".to_string()),
            }),
            tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
        ])
        .build();

    tauri::Builder::default()
        .plugin(log_plugin)
        .plugin(tauri_plugin_dialog::init())
        // Registered always; it installs nothing unless the user confirms in
        // Settings, and builds without an update key never offer to install.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        // Files dropped on the window may be read by chat; nothing else may.
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                commands::attach::remember_drop(&drop_state, paths);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::system_probe,
            commands::model_list,
            commands::hf_search,
            commands::hf_repo_files,
            commands::hf_custom_add,
            commands::model_activate,
            commands::model_remove,
            commands::model_verify,
            commands::model_presets,
            commands::model_download,
            commands::model_download_pause,
            commands::model_download_cancel,
            commands::model_download_preset,
            commands::model_preset_cancel,
            commands::model_catalog_status,
            commands::model_catalog_refresh,
            commands::model_catalog_reset,
            commands::inference_load,
            commands::inference_unload,
            commands::inference_status,
            commands::chat_stream,
            commands::chat_cancel,
            commands::chat_thread_list,
            commands::chat_thread_save,
            commands::chat_thread_load,
            commands::chat_thread_delete,
            commands::chat_thread_export_save,
            commands::chat_export_text_save,
            commands::pick_folder,
            commands::pick_document,
            commands::open_link,
            commands::summarize,
            commands::index_file,
            commands::index_folder,
            commands::search_index,
            commands::list_index,
            commands::library_compact,
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
            commands::settings_context,
            commands::settings_set_context,
            commands::settings_context_info,
            commands::selftest_run,
            commands::library_folder_presets,
            commands::library_index_start,
            commands::library_index_cancel,
            commands::library_folders,
            commands::library_folder_remove,
            commands::library_status,
            commands::library_embedding_select,
            commands::voice_settings_get,
            commands::voice_settings_set,
            commands::voice_key_set,
            commands::voice_models,
            commands::voice_engine_fetch,
            commands::voice_transcribe,
            commands::voice_live_start,
            commands::voice_live_push,
            commands::voice_live_stop,
            commands::voice_cleanup,
            commands::share_status,
            commands::share_start,
            commands::share_stop,
            commands::share_cloudflared_fetch,
            commands::settings_theme,
            commands::settings_set_theme,
            commands::settings_model_memory,
            commands::settings_set_model_memory,
            commands::chat_prefs_get,
            commands::chat_prefs_set,
            commands::skills_list,
            commands::skills_read,
            commands::skills_create,
            commands::skills_update,
            commands::skills_delete,
            commands::skills_suggestions,
            commands::skills_sources,
            commands::skills_discover,
            commands::skills_remote_detail,
            commands::skills_import,
            commands::system_health,
            commands::update_check,
            commands::update_install,
            commands::logs_tail,
            commands::engine_info,
            commands::chat_enhance_prompt,
            commands::chat_suggest_next,
            commands::chat_starters,
            commands::skills_undo,
            commands::files_open,
            commands::files_reveal,
            commands::files_open_with_apps,
            commands::files_open_with,
            commands::ui_scale_get,
            commands::ui_scale_set,
            commands::engine_fetch,
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
            commands::cloud_provider_presets,
            commands::cloud_list_models,
            commands::web_search,
            commands::web_crawl,
            commands::web_providers,
            commands::web_set_key,
            commands::web_set_default,
            commands::web_set_firecrawl_url,
            commands::developer_get,
            commands::developer_set,
            commands::developer_reset,
            commands::opencode_get,
            commands::opencode_set,
            commands::opencode_check,
            commands::opencode_send,
            commands::skills_remote_describe,
            commands::chat_tool_decide,
            commands::chat_models,
            commands::chat_target_set,
            commands::chat_favorite_toggle,
            commands::chat_attach_pick,
            commands::chat_attach_dropped,
            commands::cloud_provider_set_enabled,
            commands::cloud_online_set,
            commands::cloud_audit_list,
            commands::cloud_provider_test,
            commands::cloud_audit_clear,
            commands::cloud_models_refresh,
            commands::mcp_servers_list,
            commands::mcp_presets,
            commands::mcp_server_save,
            commands::mcp_server_toggle,
            commands::mcp_server_delete,
            commands::mcp_server_test,
            commands::mcp_import,
        ])
        .setup(move |setup_app| {
            crate::services::engine::init(setup_app.handle().clone());
            // The interface scale from Settings → Appearance.
            let scale = commands::assist::saved_scale();
            if (scale - 1.0).abs() > f64::EPSILON
                && let Some(window) = setup_app.get_webview_window("main")
            {
                let _ = window.set_zoom(scale);
            }
            if let Some((model_id, path, context)) = pending_load {
                let handle = setup_app.handle().clone();
                engine_loading.store(true, Ordering::SeqCst);
                let _ = handle.emit(
                    "engine-status",
                    app::events::EngineStatusEvent {
                        state: "loading".to_string(),
                        model_id: Some(model_id.clone()),
                        message: None,
                    },
                );
                std::thread::spawn(move || {
                    let result = tauri::async_runtime::block_on(crate::services::engine::ensure())
                        .and_then(|_| startup_state.inference_engine.load(&path, context));
                    engine_loading.store(false, Ordering::SeqCst);
                    let payload = match result {
                        Ok(_) => {
                            log::info!("Auto-loaded model: {model_id}");
                            app::events::EngineStatusEvent {
                                state: "ready".to_string(),
                                model_id: Some(model_id),
                                message: None,
                            }
                        }
                        Err(e) => {
                            log::warn!("Could not auto-load model {model_id}: {e}");
                            app::events::EngineStatusEvent {
                                state: "failed".to_string(),
                                model_id: Some(model_id),
                                message: Some(e.message),
                            }
                        }
                    };
                    let _ = handle.emit("engine-status", payload);
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            lifecycle::fatal_startup_error(&format!("The app window could not open: {e}"))
        })
        .run(move |_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                // Stop the engines. The job object ends any child that is left.
                let _ = shutdown_state.inference_engine.unload();
                shutdown_state.embedding_engine.stop();
                shutdown_state.rerank_engine.stop();
                shutdown_state.mcp.disconnect_all();
                crate::services::share::stop(&shutdown_state);
                // Fold the database log into the file, so the next start is quick.
                if let Err(e) = shutdown_state.library.checkpoint() {
                    log::warn!("Database checkpoint at exit failed: {e}");
                }
            }
        });
}
