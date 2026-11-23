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
  /** Bytes already in partial files; > 0 on an uninstalled model means it can resume. */
  partial_bytes?: number;
  /** The model and its companion files together. */
  total_bytes?: number;
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
  /** True when the repo says its safety tuning was removed. */
  uncensored: boolean;
  created_at: string | null;
}

export type HfSort = "trending" | "downloads" | "likes" | "newest";
export type HfTag = "uncensored" | "abliterated";

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
  /** Session only: what LocalAide did before answering. */
  steps?: string[];
  /** Session only: answer statistics (developer flag). */
  stats?: { tokens: number; inputTokens?: number; outputTokens?: number; ms: number; firstTokenMs: number | null } | null;
  /** The answer in order: thinking, tool calls and text. Saved with the chat. */
  parts?: MessagePart[];
  /** Files attached to a user message, with their text. */
  attachments?: ChatAttachment[];
  /** Which model wrote an answer. */
  model?: string;
  /** The answer failed; it is not sent back as history. */
  error?: boolean;
}

export type ToolStatus = "running" | "waiting" | "done" | "failed" | "denied";

export type MessagePart =
  | { type: "text"; text: string }
  | { type: "reasoning"; text: string; ms?: number | null }
  | {
      type: "tool";
      id: string;
      name: string;
      title: string;
      detail: string;
      /** "web", "notes", "skills", "files" or "mcp". */
      group: string;
      arguments: string;
      status: ToolStatus;
      summary: string;
      output: string;
      /** Subagents: what it did, newest last. */
      steps?: string[];
      /** save_skill: the change Undo reverts, and whether it was undone. */
      skill?: { change_id: string; name: string; action: string; undone?: boolean } | null;
    }
  | {
      /** A question the model asked with `ask_user`. */
      type: "question";
      id: string;
      question: string;
      options: string[];
      multiple: boolean;
      /** Every question of the call, with option descriptions (0.1.7). */
      items?: QuestionItem[];
      /** What the user picked, once answered. */
      answer?: string | null;
      /** The user sent another message instead of answering. */
      deferred?: boolean;
      dismissed?: boolean;
    }
  | {
      /** The model's to-do list (`update_todos`), newest version. */
      type: "todos";
      items: TodoItem[];
    };

export interface QuestionOption {
  label: string;
  description: string;
}

export interface QuestionItem {
  header: string;
  question: string;
  options: QuestionOption[];
  multiple: boolean;
}

export interface TodoItem {
  text: string;
  status: "pending" | "in_progress" | "done";
}

/** A file attached to a message (chat_attach_pick / chat_attach_dropped). */
export interface ChatAttachment {
  name: string;
  /** "text" or "image". */
  kind: string;
  text?: string | null;
  data_url?: string | null;
  chars: number;
  truncated: boolean;
  error?: string | null;
}

/** The model the picker chose. No provider = the local model. */
export interface ChatTarget {
  provider_id?: string | null;
  model: string;
}

/** One model in the chat's model picker (chat_models). */
export interface ModelChoice {
  key: string;
  provider_id: string | null;
  provider_label: string;
  model: string;
  name: string;
  local: boolean;
  reasoning: boolean;
  vision: boolean;
  tools: boolean;
  context: number | null;
  /** Thinking levels, lowest first; "off" when thinking can be switched off. */
  efforts: string[];
  free: boolean;
  favorite: boolean;
  active: boolean;
  size_bytes: number | null;
}

export interface ModelGroup {
  provider_id: string;
  label: string;
  enabled: boolean;
  local_router: boolean;
  needs_refresh: boolean;
}

export interface ModelChoices {
  models: ModelChoice[];
  groups: ModelGroup[];
  active_key: string | null;
  local_loaded: boolean;
  /** Online providers are switched on in Settings. Off: none are listed. */
  online_allowed: boolean;
  /** Set after a model switch recalculated the prompt budget. */
  budget_note: string | null;
}

/** Settings → Tools and MCP. */
export interface KeyValueHint {
  name: string;
  hint: string;
}

export interface KeyValueInput {
  name: string;
  /** Omit to keep the stored value. */
  value?: string;
}

/** Public OAuth 2.1 / PKCE client metadata. A token is never part of this. */
export interface McpOAuthConfig {
  /** The MCP server id this sign-in belongs to. */
  server_id: string;
  client_id: string;
  authorize_url: string;
  token_url: string;
  redirect_uri: string;
  scopes: string[];
  audience: string | null;
}

