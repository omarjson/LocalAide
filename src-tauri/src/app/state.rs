use crate::app::jobs::JobRegistry;
use crate::config::ConfigStore;
use crate::model_manager::ModelManager;
use crate::services::inference::RouterEngine;
use crate::services::library::LibraryService;
use crate::services::library::embedding::EmbeddingEngine;
use crate::services::memory::MemoryService;
use crate::services::models::ModelService;
use crate::services::tools::ToolService;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<JobRegistry>,
    pub model_manager: Arc<ModelManager>,
    pub model_service: Arc<ModelService>,
    pub inference_engine: Arc<RouterEngine>,
    pub embedding_engine: Arc<EmbeddingEngine>,
    pub library: Arc<LibraryService>,
    pub memory: Arc<MemoryService>,
    pub tools: Arc<ToolService>,
    pub config_store: Arc<Mutex<ConfigStore>>,
}

impl AppState {
    pub fn new(
        model_manager: Arc<ModelManager>,
        model_service: Arc<ModelService>,
        embedding_engine: Arc<EmbeddingEngine>,
        library: Arc<LibraryService>,
        memory: Arc<MemoryService>,
        tools: Arc<ToolService>,
    ) -> Self {
        Self {
            jobs: Arc::new(JobRegistry::new()),
            model_manager,
            model_service,
            // Routes between the local llama-server sidecar and the opt-in
            // cloud engine. Starts local; startup restores the last mode.
            inference_engine: Arc::new(RouterEngine::new()),
            embedding_engine,
            library,
            memory,
            tools,
            config_store: Arc::new(Mutex::new(ConfigStore::new())),
        }
    }
}
