import { invoke as tauriInvoke, Channel, type InvokeArgs } from "@tauri-apps/api/core";
import { recordRequest } from "../requestLog.svelte";

/** Every backend call goes through here, so the developer request log can time it. */
async function invoke<T>(command: string, args?: InvokeArgs): Promise<T> {
  const started = performance.now();
  try {
    const value = await tauriInvoke<T>(command, args);
    recordRequest(command, performance.now() - started, true);
    return value;
  } catch (e: unknown) {
    recordRequest(command, performance.now() - started, false, e);
    throw e;
  }
}
import type {
  AppConfigDto,
  AppError,
  BackfillResponse,
  BackupConfigDto,
  BackupEntry,
  BackupRunResponse,
  CatalogStatus,
  ChatPrefs,
  ChatMessage,
  ChatStreamEvent,
  ChatStreamRequest,
  ChatThread,
  CompactResponse,
  CloudConfigStatus,
  CloudModeRequest,
  ContextInfo,
  CloudProviderPreset,
  ProviderModel,
  CloudProviderSaveRequest,
  DownloadProgressEvent,
  EmbeddingStatusResponse,
  ForgetAllResponse,
  RerankStatusResponse,
  WebHit,
  WebPage,
  WebProviderStatus,
  SystemHealthResponse,
  UpdateCheckResponse,
  UpdateProgressEvent,
  IndexFolderResponse,
  IndexerStatsResponse,
  InferenceStatus,
  MemoryCreateRequest,
  MemoryCreateResponse,
  MemoryNote,
  MemoryUpdateRequest,
  HfCustomAddResult,
  HfFileInfo,
  HfRepo,
  RemoteSkill,
  SkillDescription,
  DeveloperSettings,
  DeveloperUpdate,
  OpenCodeSettings,
  OpenCodeHealth,
  OpenCodeHandoff,
  RemoteSkillDetail,
  SkillSource,
  HfSort,
  HfTag,
  ModelInfo,
  ModelPreset,
  ModelVerifyResult,
  RestoreReport,
  SearchIndexRequest,
  Skill,
  SkillSuggestion,
  SearchResultDoc,
  SummarizeRequest,
  SummarizeResponse,
  SystemProbeResult,
  ThreadExportRequest,
  ToolAuditEntry,
  ToolExecutionRequest,
  ToolExecutionResponse,
  WatchFolderResponse,
  ChatAttachment,
  ChatTarget,
  ModelChoices,
  McpImportResult,
  McpPreset,
  McpServer,
  McpServerSaveRequest,
  McpTool,
} from "./bindings";

export class ApiClient {
  // System
  async systemProbe(): Promise<SystemProbeResult> {
    return invoke<SystemProbeResult>("system_probe");
  }

  // Models
  async listModels(): Promise<ModelInfo[]> {
    return invoke<ModelInfo[]>("model_list");
  }

  async getCatalogStatus(): Promise<CatalogStatus> {
    return invoke<CatalogStatus>("model_catalog_status");
  }

  async refreshCatalog(): Promise<CatalogStatus> {
    return invoke<CatalogStatus>("model_catalog_refresh");
  }

  async resetCatalog(): Promise<CatalogStatus> {
    return invoke<CatalogStatus>("model_catalog_reset");
  }

  async activateModel(id: string): Promise<void> {
    return invoke<void>("model_activate", { id });
  }

  async removeModel(id: string): Promise<void> {
    return invoke<void>("model_remove", { id });
  }

  async verifyModel(id: string): Promise<ModelVerifyResult> {
    return invoke<ModelVerifyResult>("model_verify", { id });
  }

  async getModelPresets(): Promise<ModelPreset[]> {
    return invoke<ModelPreset[]>("model_presets");
  }

  async downloadModelPreset(
    id: string,
    onProgress?: (event: DownloadProgressEvent) => void
  ): Promise<void> {
    const channel = new Channel<DownloadProgressEvent>();
    if (onProgress) {
      channel.onmessage = onProgress;
    }
    return invoke<void>("model_download_preset", { id, channel });
  }

