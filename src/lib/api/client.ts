import { invoke as tauriInvoke, Channel, type InvokeArgs } from "@tauri-apps/api/core";
import { recordRequest } from "../requestLog.svelte";
import { notifyGoalComplete, notifyQuestion, notifyToolApproval } from "../notifications.svelte";

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
  PairInfo,
  AppError,
  BackfillResponse,
  BackupConfigDto,
  BackupEntry,
  BackupRunResponse,
  CatalogStatus,
  ChatPrefs,
  ChatMessage,
  ModelPrice,
  ChatStreamEvent,
  ChatStreamRequest,
  ChatThread,
  EngineInfo,
  OpenWithApp,
  Starter,
  CompactResponse,
  CloudConfigStatus,
  CloudModeRequest,
  ApprovalTimers,
  ContextInfo,
  EngineConfig,
  FoundServer,
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
  MemorySuggestion,
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
  UsageSummary,
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
  McpOAuthFinishRequest,
  McpOAuthStartDto,
  McpOAuthStatusDto,
  ArtCraftPreset,
  Automation,
  AutomationsState,
  McpServerSaveRequest,
  McpTool,
  LibraryStatus,
  FolderPreset,
  IndexProgress,
  IndexReport,
  SelfTestStep,
  SelfTestReport,
  CloudCall,
  VoiceSettings,
  VoiceUpdate,
  VoiceModels,
  ProgramStatus,
  Transcript,
  LiveEvent,
  VoiceCleanup,
  ShareStatus,
  ShareStarted,
  SessionHit,
  RunTarget,
  RunAnswer,
  DoctorReport,
  DoctorFixReport,
  Agent,
  ContextFileHit,
  ContextFilesResult,
  RefResult,
  PolicySettings,
  PolicyUpdate,
  Goal,
  UsageLedgerEntry,
  UsageRecordRequest,
  PluginDescriptor,
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
    // Notifications ride the real stream event tags — no invented Tauri
    // events, and the caller still receives every event unchanged.
    channel.onmessage = (event) => {
      if (event.type === "tool_approval") {
        notifyToolApproval(`LocalAide wants to ${event.payload.title.toLowerCase()}.`);
      } else if (event.type === "question") {
        notifyQuestion(event.payload.question);
      }
      onEvent(event);
    };
    return invoke<string>("chat_stream", { req, channel });
  }

  async summarize(req: SummarizeRequest): Promise<SummarizeResponse> {
    return invoke<SummarizeResponse>("summarize", { req });
  }

  async cancelChat(jobId: string): Promise<void> {
    return invoke<void>("chat_cancel", { jobId });
  }

  /** A note for a running answer. False when the answer already finished. */
  async steerChat(jobId: string, text: string): Promise<boolean> {
    return invoke<boolean>("chat_steer", { req: { job_id: jobId, text } });
  }

  async listChatThreads(): Promise<ChatThread[]> {
    return invoke<ChatThread[]>("chat_thread_list");
  }

  async saveChatThread(
    threadId: string,
    title: string,
    systemPrompt: string | undefined | null,
    messages: ChatMessage[],
    folder?: string | null,
  ): Promise<void> {
    return invoke<void>("chat_thread_save", {
      threadId,
      title,
      systemPrompt,
      messages,
      folder: folder ?? null,
    });
  }

  /** Rewrite a draft into a clearer prompt with the chat's model. */
  async enhancePrompt(text: string, target?: ChatTarget): Promise<string> {
    return invoke<string>("chat_enhance_prompt", { text, target: target ?? null });
  }

  /** Up to three suggested next messages. */
  async suggestNext(question: string, answer: string, target?: ChatTarget): Promise<string[]> {
    return invoke<string[]>("chat_suggest_next", { question, answer, target: target ?? null });
  }

  /** A short AI title for the chat, or null when the model declined. */
  async chatTitle(question: string, answer: string, target?: ChatTarget): Promise<string | null> {
    return invoke<string | null>("chat_title", { question, answer, target: target ?? null });
  }

  /** Starter prompts learned from how the user's saved chats begin. */
  async chatStarters(): Promise<Starter[]> {
    return invoke<Starter[]>("chat_starters");
  }

  /** Undo a skill the model saved or improved. Returns its name. */
  async skillsUndo(changeId: string): Promise<string> {
    return invoke<string>("skills_undo", { changeId });
  }

  async openFile(path: string, folder?: string | null): Promise<void> {
    return invoke<void>("files_open", { path, folder: folder ?? null });
  }

  /** Show a file, or the chat's folder for an empty path, in the file manager. */
  async revealFile(path: string, folder?: string | null): Promise<void> {
    return invoke<void>("files_reveal", { path, folder: folder ?? null });
  }

  async openWithApps(): Promise<OpenWithApp[]> {
    return invoke<OpenWithApp[]>("files_open_with_apps");
  }

  async openFileWith(path: string, folder: string | null | undefined, appId: string): Promise<void> {
    return invoke<void>("files_open_with", { path, folder: folder ?? null, appId });
  }

  async uiScale(): Promise<number> {
    return invoke<number>("ui_scale_get");
  }

  async setUiScale(scale: number): Promise<number> {
    return invoke<number>("ui_scale_set", { scale });
  }

  /** Keep a mockup page for the sandboxed frame; returns its id. */
  async renderFrame(html: string): Promise<string> {
    return invoke<string>("render_frame_put", { html });
  }

  async engineInfo(): Promise<EngineInfo> {
    return invoke<EngineInfo>("engine_info");
  }

  async engineFetch(): Promise<EngineInfo> {
    return invoke<EngineInfo>("engine_fetch");
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

  /** Aggregated answer usage. days 1 = today, 0 = all time, else the rolling window. */
  async usageSummary(days?: number, folder?: string | null): Promise<UsageSummary> {
    return invoke<UsageSummary>("usage_summary", { days: days ?? null, folder: folder ?? null });
  }

  /** Model prices from OpenRouter, USD per 1M tokens, keyed by model id. */
  async openrouterModelPrices(): Promise<Record<string, ModelPrice>> {
    return invoke<Record<string, ModelPrice>>("openrouter_model_prices");
  }

  // Library status, folder presets and indexing with progress

  async libraryStatus(): Promise<LibraryStatus> {
    return invoke<LibraryStatus>("library_status");
  }

  async libraryFolderPresets(): Promise<FolderPreset[]> {
    return invoke<FolderPreset[]>("library_folder_presets");
  }

  async libraryIndex(folder: string, onProgress: (p: IndexProgress) => void): Promise<IndexReport> {
    const channel = new Channel<IndexProgress>();
    channel.onmessage = onProgress;
    return invoke<IndexReport>("library_index_start", { folder, channel });
  }

  async libraryIndexCancel(): Promise<boolean> {
    return invoke<boolean>("library_index_cancel");
  }

  async libraryFolderRemove(path: string): Promise<number> {
    return invoke<number>("library_folder_remove", { path });
  }

  /** Choose the meaning model for search; it starts and re-embeds the library. */
  async libraryEmbeddingSelect(modelId: string): Promise<LibraryStatus> {
    return invoke<LibraryStatus>("library_embedding_select", { modelId });
  }

  // Voice

  async voiceSettings(): Promise<VoiceSettings> {
    return invoke<VoiceSettings>("voice_settings_get");
  }

  async setVoiceSettings(update: VoiceUpdate): Promise<VoiceSettings> {
    return invoke<VoiceSettings>("voice_settings_set", { update });
  }

  /** Save the key for an online voice provider; an empty key removes it. */
  async setVoiceKey(provider: string, key: string): Promise<VoiceSettings> {
    return invoke<VoiceSettings>("voice_key_set", { provider, key });
  }

  async voiceModels(): Promise<VoiceModels> {
    return invoke<VoiceModels>("voice_models");
  }

  async fetchVoiceEngine(onProgress: (e: DownloadProgressEvent) => void): Promise<ProgramStatus> {
    const channel = new Channel<DownloadProgressEvent>();
    channel.onmessage = onProgress;
    return invoke<ProgramStatus>("voice_engine_fetch", { channel });
  }

  /** A finished recording as base64 WAV (16 kHz mono 16-bit). */
  async transcribe(audioBase64: string): Promise<Transcript> {
    return invoke<Transcript>("voice_transcribe", { audio: audioBase64 });
  }

  async voiceLiveStart(onEvent: (e: LiveEvent) => void): Promise<string> {
    const channel = new Channel<LiveEvent>();
    channel.onmessage = onEvent;
    return invoke<string>("voice_live_start", { channel });
  }

  async voiceLivePush(session: string, pcmBase64: string): Promise<boolean> {
    return invoke<boolean>("voice_live_push", { session, pcm: pcmBase64 });
  }

  async voiceLiveStop(session: string): Promise<boolean> {
    return invoke<boolean>("voice_live_stop", { session });
  }

  async voiceCleanup(text: string): Promise<VoiceCleanup> {
    return invoke<VoiceCleanup>("voice_cleanup", { text });
  }

  // Share my assistant

  async shareStatus(): Promise<ShareStatus> {
    return invoke<ShareStatus>("share_status");
  }

  async shareStart(tunnel = true, lan = false): Promise<ShareStarted> {
    return invoke<ShareStarted>("share_start", { tunnel, lan });
  }

  async shareStop(): Promise<ShareStatus> {
    return invoke<ShareStatus>("share_stop");
  }

  /** The pairing card: QR of the remote link plus the XXXX-XXXX code. */
  async sharePair(): Promise<PairInfo> {
    return invoke<PairInfo>("share_pair");
  }

  async fetchCloudflared(onProgress: (e: DownloadProgressEvent) => void): Promise<ProgramStatus> {
    const channel = new Channel<DownloadProgressEvent>();
    channel.onmessage = onProgress;
    return invoke<ProgramStatus>("share_cloudflared_fetch", { channel });
  }

  // Check my setup

  async selfTest(onStep: (s: SelfTestStep) => void): Promise<SelfTestReport> {
    const channel = new Channel<SelfTestStep>();
    channel.onmessage = onStep;
    return invoke<SelfTestReport>("selftest_run", { channel });
  }

  // What left this computer

  async cloudAuditList(limit = 200): Promise<CloudCall[]> {
    return invoke<CloudCall[]>("cloud_audit_list", { limit });
  }

  async cloudAuditClear(): Promise<number> {
    return invoke<number>("cloud_audit_clear");
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

  /** Pins or unpins a note; pinned notes ride in every chat with recall on. */
  async setMemoryPinned(path: string, pinned: boolean): Promise<MemoryNote> {
    return invoke<MemoryNote>("memory_pinned_set", { path, pinned });
  }

  /** The model's suggested notes, waiting for review. */
  async pendingSuggestions(): Promise<MemorySuggestion[]> {
    return invoke<MemorySuggestion[]>("memory_pending_list");
  }

  /** Accepts a suggestion: it becomes a real note. */
  async acceptSuggestion(file: string, tags?: string[]): Promise<MemoryNote> {
    return invoke<MemoryNote>("memory_pending_accept", { file, tags: tags ?? null });
  }

  /** Dismisses a suggestion without saving it. */
  async dismissSuggestion(file: string): Promise<boolean> {
    return invoke<boolean>("memory_pending_dismiss", { file });
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

  /** The setup wizard finished or was skipped; setup must not reappear. */
  async markSetupComplete(): Promise<void> {
    return invoke<void>("config_setup_complete");
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

  /** Whether the compact prompt is on (Settings → Performance). */
  async getCompactPrompt(): Promise<boolean> {
    return invoke<boolean>("settings_compact_prompt");
  }

  /** Turn the compact prompt on or off. Local models only. */
  async setCompactPrompt(on: boolean): Promise<boolean> {
    return invoke<boolean>("settings_set_compact_prompt", { on });
  }

  /** llama-server flags tuned in Settings → Performance → Engine flags. */
  async getEngineFlags(): Promise<EngineConfig> {
    return invoke<EngineConfig>("settings_engine_flags");
  }

  /** Save engine flags. They apply the next time a model loads. */
  async setEngineFlags(flags: EngineConfig): Promise<EngineConfig> {
    return invoke<EngineConfig>("settings_set_engine_flags", { flags });
  }

  /** Chat servers already running on this computer. Read-only probes of
   *  well-known localhost ports; nothing leaves the machine. */
  async discoverLocalServers(): Promise<FoundServer[]> {
    return invoke<FoundServer[]>("local_servers_discover");
  }

  /** Approval timers: plan auto-accept and the terminal countdown, in seconds (0 = no timer). */
  async getApprovalTimers(): Promise<ApprovalTimers> {
    return invoke<ApprovalTimers>("settings_approval_timers");
  }

  /** Save the approval timers. The backend clamps each to 0..600 seconds. */
  async setApprovalTimers(timers: ApprovalTimers): Promise<ApprovalTimers> {
    return invoke<ApprovalTimers>("settings_set_approval_timers", { timers });
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

  /** Test connection: ask a saved provider for its models with its saved key. */
  async testCloudProvider(providerId: string): Promise<CloudConfigStatus> {
    return invoke<CloudConfigStatus>("cloud_provider_test", { providerId });
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

  async mcpArtcraftPresets(): Promise<ArtCraftPreset[]> {
    return invoke<ArtCraftPreset[]>("mcp_artcraft_presets");
  }

  async automationsList(): Promise<AutomationsState> {
    return invoke<AutomationsState>("automations_list");
  }

  async automationsSave(automation: Automation): Promise<AutomationsState> {
    return invoke<AutomationsState>("automations_save", { automation });
  }

  async automationsDelete(id: string): Promise<AutomationsState> {
    return invoke<AutomationsState>("automations_delete", { id });
  }

  async automationsRunNow(id: string): Promise<boolean> {
    return invoke<boolean>("automations_run_now", { id });
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

  /** Start PKCE sign-in: returns the browser URL and the one-time state. */
  async mcpOAuthStart(serverId: string): Promise<McpOAuthStartDto> {
    return invoke<McpOAuthStartDto>("mcp_oauth_start", {
      req: { server_id: serverId },
    });
  }

  /** Exchange the pasted callback code. Returns status only, never a token. */
  async mcpOAuthFinish(req: McpOAuthFinishRequest): Promise<McpOAuthStatusDto> {
    return invoke<McpOAuthStatusDto>("mcp_oauth_finish", { req });
  }

  // Local session search, reindexing and side-by-side model runs (session.rs).

  /** FTS hits over saved chats. The index is created on demand; run reindex to fill it. */
  async sessionSearch(query: string, limit?: number): Promise<SessionHit[]> {
    return invoke<SessionHit[]>("session_search", { query, limit: limit ?? null });
  }

  /** Rebuild the thread index from the conversations folder; returns the count. */
  async sessionReindex(): Promise<number> {
    return invoke<number>("session_reindex");
  }

  /** Send one prompt to several OpenAI-compatible endpoints at once. */
  async multiRunChat(targets: RunTarget[], prompt: string): Promise<RunAnswer[]> {
    return invoke<RunAnswer[]>("multi_run_chat", { targets, prompt });
  }

  // Read-only local setup doctor (doctor.rs).
  async doctorRun(): Promise<DoctorReport> {
    return invoke<DoctorReport>("doctor_run");
  }

  /** Repairs only missing local scaffolding; never deletes user data. */
  async doctorFix(): Promise<DoctorFixReport> {
    return invoke<DoctorFixReport>("doctor_fix");
  }

  // Saved agent personas (policy.rs).
  async agentsList(): Promise<Agent[]> {
    return invoke<Agent[]>("agents_list");
  }

  /** Add or update an agent (a matching name is overwritten in place). */
  async agentsSave(
    name: string,
    model?: string | null,
    tools?: string[],
    prompt?: string,
  ): Promise<Agent> {
    return invoke<Agent>("agents_save", {
      name,
      model: model ?? null,
      tools: tools ?? [],
      prompt: prompt ?? "",
    });
  }

  // Workspace instruction files and @path references (context.rs).
  async contextFilesList(roots?: string[], maxBytes?: number): Promise<ContextFilesResult> {
    return invoke<ContextFilesResult>("context_files_list", {
      roots: roots ?? null,
      maxBytes: maxBytes ?? null,
    });
  }

  async contextRefsResolve(
    reference: string,
    roots?: string[],
    maxBytes?: number,
  ): Promise<RefResult> {
    return invoke<RefResult>("context_refs_resolve", {
      reference,
      roots: roots ?? null,
      maxBytes: maxBytes ?? null,
    });
  }

  // Policy / configuration (policy.rs).
  async policyConfigGet(): Promise<PolicySettings> {
    return invoke<PolicySettings>("policy_config_get");
  }

  async policyConfigSet(update: PolicyUpdate): Promise<PolicySettings> {
    return invoke<PolicySettings>("policy_config_set", { update });
  }

  // Goals (commands/goal.rs). All state is local; no model call on this path.
  async goalStart(threadId: string, objective: string, budget: number): Promise<Goal> {
    return invoke<Goal>("goal_start", { threadId, objective, budget });
  }

  async goalStatus(id: string): Promise<Goal | null> {
    return invoke<Goal | null>("goal_status", { id });
  }

  async goalStep(id: string, progress: string): Promise<Goal> {
    return invoke<Goal>("goal_step", { id, progress });
  }

  async goalComplete(id: string): Promise<Goal> {
    const goal = await invoke<Goal>("goal_complete", { id });
    notifyGoalComplete(`Goal finished: ${goal.objective}`);
    return goal;
  }

  /** Pause a goal: progress stopped, but it is not done. */
  async goalPause(id: string): Promise<Goal> {
    return invoke<Goal>("goal_pause", { id });
  }

  /** Mark a goal blocked: it waits on something outside the run. */
  async goalBlock(id: string): Promise<Goal> {
    return invoke<Goal>("goal_block", { id });
  }

  /** Resume a paused or blocked goal. */
  async goalResume(id: string): Promise<Goal> {
    return invoke<Goal>("goal_resume", { id });
  }

  // Usage ledger (commands/usage_ledger.rs). The rate is caller-supplied;
  // the backend never invents a per-model price.
  async usageRecord(req: UsageRecordRequest): Promise<UsageLedgerEntry> {
    return invoke<UsageLedgerEntry>("usage_record", {
      provider: req.provider,
      model: req.model,
      inputTokens: req.inputTokens ?? 0,
      outputTokens: req.outputTokens ?? 0,
      threadId: req.threadId,
      ratePerMillionUsd: req.ratePerMillionUsd ?? null,
    });
  }

  async usageList(threadId?: string | null, limit?: number): Promise<UsageLedgerEntry[]> {
    return invoke<UsageLedgerEntry[]>("usage_list", {
      threadId: threadId ?? null,
      limit: limit ?? null,
    });
  }

  // Local HTML plugin panels (added by the plugin_list command; absent builds
  // simply reject, and the panel shows an honest empty state).
  async pluginList(): Promise<PluginDescriptor[]> {
    return invoke<PluginDescriptor[]>("plugin_list");
  }
}

export const api = new ApiClient();
export default api;

export { errorMessage } from "./errors";