export interface McpServer {
  id: string;
  name: string;
  enabled: boolean;
  auto_approve: boolean;
  kind: "stdio" | "http";
  command: string;
  args: string[];
  cwd: string | null;
  url: string;
  env: KeyValueHint[];
  headers: KeyValueHint[];
  /** Public client metadata, or null when this server has no sign-in. */
  oauth: McpOAuthConfig | null;
  connected: boolean;
}

export interface McpServerSaveRequest {
  id?: string;
  name: string;
  enabled: boolean;
  auto_approve: boolean;
  kind: "stdio" | "http";
  command: string;
  args: string[];
  cwd?: string | null;
  url: string;
  env: KeyValueInput[];
  headers: KeyValueInput[];
  /** Omit or send null to keep the stored settings on an edit. */
  oauth?: McpOAuthConfig | null;
}

/** Returned by mcp_oauth_start. The verifier stays on the backend. */
export interface McpOAuthStartDto {
  authorize_url: string;
  state: string;
  redirect_uri: string;
}

export interface McpOAuthFinishRequest {
  server_id: string;
  state: string;
  code: string;
}

/** Safe status from mcp_oauth_finish: no token, verifier or code. */
export interface McpOAuthStatusDto {
  server_id: string;
  signed_in: boolean;
  message: string;
  token_type: string;
  expires_in: number | null;
}

export interface McpTool {
  name: string;
  description: string;
}

export interface McpImportResult {
  added: number;
  skipped: number;
  servers: McpServer[];
}

export interface Automation {
  id: string;
  name: string;
  prompt: string;
  /** "daily" | "hourly" | "every" */
  kind: string;
  hour: number | null;
  minute: number | null;
  interval_minutes: number | null;
  enabled: boolean;
  created_at: number;
  last_run_at: number | null;
}

export interface AutomationRun {
  automation_id: string;
  name: string;
  at: number;
  ok: boolean;
  answer: string;
  ms: number;
}

export interface AutomationsState {
  automations: Automation[];
  runs: AutomationRun[];
}

export interface ArtCraftPreset {
  id: string;
  name: string;
  description: string;
  version: string;
  command: string;
  args: string[];
  cwd: string | null;
  env: Record<string, string>;
  ready: boolean;
  added: boolean;
}

export interface McpPreset {
  id: string;
  name: string;
  description: string;
  kind: "stdio" | "http";
  url: string;
  command: string;
  args: string[];
  requirement: string;
  needs_key: boolean;
  homepage: string;
}

export interface ChatThread {
  id: string;
  title: string;
  system_prompt?: string | null;
  created_at: number;
  updated_at: number;
  message_count: number;
  /** The project folder this chat works in; null for a plain chat. */
  folder?: string | null;
}

export interface ChatStreamRequest {
  message: string;
  model: string;
  system_prompt?: string;
  thread_id?: string | null;
  temperature?: number;
  max_tokens?: number;
  history?: ChatMessage[];
  images?: string[];
  /** Chat mode; the backend adds its instruction to the system prompt. */
  mode?: ChatModeId;
  /** Let the model search notes and documents (default true). */
  use_recall?: boolean;
  /** Let the model search the web (default false). */
  use_web?: boolean;
  /** Let the model read and (with approval) write workspace files (default true). */
  use_files?: boolean;
  /** Let the model read saved skills (default true). */
  use_skills?: boolean;
  /** MCP servers on for this chat; omit for every enabled server. */
  mcp_servers?: string[];
  /** Tools the user allowed for the rest of this chat. */
  allowed_tools?: string[];
  /** Let the model run terminal commands in the workspace. */
  use_shell?: boolean;
  approval_mode?: ApprovalMode;
  /** The picker's model; omit for the default. */
  target?: ChatTarget;
  /** "auto", "off", "on", "low", "medium", "high"… */
  effort?: string;
  attachments?: ChatAttachment[];
  /** The user's local date and time. */
  now?: string;
  /** The project folder the chat works in; omit for the shared workspace. */
  folder?: string | null;
  /** Let the model save and improve skills on its own (default true). */
  learn_skills?: boolean;
  /** Let the model hand work to read-only subagents (default true). */
  use_subagents?: boolean;
  /** Tell the model it can draw charts, diagrams and mockups (default true). */
  rich_output?: boolean;
}

