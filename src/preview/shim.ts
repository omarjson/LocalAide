/**
 * Fake Tauri IPC backend for the browser preview (development only).
 *
 * Installs `window.__TAURI_INTERNALS__` before the app boots, so
 * `@tauri-apps/api` invoke/Channel/event calls resolve against fixtures
 * instead of the Rust backend. Startup-critical commands have real fixtures;
 * everything else falls back to a safe default and logs, so a screen can be
 * styled before its data exists.
 */

type Handler = (args: Record<string, unknown>) => unknown;

const now = Date.now();
const ago = (minutes: number) => now - minutes * 60_000;
// Memory timestamps cross the bridge in seconds, like the Rust backend.
const sec = (minutes: number) => Math.floor(ago(minutes) / 1000);

const MODEL_QWEN = {
  id: "qwen3.5-9b",
  name: "Qwen3.5 9B Instruct (Q4_K_M)",
  filename: "qwen3.5-9b-instruct-q4_k_m.gguf",
  kind: "chat",
  size_bytes: 5_600_000_000,
  ram_estimate_gb: 6.2,
  quant: "Q4_K_M",
  license: "Apache-2.0",
  sha256: "a".repeat(64),
  download_url: "https://huggingface.co/fake/qwen3.5-9b/resolve/main/model.gguf",
  source: "catalog",
  vision: false,
  installed: true,
  verified: true,
  active: true,
  download_progress: null,
};

const MODEL_GEMMA = {
  ...MODEL_QWEN,
  id: "gemma-4-e4b",
  name: "Gemma 4 E4B (QAT)",
  filename: "gemma-4-e4b-qat.gguf",
  size_bytes: 4_100_000_000,
  ram_estimate_gb: 4.8,
  quant: "QAT INT4",
  license: "Gemma",
  active: false,
};

const MODEL_VISION = {
  ...MODEL_QWEN,
  id: "spark-x2.6-3b-vision",
  name: "Spark-X2.6 3B Vision",
  filename: "spark-x2.6-3b-vision-q4.gguf",
  size_bytes: 2_200_000_000,
  ram_estimate_gb: 2.9,
  vision: true,
  active: false,
};

const MODEL_TINY = {
  ...MODEL_QWEN,
  id: "qwen3.5-0.8b",
  name: "Qwen3.5 0.8B (Q4_K_M)",
  filename: "qwen3.5-0.8b-q4_k_m.gguf",
  size_bytes: 600_000_000,
  ram_estimate_gb: 1.0,
  installed: false,
  verified: false,
  active: false,
};

const MODEL_EMBED = {
  ...MODEL_QWEN,
  id: "embeddinggemma-300m",
  name: "EmbeddingGemma 300M",
  filename: "embeddinggemma-300m.gguf",
  kind: "embedding",
  size_bytes: 250_000_000,
  ram_estimate_gb: 0.5,
  vision: false,
  active: false,
};

const MODEL_RERANK = {
  ...MODEL_QWEN,
  id: "bge-reranker-v2",
  name: "BGE Reranker v2 M3",
  filename: "bge-reranker-v2-m3.gguf",
  kind: "reranker",
  size_bytes: 560_000_000,
  ram_estimate_gb: 1.2,
  vision: false,
  active: false,
};

const localChoice = (over: Record<string, unknown> = {}) => ({
  key: "local:qwen3.5-9b",
  provider_id: null,
  provider_label: "On this computer",
  model: "qwen3.5-9b",
  name: "Qwen3.5 9B Instruct",
  local: true,
  reasoning: true,
  vision: false,
  tools: true,
  context: 32768,
  efforts: ["off", "low", "medium", "high"],
  free: true,
  favorite: true,
  active: true,
  size_bytes: MODEL_QWEN.size_bytes,
  ...over,
});