  async cancelModelDownload(id: string): Promise<void> {
    return invoke<void>("model_download_cancel", { id });
  }

  /** True when a download was running and is now stopping. */
  async pauseModelDownload(id: string): Promise<boolean> {
    return invoke<boolean>("model_download_pause", { id });
  }

  async cancelPresetDownload(id: string): Promise<void> {
    return invoke<void>("model_preset_cancel", { id });
  }

  async downloadModel(
    id: string,
    onProgress?: (event: DownloadProgressEvent) => void
  ): Promise<void> {
    const channel = new Channel<DownloadProgressEvent>();
    if (onProgress) {
      channel.onmessage = onProgress;
    }
    return invoke<void>("model_download", { id, channel });
  }

  /** Public GGUF repositories. An empty query lists everything in `sort` order. */
  async hfSearch(query: string, limit?: number, sort: HfSort = "downloads", tag: HfTag | null = null): Promise<HfRepo[]> {
    return invoke<HfRepo[]>("hf_search", { query, limit: limit ?? null, sort, tag });
  }

  async hfRepoFiles(repo: string): Promise<HfFileInfo[]> {
    return invoke<HfFileInfo[]>("hf_repo_files", { repo });
  }

  async hfCustomAdd(
    repo: string,
    path: string,
    kind: string,
    license: string | null,
    withMmproj: boolean
  ): Promise<HfCustomAddResult> {
    return invoke<HfCustomAddResult>("hf_custom_add", {
      repo,
      path,
      kind,
      license: license ?? null,
      withMmproj,
    });
  }

  // Chat & Inference
  async loadInferenceModel(id: string): Promise<string> {
    return invoke<string>("inference_load", { id });
  }

  async unloadInferenceModel(): Promise<void> {
    return invoke<void>("inference_unload");
  }

  async streamChat(
    req: ChatStreamRequest,
    onEvent: (event: ChatStreamEvent) => void
  ): Promise<string> {
    const channel = new Channel<ChatStreamEvent>();
    channel.onmessage = onEvent;
    return invoke<string>("chat_stream", { req, channel });
  }

  async summarize(req: SummarizeRequest): Promise<SummarizeResponse> {
    return invoke<SummarizeResponse>("summarize", { req });
  }

  async cancelChat(jobId: string): Promise<void> {
    return invoke<void>("chat_cancel", { jobId });
  }

  async listChatThreads(): Promise<ChatThread[]> {
    return invoke<ChatThread[]>("chat_thread_list");
  }

  async saveChatThread(
    threadId: string,
    title: string,
    systemPrompt: string | undefined | null,
    messages: ChatMessage[]
  ): Promise<void> {
    return invoke<void>("chat_thread_save", {
      threadId,
      title,
      systemPrompt,
      messages,
    });
  }

  async loadChatThread(threadId: string): Promise<ChatMessage[]> {
    return invoke<ChatMessage[]>("chat_thread_load", { threadId });
  }

  async deleteChatThread(threadId: string): Promise<void> {
    return invoke<void>("chat_thread_delete", { threadId });
  }

  /** Asks where to save the chat and writes it. Null when the user cancels. */
  async saveChatExport(req: ThreadExportRequest): Promise<string | null> {
    return invoke<string | null>("chat_thread_export_save", { req });
  }

  /** Asks where to save, then writes this Markdown. Null when the user cancels. */
  async saveChatMarkdown(fileName: string, content: string): Promise<string | null> {
    return invoke<string | null>("chat_export_text_save", { fileName, content });
  }

  // Desktop: native pickers and web links

  /** Shows the folder picker. Null when the user closes it. */
  async pickFolder(): Promise<string | null> {
    return invoke<string | null>("pick_folder");
  }

  /** Shows the file picker for document types the library reads. */
  async pickDocument(): Promise<string | null> {
    return invoke<string | null>("pick_document");
  }

