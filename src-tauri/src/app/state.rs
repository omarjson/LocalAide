use crate::app::jobs::JobRegistry;
use crate::config::ConfigStore;
use crate::model_manager::ModelManager;
use crate::services::agent::AgentState;
use crate::services::inference::RouterEngine;
use crate::services::library::LibraryService;
use crate::services::library::embedding::EmbeddingEngine;
use crate::services::library::rerank::RerankEngine;
use crate::services::mcp::McpManager;
use crate::services::memory::MemoryService;
use crate::services::models::ModelService;
use crate::services::skills::SkillsService;
use crate::services::skills::discover::SkillDiscovery;
use crate::services::tools::ToolService;
use crate::services::websearch::WebSearchEngine;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<JobRegistry>,
    pub model_manager: Arc<ModelManager>,
    pub model_service: Arc<ModelService>,
    pub inference_engine: Arc<RouterEngine>,
    pub embedding_engine: Arc<EmbeddingEngine>,
    pub rerank_engine: Arc<RerankEngine>,
    pub library: Arc<LibraryService>,
    pub memory: Arc<MemoryService>,
    pub skills: Arc<SkillsService>,
    /// Browses public skill collections on GitHub (only on user action).
    pub skill_discovery: Arc<SkillDiscovery>,
    pub tools: Arc<ToolService>,
    pub web: Arc<WebSearchEngine>,
    pub config_store: Arc<Mutex<ConfigStore>>,
    /// True while the saved model loads at startup.
    pub engine_loading: Arc<AtomicBool>,
    /// Sessions with the user's MCP servers (ADR-0012).
    pub mcp: Arc<McpManager>,
    /// The chat loop's approvals and what it learned about models.
    pub agent: Arc<AgentState>,
    /// Scheduled prompts (Automations screen) and their run feed.
    pub automations: Arc<crate::services::automations::AutomationsService>,
    /// Files the user dropped on the window lately. Chat reads a dropped
    /// file only when its path is in this list, so the window cannot ask
    /// for any path it likes.
    pub dropped_paths: Arc<Mutex<Vec<(std::path::PathBuf, std::time::Instant)>>>,
}

impl AppState {
    pub fn new(
        model_manager: Arc<ModelManager>,
        model_service: Arc<ModelService>,
        library: Arc<LibraryService>,
        memory: Arc<MemoryService>,
        skills: Arc<SkillsService>,
        tools: Arc<ToolService>,
    ) -> Self {
        // The embedding and reranker sidecars both resolve models through
        // the model service; they start only on explicit user action.
        let embedding_engine = Arc::new(EmbeddingEngine::new(model_service.clone()));
        let rerank_engine = Arc::new(RerankEngine::new(model_service.clone()));
        Self {
            jobs: Arc::new(JobRegistry::new()),
            model_manager,
            model_service,
            // Routes between the local llama-server sidecar and the opt-in
            // cloud engine. Starts local; startup restores the last mode.
            inference_engine: Arc::new(RouterEngine::new()),
            embedding_engine,
            rerank_engine,
            library,
            memory,
            skills,
            skill_discovery: Arc::new(SkillDiscovery::new()),
            tools,
            // Web search is stateless beyond its HTTP client; provider keys
            // are read from config per request.
            web: Arc::new(WebSearchEngine::new()),
            config_store: Arc::new(Mutex::new(ConfigStore::new())),
            engine_loading: Arc::new(AtomicBool::new(false)),
            mcp: Arc::new(McpManager::new()),
            agent: Arc::new(AgentState::new()),
            automations: Arc::new(crate::services::automations::AutomationsService::load(&crate::app::paths::app_dir())),
            dropped_paths: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
