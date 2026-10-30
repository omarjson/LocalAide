use std::path::PathBuf;

use serde::Serialize;
use sha2::{Digest, Sha256};
use sysinfo::{Disks, System};
use tauri::State;

mod inference;
mod indexer;
mod model_manager;
mod voice;
mod tools;
mod config;
mod embeddings;
mod watcher;
use inference::LlamaState;
use indexer::{Indexer, SearchMode};
use model_manager::{ModelEntry, ModelManager};
use std::sync::Arc;

const OS_RESERVE_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB OS reserve
const BUDGET_SHARE: f64 = 0.7; // 70% of usable RAM

/// ~/.aiassistant/models/
pub fn models_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".aiassistant/models")
}

/// ~/.aiassistant/index/
pub fn index_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".aiassistant/index")
}

/// ~/.aiassistant/memory/
pub fn memory_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".aiassistant/memory")
}

/// ~/.aiassistant/config.json
pub fn config_path() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".aiassistant/config.json")
}

#[derive(Clone, Serialize)]
struct SystemProbe {
    total_ram: u64,
    usable_ram: u64,
    budget_ram: u64,
    cpu_usage: f32,
    disks: Vec<DiskInfo>,
}

#[derive(Clone, Serialize)]
struct DiskInfo {
    name: String,
    total: u64,
    available: u64,
}

/// System probe used by the Simple mode wizard and the RAM budget chip.
#[tauri::command]
fn system_probe() -> SystemProbe {
    let mut sys = System::new_all();
    sys.refresh_all();

    let total_ram = sys.total_memory();
    let usable_ram = total_ram.saturating_sub(OS_RESERVE_BYTES);
    let budget_ram = ((usable_ram as f64) * BUDGET_SHARE) as u64;
    let cpu_usage = sys.global_cpu_usage();

    let disks = Disks::new_with_refreshed_list();
    let disk = disks
        .list()
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            total: d.total_space(),
            available: d.available_space(),
        })
        .collect();

    SystemProbe {
        total_ram,
        usable_ram,
        budget_ram,
        cpu_usage,
        disks: disk,
    }
}

/// List catalog models with installed/active flags merged from persisted state.
#[tauri::command]
fn model_list(state: State<'_, ModelManager>) -> Vec<ModelEntry> {
    state.inner().list_models()
}

/// Activate a model (deactivates any previously active model).
#[tauri::command]
fn model_activate(state: State<'_, ModelManager>, id: String) -> Result<(), String> {
    state.inner().activate(&id)
}

/// Remove an installed model file and its persisted state entry.
#[tauri::command]
fn model_remove(state: State<'_, ModelManager>, id: String) -> Result<(), String> {
    state.inner().remove(&id)
}

/// Verify an installed model's SHA256 against the catalog.
#[tauri::command]
fn model_verify(state: State<'_, ModelManager>, id: String) -> Result<bool, String> {
    let mgr = state.inner();
    let entry = mgr
        .list_models()
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| format!("model {id} not in catalog"))?;
    let Some(expected) = entry.sha256 else {
        return Err("no checksum recorded for this model".into());
    };
    let path = mgr.model_path(&id);
    if !path.exists() {
        return Err("model file not found on disk".into());
    }
    let actual = sha256_file(&path).map_err(|e| e.to_string())?;
    Ok(actual == expected)
}

/// Return bytes already present on disk for a model (resume support).
#[tauri::command]
fn model_existing_size(state: State<'_, ModelManager>, id: String) -> Result<u64, String> {
    Ok(state.inner().existing_size(&id))
}

/// Download a model with progress, resume and SHA256 verification.
/// Emits `model_download_progress` and `model_download_done` events.
#[tauri::command]
async fn model_download(
    app: tauri::AppHandle,
    state: State<'_, ModelManager>,
    id: String,
) -> Result<(), String> {
    state.inner().download(app, id).await
}

/// Cancel an in-flight download for a model.
#[tauri::command]
fn model_download_cancel(state: State<'_, ModelManager>, id: String) -> Result<(), String> {
    state.inner().cancel_download(&id);
    Ok(())
}

/// Download presets exposed to the Simple mode wizard.
#[tauri::command]
fn model_presets() -> Vec<model_manager::Preset> {
    vec![
        model_manager::Preset::tiny(),
        model_manager::Preset::small(),
        model_manager::Preset::balanced(),
        model_manager::Preset::advanced(),
    ]
}

/// Download everything in a preset, one model at a time.
#[tauri::command]
async fn model_download_preset(
    app: tauri::AppHandle,
    state: State<'_, ModelManager>,
    id: String,
) -> Result<(), String> {
    let preset = match id.as_str() {
        "tiny" => model_manager::Preset::tiny(),
        "small" => model_manager::Preset::small(),
        "balanced" => model_manager::Preset::balanced(),
        "advanced" => model_manager::Preset::advanced(),
        other => return Err(format!("unknown preset: {other}")),
    };
    for model_id in &preset.models {
        let _ = state.inner().download(app.clone(), model_id.clone()).await;
    }
    Ok(())
}