  /** Opens an http(s) link in the user's browser. Other links are refused. */
  async openLink(url: string): Promise<void> {
    return invoke<void>("open_link", { url });
  }

  // Library & Search
  async indexFolder(folder: string): Promise<IndexFolderResponse> {
    return invoke<IndexFolderResponse>("index_folder", { folder });
  }

  async searchIndex(req: SearchIndexRequest): Promise<SearchResultDoc[]> {
    return invoke<SearchResultDoc[]>("search_index", {
      query: req.query,
      mode: req.mode,
      limit: req.limit,
    });
  }

  /** One page of the library, newest first (200 documents unless set). */
  async listIndex(limit?: number, offset?: number): Promise<SearchResultDoc[]> {
    return invoke<SearchResultDoc[]>("list_index", { limit, offset });
  }

  /** Shrinks the library database file. */
  async compactLibrary(): Promise<CompactResponse> {
    return invoke<CompactResponse>("library_compact");
  }

  async getIndexerStats(): Promise<IndexerStatsResponse> {
    return invoke<IndexerStatsResponse>("indexer_stats");
  }

  async removeIndexedDoc(path: string): Promise<void> {
    return invoke<void>("index_remove", { path });
  }

  async indexFile(path: string): Promise<SearchResultDoc> {
    return invoke<SearchResultDoc>("index_file", { path });
  }

  async watchFolder(folder: string): Promise<WatchFolderResponse> {
    return invoke<WatchFolderResponse>("index_watch_start", { folder });
  }

  async stopWatchingFolder(): Promise<void> {
    return invoke<void>("index_watch_stop");
  }

  // Memory
  async createMemory(req: MemoryCreateRequest): Promise<MemoryCreateResponse> {
    return invoke<MemoryCreateResponse>("memory_create", {
      title: req.title,
      content: req.content,
      tags: req.tags ?? null,
    });
  }

  async updateMemory(req: MemoryUpdateRequest): Promise<void> {
    return invoke<void>("memory_update", {
      path: req.path,
      content: req.content,
      tags: req.tags ?? null,
    });
  }

  async listMemories(): Promise<MemoryNote[]> {
    return invoke<MemoryNote[]>("memory_list");
  }

  async readMemory(path: string): Promise<string> {
    return invoke<string>("memory_read", { path });
  }

  async deleteMemory(path: string): Promise<void> {
    return invoke<void>("memory_delete", { path });
  }

  async forgetAllMemories(): Promise<ForgetAllResponse> {
    return invoke<ForgetAllResponse>("memory_forget_all");
  }

  async searchMemories(query: string): Promise<SearchResultDoc[]> {
    return invoke<SearchResultDoc[]>("memory_search", { query });
  }

  // Safe Tools & Sandbox
  async executeTool(req: ToolExecutionRequest): Promise<ToolExecutionResponse> {
    return invoke<ToolExecutionResponse>("tool_execute", { req });
  }

  async listToolAudits(limit?: number): Promise<ToolAuditEntry[]> {
    return invoke<ToolAuditEntry[]>("tool_audits_list", { limit });
  }

  async clearToolAudits(): Promise<void> {
    return invoke<void>("tool_audits_clear");
  }

  // Config & Settings
  async loadConfig(): Promise<AppConfigDto> {
    return invoke<AppConfigDto>("config_load");
  }

  // Engine context window

  async getContext(): Promise<number> {
    return invoke<number>("settings_context");
  }

  async setContext(tokens: number): Promise<number> {
    return invoke<number>("settings_set_context", { tokens: Math.round(tokens) });
  }

  /** The saved conversation memory, the model's own limit, and what is loaded now. */
  async getContextInfo(): Promise<ContextInfo> {
    return invoke<ContextInfo>("settings_context_info");
  }

  /** Share of memory models may use, in percent (20 to 90). */
  async getModelMemory(): Promise<number> {
    return invoke<number>("settings_model_memory");
  }

