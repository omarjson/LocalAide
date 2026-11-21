//! Library status, folder presets, indexing with live progress, and the
//! embedding ("meaning") model choice. Everything here stays on this
//! computer: indexing reads local files and search runs locally.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::app::blocking::with_app_state;
use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::library::FolderEvent;
use crate::services::library::db::FolderRow;

/// The indexing job that is running, so Cancel can find it.
static INDEX_JOB: Mutex<Option<String>> = Mutex::new(None);

/// Skipped files listed in the report; the rest are only counted.
const MAX_LISTED_SKIPS: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct FolderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub path: Option<String>,
    pub exists: bool,
    /// Already in the library.
    pub added: bool,
}

#[tauri::command]
pub fn library_folder_presets(state: State<'_, AppState>) -> Result<Vec<FolderPreset>, AppError> {
    let added: Vec<String> = state
        .library
        .db()
        .list_folders()?
        .into_iter()
        .map(|f| f.path)
        .collect();
    let preset = |id, label, dir: Option<PathBuf>| {
        let path = dir.map(|d| d.to_string_lossy().into_owned());
        FolderPreset {
            id,
            label,
            exists: path.as_deref().is_some_and(|p| Path::new(p).is_dir()),
            added: path.as_ref().is_some_and(|p| added.contains(p)),
            path,
        }
    };
    Ok(vec![
        preset("documents", "Documents", dirs_next::document_dir()),
        preset("desktop", "Desktop", dirs_next::desktop_dir()),
        preset("downloads", "Downloads", dirs_next::download_dir()),
    ])
}