// A fake online provider, so the picker's filters and the full effort
// ladder have something to act on in the browser harness.
const remoteChoice = (over: Record<string, unknown> = {}) => ({
  key: "kilo:auto-free",
  provider_id: "kilo",
  provider_label: "Kilo Code (free, no key)",
  model: "auto-free",
  name: "Auto Free",
  local: false,
  reasoning: true,
  vision: false,
  tools: true,
  context: 256000,
  efforts: ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"],
  free: true,
  favorite: false,
  active: false,
  size_bytes: null,
  ...over,
});

type ModelChoicesFixture = {
  models: Array<Record<string, unknown>>;
  groups: Array<Record<string, unknown>>;
  active_key: string;
  local_loaded: boolean;
  online_allowed: boolean;
};

const chatModelsFixture = (): ModelChoicesFixture => ({
  models: [
    localChoice(),
    localChoice({ key: "local:gemma-4-e4b", model: "gemma-4-e4b", name: "Gemma 4 E4B (QAT)", context: 131072, active: false, favorite: false }),
    localChoice({ key: "local:spark-x2.6-3b-vision", model: "spark-x2.6-3b-vision", name: "Spark-X2.6 3B Vision", vision: true, context: 16384, active: false, favorite: false }),
    remoteChoice({ key: "kilo:nvidia/nemotron-3.5-super-vl:free", model: "nvidia/nemotron-3.5-super-vl:free", name: "NVIDIA: Nemotron 3.5 Super VL (free)", vision: true, context: 1000000 }),
    remoteChoice({ key: "kilo:anthropic/claude-opus-5.5", model: "anthropic/claude-opus-5.5", name: "Anthropic: Claude Opus 5.5", vision: true, context: 200000, efforts: ["off", "low", "medium", "high"] }),
    remoteChoice({ key: "kilo:deepseek/deepseek-v4.1-flash-1017", model: "deepseek/deepseek-v4.1-flash-1017", name: "DeepSeek: DeepSeek V4.1 Flash 1017", vision: true, context: 128000 }),
  ],
  groups: [
    { provider_id: "kilo", label: "Kilo Code (free, no key)", enabled: true, local_router: false, needs_refresh: false },
  ],
  active_key: "local:qwen3.5-9b",
  local_loaded: true,
  online_allowed: true,
});

// A month of plausible answer history, anchored to "today" so the Usage
// screen always shows fresh-looking bars in the preview harness.
const usageSummaryFixture = (args: Record<string, unknown>) => {
  const days = Math.max(1, Math.min(3650, Number(args.days ?? 30)));
  const pattern = [3, 5, 0, 8, 6, 2, 9];
  let answers = 0;
  let input = 0;
  let output = 0;
  let ms = 0;
  const per_day: { date: string; answers: number; input: number; output: number }[] = [];
  const today = new Date();
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i);
    const date = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    const a = i % 6 === 4 ? 0 : pattern[i % pattern.length];
    per_day.push({ date, answers: a, input: a * 2400, output: a * 900 });
    answers += a;
    input += a * 2400;
    output += a * 900;
    ms += a * 9000;
  }
  return {
    days,
    answers,
    cancelled: 2,
    input_tokens: input,
    output_tokens: output,
    ms,
    tool_calls: Math.round(answers * 1.4),
    per_day,
    per_model: [
      { model: "Qwen3.5 9B", provider: "This computer", answers: Math.round(answers * 0.55), input: Math.round(input * 0.6), output: Math.round(output * 0.5), avg_ms: 11000, avg_tok_s: 28.4, avg_ttft_ms: 780 },
      { model: "kimi-k2.5", provider: "Kilo Code (free)", answers: Math.round(answers * 0.3), input: Math.round(input * 0.25), output: Math.round(output * 0.32), avg_ms: 6000, avg_tok_s: 62.5, avg_ttft_ms: 410 },
      { model: "gpt-5-mini", provider: "OpenRouter", answers: Math.round(answers * 0.15), input: Math.round(input * 0.15), output: Math.round(output * 0.18), avg_ms: 5200, avg_tok_s: 88.1, avg_ttft_ms: 320 },
    ],
  };
};

