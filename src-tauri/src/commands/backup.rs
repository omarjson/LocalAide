//! Encrypted local backup commands (ADR-0009). Archives are plain files in
//! a user-chosen folder — copyable to any synced drive by the user. The
//! passphrase is never persisted, so there is nothing to leak.

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::backup::{BackupEngine, BackupEntry, RestoreReport};
use std::path::PathBuf;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupConfigDto {
    pub target_dir: Option<String>,
    pub include_api_keys: bool,
    pub last_backup_at: Option<u64>,
}

fn backup_target(state: &AppState, target_dir: Option<String>) -> PathBuf {
    let stored = state
        .config_store
        .lock()
        .ok()
        .and_then(|s| s.load().ok())
        .and_then(|cfg| cfg.backup.target_dir);
    let dir = target_dir.or(stored).unwrap_or_default();
    if dir.trim().is_empty() {
        crate::app::paths::app_dir().join("backups")
    } else {
        PathBuf::from(dir)
    }
}

#[tauri::command]
pub fn backup_config_get(state: State<'_, AppState>) -> Result<BackupConfigDto, AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let cfg = store.load().map_err(AppError::internal)?;
    Ok(BackupConfigDto {
        target_dir: cfg.backup.target_dir,
        include_api_keys: cfg.backup.include_api_keys,
        last_backup_at: cfg.backup.last_backup_at,
    })
}

#[tauri::command]
pub fn backup_config_set(
    state: State<'_, AppState>,
    target_dir: Option<String>,
    include_api_keys: bool,
) -> Result<(), AppError> {
    let store = state
        .config_store
        .lock()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let mut cfg = store.load().map_err(AppError::internal)?;
    cfg.backup.target_dir = target_dir.filter(|d| !d.trim().is_empty());
    cfg.backup.include_api_keys = include_api_keys;
    store.save(&cfg).map_err(AppError::internal)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupRunResponse {
    pub path: String,
    pub created_at: u64,
    pub memories: usize,
    pub conversations: usize,
    pub api_keys_included: bool,
}

#[tauri::command]
pub async fn backup_run(
    state: State<'_, AppState>,
    target_dir: Option<String>,
    passphrase: String,
    include_api_keys: Option<bool>,
) -> Result<BackupRunResponse, AppError> {
    let engine = BackupEngine::new(crate::app::paths::app_dir());
    let include_keys = include_api_keys.unwrap_or(false);

    // Read the config as a raw value so the archive keeps the full shape.
    let config_value: serde_json::Value = {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        let cfg = store.load().map_err(AppError::internal)?;
        serde_json::to_value(&cfg).map_err(|e| AppError::internal(e.to_string()))?
    };

    let payload = engine.build_payload(config_value, include_keys)?;
    let (memories, conversations) = (payload.memories.len(), payload.conversations.len());
    let target = backup_target(&state, target_dir);
    let path = engine.write_backup(&target, &payload, &passphrase)?;

    // Record the successful backup time for the UI.
    {
        let store = state
            .config_store
            .lock()
            .map_err(|e| AppError::internal(e.to_string()))?;
        let mut cfg = store.load().map_err(AppError::internal)?;
        cfg.backup.last_backup_at = Some(payload.created_at);
        store.save(&cfg).map_err(AppError::internal)?;
    }

    Ok(BackupRunResponse {
        path: path.display().to_string(),
        created_at: payload.created_at,
        memories,
        conversations,
        api_keys_included: include_keys,
    })
}

#[tauri::command]
pub fn backup_list(
    state: State<'_, AppState>,
    target_dir: Option<String>,
) -> Result<Vec<BackupEntry>, AppError> {
    let target = backup_target(&state, target_dir);
    BackupEngine::new(crate::app::paths::app_dir()).list_backups(&target)
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    file_name: String,
    passphrase: String,
    restore_config: Option<bool>,
    target_dir: Option<String>,
) -> Result<RestoreReport, AppError> {
    // The file name must be a bare file name inside the backup folder — no
    // traversal into arbitrary paths.
    if file_name.contains('\\')
        || file_name.contains('/')
        || file_name.contains("..")
        || !file_name.ends_with(".labak")
    {
        return Err(AppError::invalid_input(
            "Restore takes a backup file name from the backup folder",
        ));
    }
    let target = backup_target(&state, target_dir);
    let archive_path = target.join(&file_name);
    let bytes = std::fs::read(&archive_path)
        .map_err(|e| AppError::storage(format!("Could not read the backup file: {e}")))?;

    let engine = BackupEngine::new(crate::app::paths::app_dir());
    let payload = engine.decrypt_payload(&bytes, &passphrase)?;
    engine.restore(
        &payload,
        &passphrase,
        &bytes,
        restore_config.unwrap_or(false),
    )
}
