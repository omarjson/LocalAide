use crate::app::jobs::JobRegistry;
use crate::config::ConfigStore;
use crate::indexer::Indexer;
use crate::model_manager::ModelManager;
use crate::services::inference::sidecar::SidecarEngine;
use crate::services::inference::InferenceEngine;
use crate::services::library::LibraryService;
use crate::services::memory::MemoryService;
use crate::services::models::ModelService;
use crate::services::tools::ToolService;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<JobRegistry>,
    pub model_manager: Arc<ModelManager>,
    pub model_service: Arc<ModelService>,
    pub inference_engine: Arc<dyn InferenceEngine>,
    pub library: Arc<LibraryService>,
    pub memory: Arc<MemoryService>,
    pub tools: Arc<ToolService>,
    pub indexer: Arc<Indexer>,
    pub config_store: Arc<Mutex<ConfigStore>>,
}

impl AppState {
    pub fn new(
        model_manager: Arc<ModelManager>,
        model_service: Arc<ModelService>,
        indexer: Arc<Indexer>,
        library: Arc<LibraryService>,
        memory: Arc<MemoryService>,
        tools: Arc<ToolService>,
    ) -> Self {
        Self {
            jobs: Arc::new(JobRegistry::new()),
            model_manager,
            model_service,
            inference_engine: Arc::new(SidecarEngine::new()),
            library,
            memory,
            tools,
            indexer,
            config_store: Arc::new(Mutex::new(ConfigStore::new())),
        }
    }
}