export type ChatModeId = "standard" | "strict" | "unrestricted" | "autonomous";

export interface ChatModeInfo {
  id: ChatModeId;
  label: string;
  description: string;
  /** Text added to the system prompt. Empty for standard. */
  instruction: string;
}

export type ChatStreamEvent =
  | { type: "token"; payload: { token: string } }
  | { type: "citation"; payload: Citation }
  | { type: "status"; payload: { label: string } }
  | {
      type: "done";
      payload: {
        finish_reason: string;
        total_tokens: number;
        prompt_tokens?: number | null;
        cache_tokens?: number | null;
        /** The server's prompt count for the first round: instructions, tools, history and the new message. */
        first_prompt_tokens?: number | null;
        /** Estimated tokens of the system prompt and pinned notes. */
        system_tokens?: number;
        /** Estimated tokens of the tool schemas sent with the request. */
        tool_tokens?: number;
      };
    }
  | { type: "error"; payload: { error: string } }
  | { type: "reasoning"; payload: { text: string } }
  | { type: "steered"; payload: { text: string } }
  | {
      type: "tool_call";
      payload: { id: string; name: string; title: string; detail: string; group: string; arguments: string };
    }
  | {
      type: "tool_approval";
      payload: { approval_id: string; id: string; title: string; detail: string; preview: string };
    }
  | { type: "tool_result"; payload: { id: string; ok: boolean; summary: string; output: string } }
  | { type: "question"; payload: { id: string; question: string; options: string[]; multiple: boolean; items?: QuestionItem[] } }
  | { type: "todos"; payload: { items: TodoItem[] } }
  | { type: "subagent_step"; payload: { id: string; step: string } }
  | { type: "skill_change"; payload: { id: string; change_id: string; name: string; action: string } };

/** How much the chat asks before tools run: Plan mode, Ask before changes,
 * Autonomous (asks only when there is risk) and Full access. "chat" is
 * the 0.1.6 "Allow for this chat". */
export type ApprovalMode = "plan" | "ask" | "smart" | "auto" | "chat";

/** The model engine (llama.cpp), downloaded for this processor. */
export interface EngineInfo {
  installed: boolean;
  build: string;
  cpu: string;
  download_bytes: number | null;
  custom: boolean;
  path: string | null;
}

export interface EngineDownloadEvent {
  state: "downloading" | "ready" | "failed";
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
  message: string | null;
}

export interface Starter {
  title: string;
  prompt: string;
  uses: number;
}

export interface OpenWithApp {
  id: string;
  name: string;
}

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

export interface IndexFolderResponse {
  indexed: number;
  skipped: number;
  /** True when the folder was too big or too deep and only part was added. */
  limited: boolean;
}

