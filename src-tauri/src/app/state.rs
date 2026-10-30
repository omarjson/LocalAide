use std::sync::{Arc, Mutex};
use crate::app::jobs::JobRegistry;
use crate::config::ConfigStore;
use crate::indexer::Indexer;
use crate::inference::LlamaState;
use crate::model_manager::ModelManager;
use crate::services::inference::llama::LlamaWorkerEngine;
use crate::services::inference::InferenceEngine;
use crate::services::library::LibraryService;
use crate::services::memory::MemoryService;
use crate::services::models::ModelService;
use crate::services::tools::ToolService;
use crate::voice::VoiceManager;

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<JobRegistry>,
    pub model_manager: Arc<ModelManager>,
    pub model_service: Arc<ModelService>,
    pub llama_state: Arc<LlamaState>,
    pub inference_engine: Arc<dyn InferenceEngine>,
    pub library: Arc<LibraryService>,
    pub memory: Arc<MemoryService>,
    pub tools: Arc<ToolService>,
    pub indexer: Arc<Indexer>,
    pub config_store: Arc<Mutex<ConfigStore>>,
    pub voice_manager: Arc<VoiceManager>,
}

impl AppState {
    pub fn new(
        model_manager: Arc<ModelManager>,
        model_service: Arc<ModelService>,
        indexer: Arc<Indexer>,
        library: Arc<LibraryService>,
        memory: Arc<MemoryService>,
        tools: Arc<ToolService>,
        llama_state: Arc<LlamaState>,
        voice_manager: Arc<VoiceManager>,
    ) -> Self {
        Self {
            jobs: Arc::new(JobRegistry::new()),
            model_manager,
            model_service,
            llama_state,
            inference_engine: Arc::new(LlamaWorkerEngine::new()),
            library,
            memory,
            tools,
            indexer,
            config_store: Arc::new(Mutex::new(ConfigStore::new())),
            voice_manager,
        }
    }
}