const fixtures: Record<string, Handler> = {
  config_load: () => ({ selected_models: ["qwen3.5-9b"] }),
  settings_theme: () => "dark",
  settings_set_theme: () => null,
  ui_scale_get: () => 1,
  ui_scale_set: () => null,
  system_probe: () => ({
    total_ram_gb: 32,
    available_ram_gb: 21,
    gpu_vendor: null,
    recommended_preset: "balanced",
    budget_gb: 14,
  }),
  system_health: () => ({
    app_version: "0.1.8",
    catalog_version: 5,
    engine: { loaded: true, active_model: "qwen3.5-9b", context_size: 32768, sidecar_binary_found: true },
    embedding: { ready: false, model_id: null, dimension: null, total_chunks: 184, embedded_chunks: 0 },
    rerank: { ready: false, model_id: null },
    library: { doc_count: 12 },
    config_ok: true,
  }),
  developer_get: () => ({
    enabled: false,
    flags: {},
    modules: {},
    flag_list: [
      { id: "request_log", label: "Request log", description: "Time every backend call.", default: false },
      { id: "no_motion", label: "Turn off animations", description: "Skip transitions everywhere.", default: false },
      { id: "compact", label: "Compact layout", description: "Tighter spacing.", default: false },
      { id: "answer_stats", label: "Answer statistics", description: "Tokens, time and speed under each answer.", default: false },
      { id: "plain_text", label: "Plain-text view", description: "Show the raw answer text.", default: false },
      { id: "dev_prompt", label: "Edit the system prompt", description: "Change the instructions the model starts with.", default: false },
    ],
    module_list: [
      { id: "memory", label: "Notes and file recall", description: "Chat looks in your notes and documents.", default: true },
      { id: "search", label: "Search screen", description: "The Library screen in the sidebar.", default: true },
      { id: "web", label: "Web in chat", description: "The model may search the web.", default: true },
      { id: "skills", label: "Skills", description: "Saved prompts you can reuse.", default: true },
      { id: "online", label: "Online providers", description: "Cloud models with your key.", default: true },
      { id: "opencode", label: "OpenCode link", description: "Hand work to OpenCode.", default: true },
      { id: "command_palette", label: "Command palette", description: "Ctrl+K quick actions.", default: true },
    ],
    modes: [
      { id: "standard", label: "Standard", description: "Everyday answers.", instruction: "" },
      { id: "strict", label: "Strict", description: "Careful, cites sources, says when unsure.", instruction: "Be careful and cite sources." },
      { id: "unrestricted", label: "Unrestricted", description: "Direct answers without lectures.", instruction: "Answer directly." },
      { id: "autonomous", label: "Autonomous", description: "Plans, gathers sources, works through the task, checks the result.", instruction: "Work autonomously." },
    ],
  }),
  developer_set: (args) => args.update ?? null,
  developer_reset: () => fixtures.developer_get({}),

  model_list: () => [MODEL_QWEN, MODEL_GEMMA, MODEL_VISION, MODEL_TINY, MODEL_EMBED, MODEL_RERANK],
  model_catalog_status: () => ({ version: 5, updated_at: ago(60 * 26), model_count: 41, source: "built-in", fetched_at: null, url: "" }),
  model_presets: () => [
    { id: "tiny", name: "Tiny", description: "Fastest answers, about 1 GB of memory.", models: ["qwen3.5-0.8b"], ram_gb: 1 },
    { id: "small", name: "Small", description: "Good balance for short chats.", models: ["qwen3.5-2b"], ram_gb: 4 },
    { id: "balanced", name: "Balanced", description: "The everyday pick.", models: ["qwen3.5-9b", "gemma-4-e4b"], ram_gb: 9.5 },
    { id: "advanced", name: "Advanced", description: "Longer, deeper answers.", models: ["qwen3.8-27b"], ram_gb: 20 },
  ],
  inference_status: () => ({ loaded: true, loading: false }),
  inference_load: () => "qwen3.5-9b",
  inference_unload: () => null,
  engine_info: () => ({ installed: true, build: "b12359", cpu: "x86_64 · AVX2", download_bytes: null, custom: false, path: "C:\\Users\\Omar\\.aiassistant\\engine\\llama-server.exe" }),

  chat_models: () => chatModelsFixture(),
  chat_prefs_get: () => ({ system_prompt: null, temperature: null, max_tokens: null, persona: null }),
  chat_prefs_set: () => null,
  usage_summary: (args) => usageSummaryFixture(args as Record<string, unknown>),
  chat_target_set: (args) => {
    // Flip the active model, so the picker's active row and the effort
    // ladder follow the choice like the real backend does.
    const req = (args?.req ?? args ?? {}) as Record<string, unknown>;
    const wanted = String(req.model ?? "");
    const base = chatModelsFixture();
    for (const m of base.models) m.active = !wanted || m.model === wanted || m.key === wanted;
    base.active_key = (base.models.find((m) => m.active === true)?.key as string) ?? base.active_key;
    if (wanted) base.local_loaded = !wanted.includes("/");
    return base;
  },
  chat_favorite_toggle: () => null,
  chat_starters: () => [
    { title: "Review this pull request", prompt: "Review this pull request for bugs and missing tests.", uses: 6 },
    { title: "Summarize sales.csv by quarter", prompt: "Summarize sales.csv by quarter.", uses: 4 },
    { title: "Plan my week", prompt: "Help me plan my week.", uses: 3 },
    { title: "Write an email", prompt: "Draft a polite email about a late payment.", uses: 2 },
  ],
  chat_thread_list: () => [
    { id: "t1", title: "Sales summary for Q3", created_at: ago(300), updated_at: ago(45), message_count: 8, folder: "C:\\Users\\Omar\\Documents\\finance" },
    { id: "t2", title: "Fix the tests", created_at: ago(500), updated_at: ago(120), message_count: 4, folder: "C:\\Users\\Omar\\Documents\\finance" },
    { id: "t3", title: "Landing page ideas", created_at: ago(60 * 26), updated_at: ago(60 * 20), message_count: 6, folder: "C:\\Users\\Omar\\Documents\\site" },
    { id: "t4", title: "Plan my week", created_at: ago(60 * 24 * 3), updated_at: ago(60 * 24 * 3), message_count: 2, folder: null },
    { id: "t5", title: "Email about the late invoice", created_at: ago(60 * 24 * 8), updated_at: ago(60 * 24 * 8), message_count: 3, folder: null },
  ],
  chat_thread_load: () => [],
  chat_thread_save: () => null,
  chat_thread_delete: () => null,
  chat_thread_export_save: () => null,
  chat_export_text_save: () => null,
  chat_cancel: () => null,
  chat_enhance_prompt: (args) => `${args.text} — rewritten with clear steps and a short goal statement.`,
  chat_suggest_next: () => ["Turn this into a checklist I can follow tomorrow.", "Draft an email to the client about the delay.", "What else changed in the invoice terms?"],
  chat_title: () => "Invoice terms and payment dates",
  chat_tool_decide: () => null,
  chat_attach_pick: () => null,
  chat_attach_dropped: () => [],
  summarize: () => ({ summary: "A short summary of the conversation.", title: "Summary" }),

  library_status: () => ({
    folders: [
      { path: "C:\\Users\\Omar\\Documents\\finance", added_at: ago(60 * 24 * 6), last_scan_at: ago(90), indexed: 9, skipped: 1, limited: false, documents: 9 },
      { path: "C:\\Users\\Omar\\Documents\\site", added_at: ago(60 * 24 * 2), last_scan_at: ago(200), indexed: 3, skipped: 0, limited: false, documents: 3 },
    ],
    documents: 12,
    total_bytes: 41_000_000,
    last_scan_at: ago(90),
    search_mode: "keyword",
    reason: "A meaning model makes search understand similar words. Download one below.",
    embedding_model: null,
    embedding_ready: false,
    embedded_chunks: 0,
    total_chunks: 184,
    choices: [
      { id: "embeddinggemma-300m", name: "EmbeddingGemma 300M", installed: true, size_bytes: 250_000_000 },
      { id: "nomic-embed-v2", name: "Nomic Embed v2", installed: false, size_bytes: 650_000_000 },
    ],
    storage_path: "C:\\Users\\Omar\\.aiassistant\\index\\library.db",
  }),
  library_folder_presets: () => [
    { id: "documents", label: "Documents", path: "C:\\Users\\Omar\\Documents", exists: true, added: false },
    { id: "desktop", label: "Desktop", path: "C:\\Users\\Omar\\Desktop", exists: true, added: false },
    { id: "downloads", label: "Downloads", path: "C:\\Users\\Omar\\Downloads", exists: true, added: false },
  ],
  indexer_stats: () => ({ doc_count: 12, total_bytes: 41_000_000, storage_path: "C:\\Users\\Omar\\.aiassistant\\index\\library.db" }),
  embedding_status: () => ({ engine: "not-running", model_id: null, dimension: null, ready: false, total_chunks: 184, embedded_chunks: 0 }),
  rerank_status: () => ({ model_id: null, ready: false }),
  list_index: () => [],
  search_index: () => [],

  // Memory timestamps cross the bridge in seconds, like the Rust backend.
  memory_list: () => [
    { id: "n1", path: "C:\\Users\\Omar\\.aiassistant\\memory\\team.md", title: "Team roster", size: 812, modified: sec(300), tags: ["work"], pinned: true },
    { id: "n2", path: "C:\\Users\\Omar\\.aiassistant\\memory\\invoice.md", title: "Invoice policy", size: 1240, modified: sec(60 * 30), tags: ["finance"], pinned: false },
    { id: "n3", path: "C:\\Users\\Omar\\.aiassistant\\memory\\standup-notes.md", title: "Standup notes", size: 940, modified: sec(60 * 24 * 34), tags: ["work"], pinned: false },
    { id: "n4", path: "C:\\Users\\Omar\\.aiassistant\\memory\\trip-plan.md", title: "Trip plan", size: 660, modified: sec(60 * 24 * 65), tags: [], pinned: false },
  ],
  memory_read: () => "# Team roster\n\nAna owns the design review. Ben handles releases.",
  memory_search: () => [],
  memory_create: () => ({ path: "C:\\Users\\Omar\\.aiassistant\\memory\\new-note.md" }),
  memory_update: () => null,
  memory_delete: () => null,
  memory_forget_all: () => {},
  memory_pinned_set: (args) => {
    const req = (args ?? {}) as Record<string, unknown>;
    return { id: "n2", path: String(req.path ?? ""), title: "Invoice policy", size: 1240, modified: sec(1), tags: ["finance"], pinned: req.pinned !== false };
  },
  memory_pending_list: () => [
    { file: "prefers-short-answers.md", title: "Prefers short answers", content: "The user asked twice for two-sentence answers instead of long explanations.", modified: sec(12) },
    { file: "launch-date.md", title: "Launch date is December 5", content: "The team's launch deadline is December 5; weekly reviews happen Fridays.", modified: sec(65) },
  ],
  memory_pending_accept: (args) => ({ id: "n5", path: `C:\\Users\\Omar\\.aiassistant\\memory\\${(args as Record<string, unknown>).file ?? "note"}.md`, title: "Accepted suggestion", size: 300, modified: ago(0), tags: [], pinned: false }),
  memory_pending_dismiss: () => true,

  skills_list: () => [
    { name: "review-pr", description: "Review a pull request for bugs and missing tests.", tags: ["code"], body: "Review the diff...", path: "review-pr.md" },
    { name: "standup", description: "Write a short standup update.", tags: ["work"], body: "Write the update...", path: "standup.md" },
  ],
  skills_sources: () => [],
  skills_discover: () => [],
  skills_suggestions: () => [],
  skills_create: () => null,
  skills_update: () => null,
  skills_delete: () => null,
  skills_undo: () => null,
  skills_read: () => "",
  skills_import: () => null,
  skills_remote_describe: () => "",
  skills_remote_detail: () => "",

  cloud_config_get: () => ({ online_allowed: false, cloud_enabled: false, providers: [], active_provider_id: null }),
  cloud_provider_presets: () => [],
  cloud_audit_list: () => [],

  voice_settings_get: () => ({
    enabled: false,
    model: "stt-whisper-tiny",
    local_only: true,
    cleanup: false,
    auto_send: true,
    read_aloud: false,
    voice_name: null,
    rate: 1,
    key_hints: {},
  }),
  voice_models: () => ({
    local: [
      { id: "stt-whisper-tiny", name: "Whisper tiny", params: "39 M", description: "Fast English transcription.", license: "MIT", size_bytes: 75_000_000, installed: true },
      { id: "stt-whisper-base", name: "Whisper base", params: "74 M", description: "Slightly better, still quick.", license: "MIT", size_bytes: 142_000_000, installed: false },
    ],
    online: [],
    engine: { installed: true, path: null, version: "1.10.30", download_bytes: null, available: true },
  }),

  share_status: () => ({ on: false, url: null, started_at: null, requests: 0, refused: 0, last_seen: null, cloudflared: { installed: false, path: null, version: null, download_bytes: 17_000_000, available: true } }),
  web_providers: () => [
    { id: "duckduckgo", label: "DuckDuckGo", needs_key: false, takes_key: true, supports: "search", note: "Works without a key.", key_set: false, is_default: true },
    { id: "wikipedia", label: "Wikipedia", needs_key: false, takes_key: false, supports: "search", note: "Works without a key.", key_set: false, is_default: false },
    { id: "jina", label: "Jina Reader", needs_key: false, takes_key: true, supports: "read", note: "Reads pages as text.", key_set: false, is_default: false },
  ],
  mcp_servers_list: () => [],
  mcp_presets: () => [],
  opencode_get: () => ({}),
  opencode_check: () => ({}),

  settings_context: () => 8192,
  settings_context_info: () => ({ value: 8192, min: 512, max: 32768, model_id: "qwen3.5-9b", model_name: "Qwen3.5 9B Instruct", loaded: 8192, active_id: "qwen3.5-9b" }),
  settings_model_memory: () => 60,
  settings_set_context: () => 8192,
  settings_set_model_memory: () => 60,

  update_check: () => ({ current_version: "0.2.0", latest_version: "0.2.1", update_available: true, release_url: "https://github.com/omarjson/LocalAide/releases", can_install: true, notes: null }),
  update_install: () => null,
  backup_config_get: () => ({ target_dir: "C:\\Users\\Omar\\Backups", include_api_keys: false, last_backup_at: ago(60 * 24 * 2) }),
  backup_list: () => [
    { file_name: "localaide-20261119.labak", created_at: ago(60 * 24 * 2), size_bytes: 84_000, app_version: "0.1.5" },
  ],
  tool_audits_list: () => [
    { id: 2, timestamp: ago(42), tool_name: "read_file", action: "read_file", path: "C:\\Users\\Omar\\.aiassistant\\workspace\\notes.txt", approved: true, status: "ok", details: null },
    { id: 1, timestamp: ago(160), tool_name: "write_file", action: "write_file", path: "C:\\Users\\Omar\\.aiassistant\\workspace\\summary.md", approved: true, status: "ok", details: null },
  ],
  logs_tail: () => "",
  hf_search: () => [],
  pick_folder: () => null,
  pick_document: () => null,
  files_open: () => null,
  files_reveal: () => null,
  files_open_with_apps: () => [],
  render_frame_put: () => "frame-1",
};