/** Database file size before and after compacting, in bytes. */
export interface CompactResponse {
  bytes_before: number;
  bytes_after: number;
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

export interface UsageDay {
  date: string;
  answers: number;
  input: number;
  output: number;
}

export interface UsageModel {
  model: string;
  provider: string;
  answers: number;
  input: number;
  output: number;
  cache: number;
  steps: number;
  avg_ms: number;
  avg_tok_s: number | null;
  avg_ttft_ms: number | null;
}

/** What one model costs, USD per 1M tokens. */
export interface ModelPrice {
  input: number;
  output: number;
  cache_read: number | null;
}

export interface UsageProject {
  folder: string;
  answers: number;
  input: number;
  output: number;
  ms: number;
}

export interface UsageSummary {
  days: number;
  answers: number;
  cancelled: number;
  input_tokens: number;
  output_tokens: number;
  cache_tokens: number;
  ms: number;
  tool_calls: number;
  sessions: number;
  active_days: number;
  longest_streak: number;
  per_day: UsageDay[];
  per_model: UsageModel[];
  per_folder: UsageProject[];
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
  /** Pinned notes ride in every chat's context while recall is on. */
  pinned?: boolean;
}

/** A model-suggested note waiting in the review queue. */
export interface MemorySuggestion {
  file: string;
  title: string;
  content: string;
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

/** What the app shell reads at startup. Empty `selected_models` means first run. */
export interface AppConfigDto {
  selected_models: string[];
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
  /** Shown in the chat's model picker. */
  enabled: boolean;
  /** Models saved from the last "Load models". */
  model_count: number;
  /** A few of those models, for the card. */
  sample_models: string[];
  /** The last time LocalAide reached the provider. */
  check: ProviderCheck | null;
}

export interface ProviderCheck {
  ok: boolean;
  /** Seconds since 1970. */
  at: number;
  model_count: number;
  error: string | null;
}

/** Settings → Conversation memory. */
export interface ContextInfo {
  /** The saved setting, in tokens. */
  value: number;
  min: number;
  /** The model's own context window (slider maximum). */
  max: number;
  model_id: string | null;
  model_name: string | null;
  /** What the local engine runs with now, if a model is loaded. */
  loaded: number | null;
  /** The loaded local model, which "Reload model" restarts. */
  active_id: string | null;
}

export interface CloudConfigStatus {
  /** Master switch: online providers may be used at all. */
  online_allowed: boolean;
  /** Chat is set to an online model right now. */
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

/** Live state of the local reranker engine (search second stage). */
export interface RerankStatusResponse {
  model_id: string | null;
  ready: boolean;
}

/** One web search result. */
export interface WebHit {
  title: string;
  url: string;
  snippet: string;
  /** The provider that found it, for example "duckduckgo" or "exa". */
  source: string;
}

/** A page read from the web as text. */
export interface WebPage {
  title: string | null;
  url: string;
  content: string;
}

/** A selectable web-search provider and its local key state. */
export interface WebProviderStatus {
  id: string;
  label: string;
  needs_key: boolean;
  /** A key can be added to raise the free limits. */
  takes_key: boolean;
  supports: string;
  note: string;
  /** Firecrawl only: the user's own server address. */
  base_url: string | null;
  key_set: boolean;
  is_default: boolean;
}

/** One switch in Settings → Developer. */
export interface DevToggle {
  id: string;
  label: string;
  description: string;
  default: boolean;
}

export interface DeveloperSettings {
  enabled: boolean;
  flags: Record<string, boolean>;
  modules: Record<string, boolean>;
  flag_list: DevToggle[];
  module_list: DevToggle[];
  modes: ChatModeInfo[];
}

export interface DeveloperUpdate {
  enabled?: boolean;
  flags?: Record<string, boolean>;
  modules?: Record<string, boolean>;
}

export interface OpenCodeSettings {
  base_url: string | null;
  default_url: string;
  has_password: boolean;
}

export interface OpenCodeHealth {
  version: string | null;
}

export interface OpenCodeHandoff {
  session_id: string;
  title: string;
}

/** A curated starting point for the provider form (from the 9Router registry). */
export interface CloudProviderPreset {
  id: string;
  label: string;
  base_url: string;
  /** Empty when the user should load the model list. */
  model: string;
  key_url: string;
  note: string;
  /** False when the provider works without any key. */
  needs_key: boolean;
  /** True for routers and servers on this computer. */
  local: boolean;
  /** True when there is a free way in. */
  free: boolean;
  /** A shared public key the service expects ("public" for OpenCode Zen). */
  public_key: string;
}

/** One model a provider serves (cloud_list_models). */
export interface ProviderModel {
  id: string;
  name: string | null;
  owned_by: string | null;
  context_length: number | null;
  free: boolean;
  reasoning?: boolean | null;
  vision?: boolean | null;
  tools?: boolean | null;
}

/** A saved prompt skill: one markdown file under ~/.aiassistant/skills/. */
export interface Skill {
  name: string;
  description: string;
  tags: string[];
  body: string;
  path: string;
  /** Web address an imported skill came from. */
  source?: string | null;
  license?: string | null;
}

/** A public skill collection (skills_sources). */
export interface SkillSource {
  id: string;
  label: string;
  description: string;
  url: string;
  license_note: string;
}

/** One skill in a collection listing. */
export interface RemoteSkill {
  id: string;
  name: string;
  /** What the skill is for, when already known. */
  description: string | null;
  preview: string | null;
  url: string;
}

export interface SkillDescription {
  id: string;
  name: string;
  description: string;
  license: string | null;
}

/** A listed skill's full text. */
export interface RemoteSkillDetail {
  id: string;
  name: string;
  description: string;
  body: string;
  license: string | null;
  url: string;
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
  rerank: {
    ready: boolean;
    model_id: string | null;
  };
  library: {
    doc_count: number;
  };
  /** Cheap process counters for the About → Performance panel. */
  performance: {
    uptime_secs: number;
    rss_bytes: number;
    logs_bytes: number;
    library_db_bytes: number;
  };
  config_ok: boolean;
}

/** Result of "Check for updates" (commands/updates.rs). */
export interface UpdateCheckResponse {
  current_version: string;
  latest_version: string | null;
  update_available: boolean;
  release_url: string;
  /** True when this build can download and install the update itself. */
  can_install: boolean;
  notes: string | null;
}

/** Progress of an in-app update install. */
export interface UpdateProgressEvent {
  stage: "downloading" | "installing" | "restarting";
  downloaded: number;
  total: number | null;
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

/** What the chat screen needs to know about the local engine. */
export interface InferenceStatus {
  loaded: boolean;
  loading: boolean;
}

/** Sent on the "engine-status" event while the saved model loads at startup. */
export interface EngineStatusEvent {
  /** One of: "loading", "ready", "failed". */
  state: string;
  model_id: string | null;
  message: string | null;
}

/** Search → Library status. */
export interface LibraryFolder {
  path: string;
  added_at: number;
  last_scan_at: number;
  indexed: number;
  skipped: number;
  limited: boolean;
  documents: number;
}

export interface EmbeddingChoice {
  id: string;
  name: string;
  installed: boolean;
  size_bytes: number;
}

export interface LibraryStatus {
  folders: LibraryFolder[];
  documents: number;
  total_bytes: number;
  last_scan_at: number | null;
  search_mode: "semantic" | "keyword";
  reason: string | null;
  embedding_model: string | null;
  embedding_ready: boolean;
  embedded_chunks: number;
  total_chunks: number;
  choices: EmbeddingChoice[];
  storage_path: string;
}

export interface FolderPreset {
  id: string;
  label: string;
  path: string | null;
  exists: boolean;
  added: boolean;
}

export interface IndexProgress {
  folder: string;
  seen: number;
  indexed: number;
  skipped: number;
  current: string;
}

export interface IndexReport {
  folder: string;
  indexed: number;
  skipped: number;
  limited: boolean;
  cancelled: boolean;
  ms: number;
  reasons: [string, number][];
  skipped_files: { path: string; reason: string }[];
}

/** Settings → General → Check my setup. */
export interface SelfTestStep {
  step: "model" | "engine" | "load" | "reply";
  label: string;
  status: "running" | "pass" | "fail" | "skip";
  ms: number;
  detail: string;
  percent: number | null;
}

export interface SelfTestReport {
  passed: boolean;
  model_id: string;
  steps: SelfTestStep[];
  ms: number;
}

/** One call that left this computer. */
export interface CloudCall {
  id: number;
  timestamp: number;
  provider: string;
  host: string;
  model: string;
  kind: string;
  preview: string;
  bytes: number;
  status: string;
  detail: string | null;
}

/** Settings → Voice. */
export interface VoiceSettings {
  enabled: boolean;
  /** A local model id ("stt-whisper-tiny") or "online:<provider>:<model>:<live|batch>". */
  model: string;
  local_only: boolean;
  cleanup: boolean;
  auto_send: boolean;
  read_aloud: boolean;
  voice_name: string | null;
  rate: number;
  /** The end of each saved key, by provider id. */
  key_hints: Record<string, string>;
}

export type VoiceUpdate = Partial<Omit<VoiceSettings, "key_hints" | "voice_name">> & { voice_name?: string };

export interface LocalVoiceModel {
  id: string;
  name: string;
  params: string;
  description: string;
  license: string;
  size_bytes: number;
  installed: boolean;
}

export interface OnlineVoiceModel {
  key: string;
  provider: string;
  provider_label: string;
  host: string;
  key_url: string;
  model: string;
  label: string;
  delivery: "live" | "batch";
  note: string;
  has_key: boolean;
}

/** A program LocalAide fetches once (voice engine, cloudflared). */
export interface ProgramStatus {
  installed: boolean;
  path: string | null;
  version: string | null;
  download_bytes: number | null;
  available: boolean;
}

export interface VoiceModels {
  local: LocalVoiceModel[];
  online: OnlineVoiceModel[];
  engine: ProgramStatus;
}

export interface Transcript {
  text: string;
  model: string;
  ms: number;
  online: boolean;
  language: string | null;
}

export type LiveEvent =
  | { type: "open" }
  | { type: "interim"; text: string }
  | { type: "final"; text: string }
  | { type: "done"; text: string }
  | { type: "error"; message: string };

export interface VoiceCleanup {
  text: string;
  changed: boolean;
  skipped: string | null;
}

/** Settings → Share my assistant. */
export interface ShareStatus {
  on: boolean;
  url: string | null;
  started_at: number | null;
  requests: number;
  refused: number;
  last_seen: number | null;
  cloudflared: ProgramStatus;
}

export interface PairInfo {
  pair_code: string;
  qr_svg: string;
  on: boolean;
}

export interface ShareStarted {
  status: import("./bindings").ShareStatus;
  viewer_link: string;
  remote_link: string;
  pair_code: string;
  qr_svg: string;
  /** One-time link with the server password in the query string. */
  pairing_url: string;
  /** The same password, shown once for manual entry. */
  pairing_token: string;
}

// Local session search, reindexing and multi-run (commands/session.rs).
/** One hit from the local chat-thread index. */
export interface SessionHit {
  thread_id: string;
  title: string;
  /** Match-centred excerpt with `[` and `]` around the term. */
  snippet: string;
  /** Path of the JSON file the hit came from. */
  path: string;
}

/** One OpenAI-compatible endpoint to query in a side-by-side run. */
export interface RunTarget {
  label: string;
  endpoint: string;
  model: string;
  api_key?: string | null;
  max_tokens?: number | null;
}

/** Answer (or per-target error) from one target of a multi-run. */
export interface RunAnswer {
  label: string;
  text: string;
  error?: string | null;
}

// The read-only setup doctor (services/doctor.rs).
export interface DoctorCheck {
  name: string;
  ok: boolean;
  detail: string;
}

export interface DoctorFixReport {
  fixed: string[];
  remaining: string[];
}

export interface DoctorReport {
  /** All required checks passed; the best-effort keychain probe is excluded. */
  ok: boolean;
  checks: DoctorCheck[];
}

// Saved agent personas (services/agents.rs, commands/policy.rs).
export interface Agent {
  name: string;
  model: string;
  tools: string[];
  prompt: string;
  /** Absolute path of the backing markdown file. */
  path: string;
}

// Workspace instruction files and @path references (commands/context.rs).
export interface ContextFileHit {
  path: string;
  bytes: number;
}

export interface ContextFilesResult {
  text: string;
  files: ContextFileHit[];
  truncated: boolean;
}

export interface RefResult {
  path: string;
  text: string;
  start_line: number;
  end_line: number;
  total_lines: number;
  truncated: boolean;
}

// Policy / configuration (commands/policy.rs, config.rs).
export type PermissionDecision = "allow" | "ask" | "deny";

export interface PermissionRule {
  action: string;
  resource: string;
  decision: PermissionDecision;
}

export interface HooksConfig {
  pre_tool?: string | null;
  post_tool?: string | null;
  timeout_secs: number;
}

export interface RoutingConfig {
  provider_priority: string[];
  key_pools: Record<string, string[]>;
}

export interface PolicySettings {
  permissions: PermissionRule[];
  hooks: HooksConfig;
  routing: RoutingConfig;
  provider_order: string[];
}

/** Null/absent fields are left untouched by a partial policy update. */
export interface PolicyUpdate {
  permissions?: PermissionRule[] | null;
  hooks?: HooksConfig | null;
  routing?: RoutingConfig | null;
}

/** A tracked goal (commands/goal.rs, services/goal.rs). */
export interface Goal {
  id: string;
  thread_id: string;
  objective: string;
  /** Steps still available; a step decrements it by one. */
  budget: number;
  /** "active" | "completed" | "abandoned". */
  status: string;
  /** Unix timestamp, seconds since the epoch (UTC). */
  created_at: number;
}

/** One recorded model call (commands/usage_ledger.rs). */
export interface UsageLedgerEntry {
  id: number;
  provider: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  /** Flat estimate from the caller's USD-per-1M-tokens rate. */
  estimated_cost_usd: number;
  /** Unix timestamp, seconds since the epoch (UTC). */
  at: number;
  thread_id: string;
}

/** Arguments recorded by the caller for one model call (camelCase at IPC). */
export interface UsageRecordRequest {
  provider: string;
  model: string;
  inputTokens?: number;
  outputTokens?: number;
  threadId: string;
  ratePerMillionUsd?: number | null;
}

/** One local HTML panel reported by the upcoming `plugin_list` command.
 * `label` and `name` are both accepted while the command's shape settles. */
export interface PluginDescriptor {
  id: string;
  label?: string;
  name?: string;
  /** Absolute path to a local HTML file. */
  path: string;
}
