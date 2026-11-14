import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  AppConfigDto,
  AppError,
  CatalogStatus,
  ChatMessage,
  ChatStreamEvent,
  ChatStreamRequest,
  ChatThread,
  CloudConfigStatus,
  CloudModeRequest,
  CloudProviderSaveRequest,
  DownloadProgressEvent,
  ForgetAllResponse,
  IndexFolderResponse,
  IndexerStatsResponse,
  MemoryCreateRequest,
  MemoryCreateResponse,
  MemoryNote,
  MemoryUpdateRequest,
  ModelInfo,
  ModelPreset,
  ModelVerifyResult,
  RamComputationResponse,
  SearchIndexRequest,
  SearchResultDoc,
  SummarizeRequest,
  SummarizeResponse,
  SystemProbeResult,
  ThreadExportRequest,
  ThreadExportResponse,
  ToolAuditEntry,
  ToolExecutionRequest,
  ToolExecutionResponse,
  WatchFolderResponse,
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

  async exportChatThread(req: ThreadExportRequest): Promise<ThreadExportResponse> {
    return invoke<ThreadExportResponse>("chat_thread_export", { req });
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

  async listIndex(): Promise<SearchResultDoc[]> {
    return invoke<SearchResultDoc[]>("list_index");
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

  async saveConfig(cfg: AppConfigDto): Promise<void> {
    return invoke<void>("config_save", { cfg });
  }

  async computeRam(modelIds: string[]): Promise<RamComputationResponse> {
    return invoke<RamComputationResponse>("config_compute_ram", {
      model_ids: modelIds,
    });
  }

  // Engine context window

  async getContext(): Promise<number> {
    return invoke<number>("settings_context");
  }

  async setContext(tokens: number): Promise<number> {
    return invoke<number>("settings_set_context", { tokens: Math.round(tokens) });
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
}

export const api = new ApiClient();
export default api;

/**
 * Extract a readable message from a Tauri rejection, which can be a string,
 * an Error, or the serialized AppError object from the backend.
 */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (typeof e === "object" && e !== null && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}