/// Index a single file and return metadata.
#[tauri::command]
fn index_file(state: State<'_, Arc<Indexer>>, path: String) -> Result<serde_json::Value, String> {
    let doc = state.inner().index_file(&path)?;
    Ok(serde_json::to_value(doc).map_err(|e| e.to_string())?)
}

/// Index a folder recursively.
#[tauri::command]
fn index_folder(state: State<'_, Arc<Indexer>>, folder: String) -> Result<serde_json::Value, String> {
    let (indexed, skipped) = state.inner().index_folder(&folder)?;
    Ok(serde_json::json!({ "indexed": indexed, "skipped": skipped }))
}

/// Search indexed documents.
#[tauri::command]
fn search_index(state: State<'_, Arc<Indexer>>, query: String, mode: Option<String>, limit: Option<usize>) -> Result<Vec<serde_json::Value>, String> {
    let mode = match mode.as_deref() {
        Some("semantic") => SearchMode::Semantic,
        Some("keyword") => SearchMode::Keyword,
        _ => SearchMode::Hybrid,
    };
    let limit = limit.unwrap_or(20);
    let results = state.inner().search(&query, mode, limit)?;
    Ok(results.iter().map(|r| serde_json::to_value(r).unwrap()).collect())
}

/// List indexed documents.
#[tauri::command]
fn list_index(state: State<'_, Arc<Indexer>>) -> Result<Vec<serde_json::Value>, String> {
    Ok(state.inner().list().iter().map(|d| serde_json::to_value(d).unwrap()).collect())
}

/// Indexer statistics.
#[tauri::command]
fn indexer_stats(state: State<'_, Arc<Indexer>>) -> Result<serde_json::Value, String> {
    Ok(serde_json::to_value(state.inner().stats()).map_err(|e| e.to_string())?)
}

/// Remove indexed document.
#[tauri::command]
fn index_remove(state: State<'_, Arc<Indexer>>, path: String) -> Result<(), String> {
    state.inner().remove(&path)
}

/// Create a markdown memory note.
#[tauri::command]
fn memory_create(state: State<'_, Arc<Indexer>>, title: String, content: String) -> Result<serde_json::Value, String> {
    use crate::memory_dir;
    let dir = memory_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe_title = title.chars().take(100).map(|c| if c.is_alphanumeric() || c=='-' || c=='_' {c} else {'_'}).collect::<String>();
    let path = dir.join(format!("{safe_title}.md"));
    let md = format!("# {title}\n\n{content}\n");
    std::fs::write(&path, md).map_err(|e| e.to_string())?;
    // Index the new note for semantic search
    let idx_path = path.to_string_lossy().to_string();
    let _ = state.inner().index_file(&idx_path);
    Ok(serde_json::json!({ "path": path.to_string_lossy() }))
}

/// List memory notes.
#[tauri::command]
fn memory_list() -> Result<Vec<serde_json::Value>, String> {
    use crate::memory_dir;
    let dir = memory_dir();
    if !dir.exists() { return Ok(vec![]); }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            out.push(serde_json::json!({
                "path": path.to_string_lossy(),
                "size": meta.len(),
                "modified": modified
            }));
        }
    }
    Ok(out)
}

/// Read a memory note by path.
#[tauri::command]
fn memory_read(path: String) -> Result<String, String> {
    use std::fs;
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("file not found".to_string());
    }
    fs::read_to_string(p).map_err(|e| e.to_string())
}

/// Search memory notes via indexer with recency bias.
#[tauri::command]
fn memory_search(state: State<'_, Arc<Indexer>>, query: String) -> Result<Vec<serde_json::Value>, String> {
    let mut results = state.inner().search(&query, SearchMode::Semantic, 50)?;
    // Recency bias: boost score by age (newer docs get higher boost)
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as f64;
    for r in &mut results {
        let age_days = (now - r.doc.modified as f64) / 86400.0;
        let recency_boost = if age_days.is_finite() {
            // Exponential decay, half-life ~180 days
            (-age_days / 180.0).exp()
        } else {
            0.0
        };
        r.score = r.score * 0.7 + recency_boost as f32 * 0.3;
    }
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(20);
    Ok(results.iter().map(|r| serde_json::to_value(r).unwrap()).collect())
}

/// STT transcribe audio file.
#[tauri::command]
fn stt_transcribe(state: State<'_, tauri::State<'_, voice::VoiceManager>>, path: String) -> Result<serde_json::Value, String> {
    let res = state.inner().transcribe(&path)?;
    Ok(serde_json::to_value(res).map_err(|e| e.to_string())?)
}