/** A canned streaming answer that exercises every chat event the UI renders. */
function streamAnswer(channel: { id: number } | undefined, req: Record<string, unknown>): void {
  if (!channel) return;
  const cb = callbacks.get(channel.id);
  if (!cb) return;
  const events: Array<Record<string, unknown>> = [
    { type: "status", payload: { label: "Checking your notes" } },
    {
      type: "tool_call",
      payload: { id: "call-1", name: "search_notes", title: "Searching your notes", detail: "invoice policy", group: "notes", arguments: "{}" },
    },
    { type: "tool_result", payload: { id: "call-1", ok: true, summary: "1 note matched", output: "Invoice policy: net 30 days." } },
    { type: "reasoning", payload: { text: "The user wants the payment window. The note says net 30 days, so the answer is 30 days after the invoice date." } },
    { type: "citation", payload: { index: 1, title: "Invoice policy", source: "C:\\Users\\Omar\\.aiassistant\\memory\\invoice.md", snippet: "Payment is due net 30 days after the invoice date.", score: 0.42 } },
    { type: "token", payload: { token: "" } },
  ];
  const answer =
    "Payment terms follow your **invoice policy**: invoices are due **net 30 days** after the invoice date.\n\nThat means for invoice `INV-1042` dated November 1, the due date is December 1.\n\n```text\nINV-1042 · issued 2026-11-01 · due 2026-12-01\n```";
  let i = 0;
  let index = 0;
  const push = (event: Record<string, unknown>) => {
    cb({ index: index++, message: event });
  };
  const tick = () => {
    if (i < events.length) {
      push(events[i++]);
      setTimeout(tick, 120);
      return;
    }
    const chunks = answer.match(/\s*\S+/g) ?? [];
    let c = 0;
    const tokenTick = () => {
      if (c < chunks.length) {
        push({ type: "token", payload: { token: chunks[c++] } });
        setTimeout(tokenTick, 24);
      } else {
        push({ type: "done", payload: { finish_reason: "stop", total_tokens: 180, prompt_tokens: 960 } });
      }
    };
    tokenTick();
  };
  setTimeout(tick, 150);
}

