/**
 * Machine-generated / verified IPC bindings matching Rust domain DTOs.
 * Do NOT use `any` or `serde_json::Value` at this boundary.
 */

export interface AppError {
  code: string;
  message: string;
  detail?: string | null;
  retryable: boolean;
}

export interface SystemProbeResult {
  total_ram_gb: number;
  available_ram_gb: number;
  gpu_vendor: string | null;
  recommended_preset: string;
  budget_gb: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  filename: string;
  /** One of: "chat", "code", "embedding", "reranker". */
  kind: string;
  size_bytes: number;
  ram_estimate_gb: number;
  quant: string;
  license: string;
  sha256: string;
  download_url: string;
  /** Where the entry came from: "catalog" (curated) or "community" (HF hub). */
  source: string;
  /** True when the model ships a vision projector and accepts images. */
  vision: boolean;
  installed: boolean;
  verified: boolean;
  active: boolean;
  download_progress?: number | null;
}

/** Where the active model catalog came from (built-in or refreshed). */
export interface CatalogStatus {
  version: number;
  updated_at: number;
  model_count: number;
  /** "built-in" or "refreshed". */
  source: string;
  fetched_at: number | null;
  url: string;
}

export interface ModelPreset {
  id: string;
  name: string;
  description: string;
  models: string[];
  ram_gb: number;
}

export interface ModelVerifyResult {
  id: string;
  verified: boolean;
  expected_sha256: string;
  actual_sha256: string | null;
  error: string | null;
}

/** A public GGUF repository found by the Hugging Face search. */
export interface HfRepo {
  id: string;
  downloads: number;
  likes: number;
  gated: boolean;
  /** License tag on the repo (for example "apache-2.0"), when present. */
  license: string | null;
}

/** A pinned GGUF file inside a repository. */
export interface HfFileInfo {
  path: string;
  size: number;
  sha256: string;
  is_mmproj: boolean;
}

export interface HfCustomAddResult {
  id: string;
}

export interface ChatMessage {
  role: string;
  content: string;
  ts?: number;
  citations?: Citation[];
  /** Attached images as data URLs. Kept in the session only, not saved with the thread. */
  images?: string[];
}

export interface ChatThread {
  id: string;
  title: string;
  system_prompt?: string | null;
  created_at: number;
  updated_at: number;
  message_count: number;
}

export interface ChatStreamRequest {
  message: string;
  model: string;
  system_prompt?: string;
  temperature?: number;
  max_tokens?: number;
  history?: ChatMessage[];
  images?: string[];
}

export type ChatStreamEvent =
  | { type: "token"; payload: { token: string } }
  | { type: "citation"; payload: Citation }
  | { type: "done"; payload: { finish_reason: string; total_tokens: number } }
  | { type: "error"; payload: { error: string } };

export interface SummarizeRequest {
  text: string;
  max_words?: number;
}

export interface SummarizeResponse {
  summary: string;
}

export interface ThreadExportRequest {
  thread_id: string;
  format: "markdown" | "json";
}

export interface ThreadExportResponse {
  content: string;
  filename: string;
}

export interface IndexFolderResponse {
  indexed: number;
  skipped: number;
}

export interface SearchIndexRequest {
  query: string;
  mode?: "semantic" | "keyword" | "hybrid";
  limit?: number;
}

export interface SearchResultDoc {
  path: string;
  title: string;
  snippet: string;
  score: number;
  modified: number;
  size: number;
}

export interface IndexerStatsResponse {
  doc_count: number;
  total_bytes: number;
  storage_path: string;
}

export interface WatchFolderResponse {
  folder: string;
  status: string;
}

export interface MemoryCreateRequest {
  title: string;
  content: string;
  tags?: string[];
}

export interface MemoryUpdateRequest {
  path: string;
  content: string;
  tags?: string[];
}

export interface MemoryNote {
  id?: string;
  path: string;
  title: string;
  content?: string | null;
  tags?: string[];
  size: number;
  modified: number;
}

export interface MemoryCreateResponse {
  path: string;
}

export interface Citation {
  index: number;
  source: string;
  title: string;
  snippet: string;
  score: number;
}