/// Live progress while a folder is added.
#[derive(Debug, Clone, Serialize)]
pub struct IndexProgress {
    pub folder: String,
    pub seen: usize,
    pub indexed: usize,
    pub skipped: usize,
    /// The file being read now, relative to the folder.
    pub current: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexReport {
    pub folder: String,
    pub indexed: usize,
    pub skipped: usize,
    /// The walk stopped at 20,000 files or 32 folder levels.
    pub limited: bool,
    pub cancelled: bool,
    pub ms: u128,
    /// Skipped files grouped by reason, most common first.
    pub reasons: Vec<(String, usize)>,
    /// Up to 200 skipped files with their reason.
    pub skipped_files: Vec<SkippedFile>,
}

/// Add a folder (or scan it again) with progress. Search stays local.
#[tauri::command]
pub async fn library_index_start(
    state: State<'_, AppState>,
    folder: String,
    channel: tauri::ipc::Channel<IndexProgress>,
) -> Result<IndexReport, AppError> {
    let path = PathBuf::from(folder.trim());
    if !path.is_dir() {
        return Err(AppError::invalid_input(format!(
            "There is no folder at {}.",
            path.display()
        )));
    }
    let path = path.canonicalize().unwrap_or(path);
    let (job_id, cancel) = state.jobs.create_job("library_index");
    *INDEX_JOB.lock().unwrap_or_else(|e| e.into_inner()) = Some(job_id.clone());
    let jid = job_id.clone();
    let result = with_app_state(&state, move |state| {
        let shown = path.to_string_lossy().into_owned();
        let started = Instant::now();
        let mut seen = 0usize;
        let mut indexed = 0usize;
        let mut skipped = 0usize;
        let mut reasons: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut listed: Vec<SkippedFile> = Vec::new();
        let mut last_sent = Instant::now() - Duration::from_secs(1);
        let rel = |p: &Path| {
            p.strip_prefix(&path)
                .map(|r| r.to_string_lossy().into_owned())
                .unwrap_or_else(|_| p.to_string_lossy().into_owned())
        };
        let report =
            state
                .library
                .index_folder_with(&path, Some(cancel.clone()), &mut |event| {
                    let current = match event {
                        FolderEvent::Reading(p) => {
                            seen += 1;
                            rel(p)
                        }
                        FolderEvent::Indexed(_) => {
                            indexed += 1;
                            return;
                        }
                        FolderEvent::Skipped(p, reason) => {
                            seen += 1;
                            skipped += 1;
                            *reasons.entry(reason).or_default() += 1;
                            if listed.len() < MAX_LISTED_SKIPS {
                                listed.push(SkippedFile {
                                    path: rel(p),
                                    reason,
                                });
                            }
                            rel(p)
                        }
                    };
                    if last_sent.elapsed() >= Duration::from_millis(120) {
                        last_sent = Instant::now();
                        let _ = channel.send(IndexProgress {
                            folder: shown.clone(),
                            seen,
                            indexed,
                            skipped,
                            current,
                        });
                    }
                })?;
        let cancelled = cancel.load(std::sync::atomic::Ordering::Relaxed);
        let _ = channel.send(IndexProgress {
            folder: shown.clone(),
            seen,
            indexed: report.indexed,
            skipped: report.skipped,
            current: String::new(),
        });
        state.library.db().save_folder_scan(
            &shown,
            report.indexed,
            report.skipped,
            report.limited,
        )?;
        let mut reasons: Vec<(String, usize)> = reasons
            .into_iter()
            .map(|(r, n)| (r.to_string(), n))
            .collect();
        reasons.sort_by_key(|a| std::cmp::Reverse(a.1));
        // New text gets meaning vectors in the background when the
        // embedding model runs, so semantic search covers it.
        if state.embedding_engine.is_running() {
            let lib = state.library.clone();
            std::thread::spawn(move || {
                if let Err(e) = lib.backfill_embeddings(64) {
                    log::warn!("Embedding the new files failed: {e}");
                }
            });
        }
        Ok(IndexReport {
            folder: shown,
            indexed: report.indexed,
            skipped: report.skipped,
            limited: report.limited,
            cancelled,
            ms: started.elapsed().as_millis(),
            reasons,
            skipped_files: listed,
        })
    })
    .await;
    match &result {
        Ok(_) => state.jobs.complete_job(&jid),
        Err(e) => state.jobs.fail_job(&jid, &e.message),
    }
    let mut current = INDEX_JOB.lock().unwrap_or_else(|e| e.into_inner());
    if current.as_deref() == Some(job_id.as_str()) {
        *current = None;
    }
    result
}

#[tauri::command]
pub fn library_index_cancel(state: State<'_, AppState>) -> bool {
    let id = INDEX_JOB.lock().unwrap_or_else(|e| e.into_inner()).clone();
    id.is_some_and(|id| state.jobs.cancel_job(&id))
}

#[tauri::command]
pub fn library_folders(state: State<'_, AppState>) -> Result<Vec<FolderRow>, AppError> {
    state.library.db().list_folders()
}

/// Remove a folder and its files from the library (the files on disk stay).
#[tauri::command]
pub fn library_folder_remove(state: State<'_, AppState>, path: String) -> Result<usize, AppError> {
    let db = state.library.db();
    if !db.remove_folder(&path)? {
        return Err(AppError::invalid_input(
            "That folder is not in the library.",
        ));
    }
    db.delete_documents_under(&path)
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingChoice {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryStatus {
    pub folders: Vec<FolderRow>,
    pub documents: u64,
    pub total_bytes: u64,
    /// Newest scan of any folder, seconds since 1970.
    pub last_scan_at: Option<i64>,
    /// "semantic" when the meaning model runs, else "keyword".
    pub search_mode: &'static str,
    /// Why search is keyword-only, in plain words; None when semantic.
    pub reason: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_ready: bool,
    pub embedded_chunks: usize,
    pub total_chunks: usize,
    /// Embedding models the user can pick, smallest first.
    pub choices: Vec<EmbeddingChoice>,
    pub storage_path: String,
}

fn embedding_choices(state: &AppState) -> Vec<EmbeddingChoice> {
    let mut list: Vec<_> = state
        .model_service
        .all_entries()
        .into_iter()
        .filter(|e| e.kind == "embedding")
        .map(|e| EmbeddingChoice {
            installed: state.model_service.is_installed(&e.id),
            size_bytes: e.file_size,
            name: e.name,
            id: e.id,
        })
        .collect();
    list.sort_by_key(|c| c.size_bytes);
    list
}

pub(crate) fn library_status_of(state: &AppState) -> Result<LibraryStatus, AppError> {
    let folders = state.library.db().list_folders()?;
    let stats = state.library.stats()?;
    let (total_chunks, embedded_chunks) = state.library.embedding_coverage()?;
    let engine = state.embedding_engine.state();
    let selected = state
        .config_store
        .lock()
        .ok()
        .and_then(|s| s.load().ok())
        .and_then(|c| c.embedding_model);
    let choices = embedding_choices(state);
    let reason = if engine.ready {
        None
    } else if !choices.iter().any(|c| c.installed) {
        Some("No meaning model is installed, so search matches exact words only. Download one to also find related wording.".to_string())
    } else if selected.is_none() {
        Some("A meaning model is installed but not chosen, so search matches exact words only. Choose it to search by meaning.".to_string())
    } else {
        Some("The meaning model is not running, so search matches exact words only. Start it again below.".to_string())
    };
    Ok(LibraryStatus {
        last_scan_at: folders.iter().map(|f| f.last_scan_at).max(),
        folders,
        documents: stats.doc_count,
        total_bytes: stats.total_bytes,
        search_mode: if engine.ready { "semantic" } else { "keyword" },
        reason,
        embedding_model: engine.model_id.or(selected),
        embedding_ready: engine.ready,
        embedded_chunks,
        total_chunks,
        choices,
        storage_path: stats.storage_path,
    })
}

#[tauri::command]
pub fn library_status(state: State<'_, AppState>) -> Result<LibraryStatus, AppError> {
    library_status_of(&state)
}

/// Choose the embedding model: remember it, start it, and give the library
/// meaning vectors from it (vectors from another model are cleared first,
/// because they cannot be compared).
#[tauri::command]
pub async fn library_embedding_select(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<LibraryStatus, AppError> {
    crate::services::engine::ensure().await?;
    with_app_state(&state, move |state| {
        let entry = state
            .model_service
            .get_model_entry(&model_id)
            .filter(|e| e.kind == "embedding")
            .ok_or_else(|| AppError::invalid_input("That is not an embedding model."))?;
        if !state.model_service.is_installed(&model_id) {
            return Err(AppError::model_not_installed(&entry.id));
        }
        let previous = {
            let store = state
                .config_store
                .lock()
                .map_err(|e| AppError::internal(e.to_string()))?;
            let mut cfg = store.load().map_err(AppError::internal)?;
            let previous = cfg.embedding_model.clone();
            cfg.embedding_model = Some(model_id.clone());
            store.save(&cfg).map_err(AppError::internal)?;
            previous
        };
        let was_running = state.embedding_engine.state().model_id;
        if previous.as_deref() != Some(model_id.as_str())
            || was_running.as_deref().is_some_and(|m| m != model_id)
        {
            state.library.db().clear_embeddings()?;
        }
        state.embedding_engine.start(Some(&model_id))?;
        let lib = state.library.clone();
        std::thread::spawn(move || {
            if let Err(e) = lib.backfill_embeddings(64) {
                log::warn!("Embedding the library failed: {e}");
            }
        });
        library_status_of(&state)
    })
    .await
}

/// At launch: start the chosen embedding model, then fill in any missing
/// vectors. Runs on its own thread so startup is not slowed.
pub fn start_embedding_at_launch(state: AppState) {
    std::thread::spawn(move || {
        let Some(id) = state
            .config_store
            .lock()
            .ok()
            .and_then(|s| s.load().ok())
            .and_then(|c| c.embedding_model)
        else {
            return;
        };
        if !state.model_service.is_installed(&id) {
            log::info!("The chosen embedding model {id} is not installed; search uses keywords.");
            return;
        }
        if let Err(e) = tauri::async_runtime::block_on(crate::services::engine::ensure()) {
            log::warn!("The model engine is not available: {e}");
            return;
        }
        match state.embedding_engine.start(Some(&id)) {
            Ok(_) => {
                if let Err(e) = state.library.backfill_embeddings(64) {
                    log::warn!("Embedding the library failed: {e}");
                }
            }
            Err(e) => log::warn!("The embedding model did not start: {e}"),
        }
    });
}