const callbacks = new Map<number, (payload: unknown) => void>();
let callbackId = 0;

export function installShim(): void {
  const internals = {
    transformCallback: (cb: (payload: unknown) => void, once = false): number => {
      const id = ++callbackId;
      callbacks.set(id, once
        ? (payload) => { callbacks.delete(id); cb(payload); }
        : cb);
      return id;
    },
    unregisterCallback: (id: number): void => {
      callbacks.delete(id);
    },
    convertFileSrc: (path: string): string => path,
    invoke: (cmd: string, args: Record<string, unknown> = {}): Promise<unknown> => {
      if (cmd === "plugin:event|listen") return Promise.resolve(++callbackId);
      if (cmd === "plugin:event|unlisten") return Promise.resolve(null);
      if (cmd === "plugin:window|is_maximized") return Promise.resolve(false);
      if (cmd.startsWith("plugin:window|")) return Promise.resolve(null);
      const handler = fixtures[cmd];
      if (handler) return Promise.resolve(handler(args));
      if (cmd === "chat_stream") {
        streamAnswer(args.channel as { id: number }, args.req as Record<string, unknown>);
        return Promise.resolve("job-1");
      }
      console.warn(`[preview shim] no fixture for "${cmd}", returning null`, args);
      return Promise.resolve(null);
    },
  };
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = internals;
  // The webview/window APIs read this metadata at import time.
  (window as unknown as { __TAURI_INTERNALS__: { metadata?: unknown } }).__TAURI_INTERNALS__.metadata = {
    currentWindow: { label: "main" },
    currentWebview: { label: "main", windowLabel: "main" },
    currentMonitor: null,
  };
}