export interface ForgetAllResponse {
  deleted_count: number;
}

export type ToolActionType = "list_dir" | "read_file" | "write_file" | "search_files";

export interface ToolAction {
  action_type: ToolActionType;
  path: string;
  content?: string;
  query?: string;
}

export interface ToolExecutionRequest {
  tool_name: string;
  action: ToolAction;
  approved?: boolean;
  session_token?: string;
}

export interface ToolExecutionResponse {
  success: boolean;
  output: string;
  truncated: boolean;
  requires_approval: boolean;
  error: string | null;
}

export interface ToolAuditEntry {
  id: number;
  timestamp: number;
  tool_name: string;
  action: string;
  path: string;
  approved: boolean;
  status: string;
  details: string | null;
}

export interface UserPresetDto {
  id: string;
  name: string;
  model_ids: string[];
  total_ram_gb: number;
}

export interface AppConfigDto {
  selected_models: string[];
  presets: UserPresetDto[];
  ram_budget_mb: number;
  cloud_llm_enabled: boolean;
}

export interface RamComputationResponse {
  total_ram_gb: number;
}

export interface DownloadProgressEvent {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
  speed_bps: number;
  status: string;
}

// Online providers (opt-in cloud mode)
export interface CloudProviderDto {
  id: string;
  label: string;
  base_url: string;
  model: string;
  api_key_hint: string;
  is_active: boolean;
}

export interface CloudConfigStatus {
  cloud_enabled: boolean;
  providers: CloudProviderDto[];
  active_provider_id: string | null;
}

export interface CloudProviderSaveRequest {
  id?: string;
  label: string;
  base_url: string;
  model: string;
  /** Full key. Omit when editing to keep the stored key. */
  api_key?: string;
}

export interface CloudModeRequest {
  enabled: boolean;
  provider_id?: string;
}

/** Live state of the local embedding engine (semantic/hybrid search). */
export interface EmbeddingStatusResponse {
  /** "llama-server" when running, "not-running" otherwise. */
  engine: string;
  model_id: string | null;
  dimension: number | null;
  ready: boolean;
  total_chunks: number;
  embedded_chunks: number;
}

export interface BackfillResponse {
  updated: number;
  total_chunks: number;
  embedded_chunks: number;
}

/** A saved prompt skill: one markdown file under ~/.aiassistant/skills/. */
export interface Skill {
  name: string;
  description: string;
  tags: string[];
  body: string;
  path: string;
}

/** A custom prompt reused often enough for the save-as-skill hint. */
export interface SkillSuggestion {
  prompt: string;
  use_count: number;
}

/** Chat preferences persisted across sessions. */
export interface ChatPrefs {
  system_prompt: string | null;
  temperature: number | null;
  max_tokens: number | null;
  persona: string | null;
}

/** Aggregate local-subsystem health snapshot. */
export interface SystemHealthResponse {
  app_version: string;
  catalog_version: number;
  engine: {
    loaded: boolean;
    active_model: string | null;
    context_size: number;
    sidecar_binary_found: boolean;
  };
  embedding: {
    ready: boolean;
    model_id: string | null;
    dimension: number | null;
    total_chunks: number;
    embedded_chunks: number;
  };
  library: {
    doc_count: number;
  };
  config_ok: boolean;
}

/** Result of the display-only update check against GitHub Releases. */
export interface UpdateCheckResponse {
  current_version: string;
  latest_version: string | null;
  update_available: boolean;
  release_url: string;
}

/** Encrypted local backup settings (ADR-0009). */
export interface BackupConfigDto {
  target_dir: string | null;
  include_api_keys: boolean;
  last_backup_at: number | null;
}

export interface BackupEntry {
  file_name: string;
  created_at: number;
  size_bytes: number;
  app_version: string;
}

export interface BackupRunResponse {
  path: string;
  created_at: number;
  memories: number;
  conversations: number;
  api_keys_included: boolean;
}

export interface RestoreReport {
  memories_restored: number;
  conversations_restored: number;
  config_restored: boolean;
  skipped_newer_config: boolean;
}