  async setModelMemory(percent: number): Promise<number> {
    return invoke<number>("settings_set_model_memory", { percent: Math.round(percent) });
  }

  // Local embedding engine (semantic / hybrid search)

  async getEmbeddingStatus(): Promise<EmbeddingStatusResponse> {
    return invoke<EmbeddingStatusResponse>("embedding_status");
  }

  async startEmbeddingEngine(modelId?: string): Promise<EmbeddingStatusResponse> {
    return invoke<EmbeddingStatusResponse>("embedding_start", { modelId });
  }

  async stopEmbeddingEngine(): Promise<void> {
    return invoke<void>("embedding_stop");
  }

  async backfillEmbeddings(): Promise<BackfillResponse> {
    return invoke<BackfillResponse>("embedding_backfill");
  }

  // Local reranker engine (search second stage)

  async getRerankStatus(): Promise<RerankStatusResponse> {
    return invoke<RerankStatusResponse>("rerank_status");
  }

  async startRerankEngine(modelId?: string): Promise<RerankStatusResponse> {
    return invoke<RerankStatusResponse>("rerank_start", { modelId });
  }

  async stopRerankEngine(): Promise<void> {
    return invoke<void>("rerank_stop");
  }

  // Web search (keyless + keyed providers)

  async webSearch(query: string, provider?: string, limit?: number): Promise<WebHit[]> {
    return invoke<WebHit[]>("web_search", { query, provider, limit });
  }

  async webCrawl(url: string, provider?: string): Promise<WebPage> {
    return invoke<WebPage>("web_crawl", { url, provider });
  }

  async webProviders(): Promise<WebProviderStatus[]> {
    return invoke<WebProviderStatus[]>("web_providers");
  }

  async webSetKey(provider: string, key: string | null): Promise<void> {
    return invoke<void>("web_set_key", { provider, key });
  }

  async webSetDefault(provider: string): Promise<void> {
    return invoke<void>("web_set_default", { provider });
  }

  /** Use a Firecrawl server on this computer (null clears it). */
  async webSetFirecrawlUrl(url: string | null): Promise<void> {
    return invoke<void>("web_set_firecrawl_url", { url });
  }

  // Developer mode
  async developerGet(): Promise<DeveloperSettings> {
    return invoke<DeveloperSettings>("developer_get");
  }

  async developerSet(update: DeveloperUpdate): Promise<DeveloperSettings> {
    return invoke<DeveloperSettings>("developer_set", { update });
  }

  async developerReset(): Promise<DeveloperSettings> {
    return invoke<DeveloperSettings>("developer_reset");
  }

  // OpenCode link
  async opencodeGet(): Promise<OpenCodeSettings> {
    return invoke<OpenCodeSettings>("opencode_get");
  }

  /** password: undefined keeps the saved one, "" clears it. */
  async opencodeSet(baseUrl: string | null, password?: string): Promise<OpenCodeSettings> {
    return invoke<OpenCodeSettings>("opencode_set", { baseUrl, password: password ?? null });
  }

  async opencodeCheck(): Promise<OpenCodeHealth> {
    return invoke<OpenCodeHealth>("opencode_check");
  }

  async opencodeSend(task: string, title?: string): Promise<OpenCodeHandoff> {
    return invoke<OpenCodeHandoff>("opencode_send", { task, title: title ?? null });
  }

  async cloudProviderPresets(): Promise<CloudProviderPreset[]> {
    return invoke<CloudProviderPreset[]>("cloud_provider_presets");
  }

  /** Ask a provider which models it serves. Contacts the provider. */
  async cloudListModels(baseUrl: string, apiKey?: string, providerId?: string): Promise<ProviderModel[]> {
    return invoke<ProviderModel[]>("cloud_list_models", {
      req: { base_url: baseUrl, api_key: apiKey || null, provider_id: providerId ?? null },
    });
  }

  // Theme

  async getTheme(): Promise<string | null> {
    return invoke<string | null>("settings_theme");
  }

  async setTheme(theme: "dark" | "light" | null): Promise<void> {
    return invoke<void>("settings_set_theme", { theme });
  }

