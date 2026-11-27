// The config file: one JSON document under ~/.aiassistant/config.json.
// Field names match the Rust AppConfig exactly, so a config written by the
// Tauri build loads here without a migration.

import fs from "node:fs";
import path from "node:path";
import { appDir, configPath } from "./paths.mjs";

// Mirrors the Rust AppConfig field for field, so a config written by the
// Tauri build loads here unchanged.
const DEFAULTS = {
  selected_models: [],
  cloud_llm_enabled: false,
  setup_complete: false,
  cloud_providers: [],
  online_allowed: null,
  active_cloud_provider: null,
  context_tokens: 8192,
  chat_system_prompt: null,
  chat_temperature: null,
  chat_max_tokens: null,
  chat_persona: null,
  backup: { target_dir: null, include_api_keys: false, last_backup_at: null },
  web_search: { provider: null, exa_api_key: null, firecrawl_api_key: null, firecrawl_base_url: null },
  theme: null,
  model_memory_percent: null,
  opencode: { base_url: null, password: null },
  developer: { enabled: false, flags: {}, modules: {} },
  active_cloud_model: null,
  embedding_model: null,
  voice: { enabled: false, model: "stt-whisper-tiny", local_only: true, cleanup: false, auto_send: true, read_aloud: false, voice_name: null, rate: 1.0, keys: {} },
  favorite_models: [],
  mcp_servers: [],
  permissions: [],
  hooks: { pre_tool: null, post_tool: null, timeout_secs: 0 },
  routing: { provider_priority: [], key_pools: {} },
  auto_approve_capabilities: {
    read_files: true, edit_files: false, safe_commands: false,
    all_commands: false, browser: false, external_tools: false,
  },
  use_compact_prompt: false,
  plan_auto_accept_secs: 0,
  terminal_approval_secs: 60,
  engine: { accel: "", gpu_layers: null, threads: null, flash_attn: null, mmap: null, mlock: false, cache_type_k: null, cache_type_v: null, parallel: null, batch_size: null, rope_freq_scale: null, extra_args: "" },
};

/** Limits the settings commands clamp against, from the Rust config module. */
export const MIN_CONTEXT_TOKENS = 512;
export const MAX_CONTEXT_TOKENS = 1_048_576;
export const MAX_APPROVAL_TIMER_SECS = 600;
export const DEFAULT_MODEL_MEMORY_PERCENT = 70;
export const MIN_MODEL_MEMORY_PERCENT = 20;
export const MAX_MODEL_MEMORY_PERCENT = 90;

/** The memory share for models, kept inside the allowed range. */
export function memoryPercentForModels(cfg) {
  return Math.min(
    MAX_MODEL_MEMORY_PERCENT,
    Math.max(MIN_MODEL_MEMORY_PERCENT, cfg.model_memory_percent ?? DEFAULT_MODEL_MEMORY_PERCENT),
  );
}

/** Whether online providers may be used at all. */
export const onlineAllowed = (cfg) =>
  cfg.online_allowed === null || cfg.online_allowed === undefined
    ? cfg.cloud_providers.length > 0
    : cfg.online_allowed === true;

/** Deep merge, so a config from an older version keeps every new default. */
function merge(base, override) {
  if (Array.isArray(base) || Array.isArray(override)) return override ?? base;
  if (typeof base !== "object" || base === null) return override ?? base;
  if (typeof override !== "object" || override === null) return base;
  const out = { ...base };
  for (const [key, value] of Object.entries(override)) {
    out[key] = key in base ? merge(base[key], value) : value;
  }
  return out;
}

let cache = null;

export function loadConfig() {
  if (cache) return cache;
  try {
    const raw = fs.readFileSync(configPath(), "utf8");
    cache = merge(structuredClone(DEFAULTS), JSON.parse(raw));
  } catch {
    cache = structuredClone(DEFAULTS);
  }
  return cache;
}

/** Applies a change and writes the file atomically. */
export function updateConfig(mutator) {
  const cfg = loadConfig();
  const result = mutator(cfg);
  const next = result ?? cfg;
  cache = next;
  writeConfig(next);
  return next;
}

export function writeConfig(cfg) {
  const file = configPath();
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(cfg, null, 2));
  fs.renameSync(tmp, file);
  // The config holds API keys: owner-only on POSIX, hidden elsewhere.
  try {
    fs.chmodSync(file, 0o600);
  } catch {
    /* Windows has no mode bits */
  }
}

/** Reloads from disk, for a restore that replaced the file. */
export function invalidateConfig() {
  cache = null;
}

/** The app directory exists before anything reads it. */
export function ensureAppDir() {
  fs.mkdirSync(appDir(), { recursive: true });
}