/// TTS synthesize text to audio.
#[tauri::command]
fn tts_synthesize(state: State<'_, tauri::State<'_, voice::VoiceManager>>, text: String, voice_id: String) -> Result<serde_json::Value, String> {
    let res = state.inner().synthesize(&text, &voice_id)?;
    Ok(serde_json::to_value(res).map_err(|e| e.to_string())?)
}

/// Execute a safe tool.
#[tauri::command]
fn tool_execute(call: serde_json::Value) -> Result<serde_json::Value, String> {
    let tool_call: tools::ToolCall = serde_json::from_value(call).map_err(|e| e.to_string())?;
    let res = tools::execute_tool(tool_call);
    Ok(serde_json::to_value(res).map_err(|e| e.to_string())?)
}

/// Tool call endpoint for LLM integration.
#[tauri::command]
fn tool_call(call: serde_json::Value) -> Result<serde_json::Value, String> {
    tool_execute(call)
}

/// Vision image identification stub.
#[tauri::command]
fn vision_identify(image_path: String) -> Result<serde_json::Value, String> {
    use std::path::Path;
    if !Path::new(&image_path).exists() {
        return Err(format!("image not found: {}", image_path));
    }
    // TODO: load Qwen2.5-VL via candle-transformers
    // Current scaffold returns embedding from EmbeddingEngine
    let engine = crate::embeddings::EmbeddingEngine::new();
    let embedding = engine.embed_image(&image_path);
    Ok(serde_json::json!({
        "path": image_path,
        "tags": ["stub"],
        "description": "Vision stub – Qwen2.5-VL model loading scaffolded; returning embedding placeholder",
        "embedding": embedding
    }))
}

/// Start watching a folder for changes and index new/modified files.
#[tauri::command]
fn index_watch_start(state: State<'_, Arc<Indexer>>, folder: String) -> Result<serde_json::Value, String> {
    let indexer = state.inner().clone();
    let watcher = crate::watcher::IndexWatcher::new(indexer);
    watcher.start_watch(&folder)?;
    Ok(serde_json::json!({ "folder": folder, "status": "watching" }))
}

/// Load app config.
#[tauri::command]
fn config_load() -> Result<serde_json::Value, String> {
    let store = config::ConfigStore::new();
    let cfg = store.load()?;
    Ok(serde_json::to_value(cfg).map_err(|e| e.to_string())?)
}

/// Save app config.
#[tauri::command]
fn config_save(cfg: serde_json::Value) -> Result<(), String> {
    let cfg_obj: config::AppConfig = serde_json::from_value(cfg).map_err(|e| e.to_string())?;
    let store = config::ConfigStore::new();
    store.save(&cfg_obj)
}

/// Compute total RAM for selected model ids.
#[tauri::command]
fn config_compute_ram(state: State<'_, ModelManager>, model_ids: Vec<String>) -> Result<serde_json::Value, String> {
    let catalog = state.inner().list_models();
    let store = config::ConfigStore::new();
    let total = store.compute_total_ram_gb(&model_ids, &catalog);
    Ok(serde_json::json!({ "total_ram_gb": total }))
}

/// Embedding engine status.
#[tauri::command]
fn embedding_status() -> Result<serde_json::Value, String> {
    let engine = crate::embeddings::EmbeddingEngine::new();
    Ok(engine.status())
}

/// Embed text using the active embedding engine.
#[tauri::command]
fn embed_text(text: String) -> Result<serde_json::Value, String> {
    let engine = crate::embeddings::EmbeddingEngine::new();
    let vec = engine.embed_text(&text);
    Ok(serde_json::json!({ "embedding": vec }))
}

/// Embed image using the active embedding engine.
#[tauri::command]
fn embed_image(image_path: String) -> Result<serde_json::Value, String> {
    let engine = crate::embeddings::EmbeddingEngine::new();
    let vec = engine.embed_image(&image_path);
    Ok(serde_json::json!({ "embedding": vec }))
}

fn sha256_file(path: &PathBuf) -> Result<String, std::io::Error> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn run() {
    tauri::Builder::default()
        .manage(ModelManager::new().expect("failed to init model manager"))
        .manage(Arc::new(LlamaState::new()))
        .manage(Arc::new(Indexer::open().expect("failed to init indexer")))
        .manage(voice::VoiceManager::new())
        .invoke_handler(tauri::generate_handler![
            system_probe,
            model_list,
            model_activate,
            model_remove,
            model_verify,
            model_existing_size,
            model_download,
            model_download_cancel,
            model_presets,
            model_download_preset,
            inference::inference_load,
            inference::chat_stream,
            inference::summarize,
            index_file,
            index_folder,
            search_index,
            list_index,
            indexer_stats,
            index_remove,
            memory_create,
            memory_list,
            memory_read,
            memory_search,
            stt_transcribe,
            tts_synthesize,
            tool_execute,
            tool_call,
            config_load,
            config_save,
            config_compute_ram,
            embedding_status,
            vision_identify,
            embed_text,
            embed_image,
            index_watch_start,
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