  // Skills (saved prompt templates)

  async skillsList(): Promise<Skill[]> {
    return invoke<Skill[]>("skills_list");
  }

  async skillsRead(path: string): Promise<Skill> {
    return invoke<Skill>("skills_read", { path });
  }

  async skillsCreate(
    name: string,
    description: string,
    tags: string[],
    body: string
  ): Promise<Skill> {
    return invoke<Skill>("skills_create", { name, description, tags, body });
  }

  async skillsUpdate(
    path: string,
    name: string,
    description: string,
    tags: string[],
    body: string
  ): Promise<Skill> {
    return invoke<Skill>("skills_update", { path, name, description, tags, body });
  }

  async skillsDelete(path: string): Promise<void> {
    return invoke<void>("skills_delete", { path });
  }

  /** Public skill collections on GitHub. */
  async skillsSources(): Promise<SkillSource[]> {
    return invoke<SkillSource[]>("skills_sources");
  }

  /** List a collection. Contacts GitHub. `repo` is owner/name for source "github". */
  async skillsDiscover(source: string, repo?: string): Promise<RemoteSkill[]> {
    return invoke<RemoteSkill[]>("skills_discover", { source, repo: repo ?? null });
  }

  /** Descriptions for listed skills, one page at a time. */
  async skillsRemoteDescribe(source: string, ids: string[], repo?: string): Promise<SkillDescription[]> {
    return invoke<SkillDescription[]>("skills_remote_describe", { source, ids, repo: repo ?? null });
  }

  async skillsRemoteDetail(source: string, id: string, repo?: string): Promise<RemoteSkillDetail> {
    return invoke<RemoteSkillDetail>("skills_remote_detail", { source, id, repo: repo ?? null });
  }

  /** Save a listed skill on this computer. */
  async skillsImport(source: string, id: string, repo?: string): Promise<Skill> {
    return invoke<Skill>("skills_import", { source, id, repo: repo ?? null });
  }

  async skillsSuggestions(): Promise<SkillSuggestion[]> {
    return invoke<SkillSuggestion[]>("skills_suggestions");
  }

  // Persisted chat preferences

  async getChatPrefs(): Promise<ChatPrefs> {
    return invoke<ChatPrefs>("chat_prefs_get");
  }

  async setChatPrefs(prefs: ChatPrefs): Promise<void> {
    return invoke<void>("chat_prefs_set", { prefs });
  }

  // Diagnostics (local) and updates (GitHub, only when the user asks)

  async getSystemHealth(): Promise<SystemHealthResponse> {
    return invoke<SystemHealthResponse>("system_health");
  }

  async getInferenceStatus(): Promise<InferenceStatus> {
    return invoke<InferenceStatus>("inference_status");
  }

  async checkForUpdate(): Promise<UpdateCheckResponse> {
    return invoke<UpdateCheckResponse>("update_check");
  }

  /** Download, verify and install the newest version. The app restarts when it succeeds. */
  async installUpdate(onProgress: (event: UpdateProgressEvent) => void): Promise<void> {
    const channel = new Channel<UpdateProgressEvent>();
    channel.onmessage = onProgress;
    await invoke("update_install", { channel });
  }

  /** The end of a local log: LocalAide's own ("app") or the model engine's. */
  async getLogTail(lines?: number, source: "app" | "engine" = "engine"): Promise<string[]> {
    return invoke<string[]>("logs_tail", { lines, source });
  }

  // Encrypted local backups (ADR-0009)

  async getBackupConfig(): Promise<BackupConfigDto> {
    return invoke<BackupConfigDto>("backup_config_get");
  }

  async setBackupConfig(targetDir: string | null, includeApiKeys: boolean): Promise<void> {
    return invoke<void>("backup_config_set", {
      targetDir,
      includeApiKeys,
    });
  }

  async runBackup(
    passphrase: string,
    includeApiKeys?: boolean,
    targetDir?: string
  ): Promise<BackupRunResponse> {
    return invoke<BackupRunResponse>("backup_run", {
      passphrase,
      includeApiKeys,
      targetDir,
    });
  }

  async listBackups(targetDir?: string): Promise<BackupEntry[]> {
    return invoke<BackupEntry[]>("backup_list", { targetDir });
  }

  async restoreBackup(
    fileName: string,
    passphrase: string,
    restoreConfig: boolean,
    targetDir?: string
  ): Promise<RestoreReport> {
    return invoke<RestoreReport>("backup_restore", {
      fileName,
      passphrase,
      restoreConfig,
      targetDir,
    });
  }

  // Online providers (opt-in cloud mode)

  async getCloudConfig(): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_config_get");
  }

  async saveCloudProvider(req: CloudProviderSaveRequest): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_provider_save", { req });
  }

  async deleteCloudProvider(providerId: string): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_provider_delete", { providerId });
  }

  async setCloudMode(req: CloudModeRequest): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_mode_set", { req });
  }

  /** The master switch for online providers. Off sends chat back to this computer. */
  async setOnlineAllowed(enabled: boolean): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_online_set", { enabled });
  }

  /** Show or hide a provider's models in the chat's model picker. */
  async setProviderEnabled(providerId: string, enabled: boolean): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_provider_set_enabled", { req: { provider_id: providerId, enabled } });
  }

  /** Ask a saved provider for its models again (an explicit network call). */
  async refreshProviderModels(providerId: string): Promise<ProviderModel[]> {
    return invoke<ProviderModel[]>("cloud_models_refresh", { providerId });
  }

  // Model picker

  async chatModels(): Promise<ModelChoices> {
    return invoke<ModelChoices>("chat_models");
  }

  /** Make a model the one chat uses. A local model loads first. */
  async setChatTarget(target: ChatTarget): Promise<ModelChoices> {
    return invoke<ModelChoices>("chat_target_set", { req: target });
  }

  async toggleFavoriteModel(key: string): Promise<string[]> {
    return invoke<string[]>("chat_favorite_toggle", { key });
  }

  // Chat tools and attachments

  /** Answer a tool approval question: "allow", "allow_chat" or "deny". */
  async decideTool(approvalId: string, decision: "allow" | "allow_chat" | "deny"): Promise<boolean> {
    return invoke<boolean>("chat_tool_decide", { req: { approval_id: approvalId, decision } });
  }

  async attachFiles(): Promise<ChatAttachment[]> {
    return invoke<ChatAttachment[]>("chat_attach_pick");
  }

  /** Read files that were just dropped on the window. */
  async attachDropped(paths: string[]): Promise<ChatAttachment[]> {
    return invoke<ChatAttachment[]>("chat_attach_dropped", { paths });
  }

  // MCP servers

  async mcpServers(): Promise<McpServer[]> {
    return invoke<McpServer[]>("mcp_servers_list");
  }

  async mcpPresets(): Promise<McpPreset[]> {
    return invoke<McpPreset[]>("mcp_presets");
  }

  async saveMcpServer(req: McpServerSaveRequest): Promise<McpServer[]> {
    return invoke<McpServer[]>("mcp_server_save", { req });
  }

  async toggleMcpServer(id: string, change: { enabled?: boolean; auto_approve?: boolean }): Promise<McpServer[]> {
    return invoke<McpServer[]>("mcp_server_toggle", { req: { id, ...change } });
  }

  async deleteMcpServer(id: string): Promise<McpServer[]> {
    return invoke<McpServer[]>("mcp_server_delete", { id });
  }

  /** Connect and list the server's tools. */
  async testMcpServer(id: string): Promise<McpTool[]> {
    return invoke<McpTool[]>("mcp_server_test", { id });
  }

  async importMcpServers(text: string): Promise<McpImportResult> {
    return invoke<McpImportResult>("mcp_import", { text });
  }
}

export const api = new ApiClient();
export default api;

export { errorMessage } from "./errors";
