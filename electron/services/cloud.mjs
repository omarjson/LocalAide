// Online providers: opt-in OpenAI-compatible endpoints. Keys stay in the
// local config file and travel only to their own provider. Ported from
// commands/cloud.rs and services/cloud_audit.rs.

import crypto from "node:crypto";
import { AppError } from "../core/errors.mjs";
import { loadConfig, updateConfig, onlineAllowed } from "../core/config.mjs";
import { run, get, all } from "../core/db.mjs";

const nowSecs = () => Math.floor(Date.now() / 1000);

/** True when the URL's host is this machine (parsed, not string-matched). */
function isLocalHost(url) {
  try {
    const parsed = new URL(url);
    const name = parsed.hostname.toLowerCase();
    return name === "localhost" || /^127\.\d+\.\d+\.\d+$/.test(name) || name === "[::1]" || name === "::1";
  } catch {
    return false;
  }
}

/** HTTPS unless loopback, no credentials in the URL. */
export function validateBaseUrl(url) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    throw AppError.invalidInput("The base URL is not a valid URL. Use https://provider.example/v1.");
  }
  if (parsed.username || parsed.password) {
    throw AppError.invalidInput("Do not put a username or password in the base URL.");
  }
  if (parsed.protocol === "https:") return;
  if (parsed.protocol === "http:") {
    if (isLocalHost(url)) return;
    throw AppError.invalidInput(
      "Plain HTTP is only allowed for addresses on this machine (127.0.0.1 or localhost). Use HTTPS for any remote provider.",
    );
  }
  throw AppError.invalidInput("The base URL must start with https:// (or http:// for a proxy on this machine).");
}

/** The card the Settings screen shows, with the key reduced to a hint. */
function providerDto(provider) {
  const key = String(provider.api_key ?? "");
  return {
    id: provider.id,
    label: provider.label,
    base_url: provider.base_url,
    model: provider.model ?? "",
    api_key_hint: key ? `${key.slice(0, 3)}…${key.slice(-4)}` : "",
    is_active: loadConfig().active_cloud_provider === provider.id,
    enabled: provider.enabled !== false,
    model_count: (provider.models ?? []).length,
    sample_models: (provider.models ?? []).slice(0, 5).map((m) => m.id),
    check: provider.check ?? null,
  };
}

export function cloudConfigStatus() {
  const cfg = loadConfig();
  return {
    online_allowed: onlineAllowed(cfg),
    cloud_enabled: cfg.cloud_llm_enabled === true && onlineAllowed(cfg),
    providers: cfg.cloud_providers.map(providerDto),
    active_provider_id: cfg.active_cloud_provider ?? null,
  };
}

/** The model entries a provider holds, plus its default and chosen model. */
export function providerModels(provider, chosen = null) {
  const list = [...(provider.models ?? [])];
  for (const extra of [provider.model, chosen]) {
    const id = String(extra ?? "").trim();
    if (id && !list.some((m) => m.id === id)) {
      list.unshift({ id, name: null, owned_by: null, context_length: null, free: false, reasoning: null, vision: null, tools: null });
    }
  }
  return list;
}

export function saveCloudProvider(req) {
  const label = String(req.label ?? "").trim();
  const baseUrl = String(req.base_url ?? "").trim().replace(/\/+$/, "");
  const model = String(req.model ?? "").trim();
  if (!label) throw AppError.invalidInput("Give the provider a name.");
  validateBaseUrl(baseUrl);
  if (!baseUrl) throw AppError.invalidInput("Enter the provider address first.");
  let savedId = null;
  updateConfig((cfg) => {
    const existing = req.id ? cfg.cloud_providers.find((p) => p.id === req.id) : undefined;
    // A key stays with its origin: adding a key from the same form is fine,
    // but a key is never copied to a provider at another address.
    const key = req.api_key === undefined ? (existing?.api_key ?? "") : String(req.api_key ?? "").trim();
    if (existing) {
      existing.label = label;
      existing.base_url = baseUrl;
      existing.model = model;
      if (req.api_key !== undefined) existing.api_key = key;
      savedId = existing.id;
      return;
    }
    // Reject a duplicate: same address + model, or same address + label.
    const dupBase = baseUrl.toLowerCase();
    if (
      cfg.cloud_providers.some(
        (p) =>
          p.base_url.toLowerCase().replace(/\/+$/, "") === dupBase &&
          ((p.model ?? "") === model && model !== "") ||
          (p.label.trim().toLowerCase() === label.toLowerCase() && (p.base_url.toLowerCase().replace(/\/+$/, "") === dupBase)),
      )
    ) {
      throw AppError.invalidInput("This provider is saved already (same address).");
    }
    const provider = {
      id: `cp-${crypto.randomUUID().slice(0, 8)}`,
      label,
      base_url: baseUrl,
      model,
      api_key: key,
      enabled: true,
      models: [],
      check: null,
    };
    cfg.cloud_providers.push(provider);
    savedId = provider.id;
  });
  return cloudConfigStatus();
}

export function deleteCloudProvider(providerId) {
  updateConfig((cfg) => {
    cfg.cloud_providers = cfg.cloud_providers.filter((p) => p.id !== providerId);
    if (cfg.active_cloud_provider === providerId) {
      cfg.active_cloud_provider = null;
      cfg.cloud_llm_enabled = false;
      cfg.active_cloud_model = null;
    }
    cfg.favorite_models = cfg.favorite_models.filter((k) => !k.startsWith(`${providerId}:`));
  });
  return cloudConfigStatus();
}

export function setCloudMode(req) {
  updateConfig((cfg) => {
    cfg.cloud_llm_enabled = req.enabled === true;
    if (req.enabled === true && req.provider_id) {
      const provider = cfg.cloud_providers.find((p) => p.id === req.provider_id);
      if (!provider) throw AppError.invalidInput("That provider no longer exists.");
      cfg.active_cloud_provider = provider.id;
      if (provider.model) cfg.active_cloud_model = provider.model;
    }
    if (req.enabled === false) {
      cfg.active_cloud_provider = null;
      cfg.active_cloud_model = null;
    }
  });
  return cloudConfigStatus();
}

export function setOnlineAllowed(enabled) {
  updateConfig((cfg) => {
    cfg.online_allowed = enabled === true;
  });
  return cloudConfigStatus();
}

export function setProviderEnabled(providerId, enabled) {
  updateConfig((cfg) => {
    const provider = cfg.cloud_providers.find((p) => p.id === providerId);
    if (!provider) throw AppError.invalidInput("That provider no longer exists.");
    provider.enabled = enabled === true;
  });
  return cloudConfigStatus();
}

/** Ask a provider which models it serves. Only on an explicit user action. */
export async function listProviderModels(req) {
  const baseUrl = String(req.base_url ?? "").trim().replace(/\/+$/, "");
  if (!baseUrl) throw AppError.invalidInput("Enter the provider address first.");
  validateBaseUrl(baseUrl);
  const cfg = loadConfig();
  const typed = String(req.api_key ?? "").trim();
  let key = typed || null;
  if (!key && req.provider_id) {
    const saved = cfg.cloud_providers.find((p) => p.id === req.provider_id);
    // A saved key is only reused while it still points at the same site.
    if (saved && saved.base_url.replace(/\/+$/, "").toLowerCase() === baseUrl.toLowerCase()) {
      key = saved.api_key || null;
    }
  }
  let response;
  try {
    response = await fetch(`${baseUrl}/models`, {
      headers: {
        Accept: "application/json",
        ...(key ? { Authorization: `Bearer ${key}` } : {}),
      },
      signal: AbortSignal.timeout(20_000),
    });
  } catch (e) {
    throw new AppError("PROVIDER_UNREACHABLE", "Could not reach the provider. Check the address.", { detail: String(e), retryable: true });
  }
  if (!response.ok) {
    throw new AppError(
      response.status === 401 || response.status === 403 ? "PROVIDER_AUTH" : "PROVIDER_ERROR",
      response.status === 401 || response.status === 403
        ? "The provider did not accept the key."
        : `The provider answered with an error (${response.status}).`,
      { retryable: response.status >= 500 || response.status === 429 },
    );
  }
  const body = await response.json().catch(() => null);
  const models = Array.isArray(body?.data) ? body.data : Array.isArray(body) ? body : [];
  const mapped = models.map((m) => ({
    id: String(m.id ?? m.name ?? ""),
    name: m.name ?? null,
    owned_by: m.owned_by ?? m.owned_by ?? null,
    context_length: m.context_length ?? m.max_context_length ?? null,
    free: /:free$|(^|\/)free(-|\/|$)/.test(String(m.id ?? "")) || m.pricing?.prompt === "0",
    reasoning: null,
    vision: null,
    tools: null,
  }));
  return mapped;
}

/** Save the fetched model list onto the provider and record the check. */
export async function refreshProviderModels(providerId) {
  const cfg = loadConfig();
  const provider = cfg.cloud_providers.find((p) => p.id === providerId);
  if (!provider) throw AppError.invalidInput("That provider no longer exists.");
  const models = await listProviderModels({ base_url: provider.base_url, api_key: null, provider_id: providerId });
  updateConfig((c) => {
    const saved = c.cloud_providers.find((p) => p.id === providerId);
    if (!saved) return;
    saved.models = models;
    saved.check = { ok: true, at: nowSecs(), model_count: models.length, error: null };
  });
  auditCloud(provider.id, provider.base_url, "", "load models", `${models.length} models`, true);
  return models;
}

/** Test connection: ask a saved provider for its models with its saved key. */
export async function testProvider(providerId) {
  const cfg = loadConfig();
  const provider = cfg.cloud_providers.find((p) => p.id === providerId);
  if (!provider) throw AppError.invalidInput("That provider no longer exists.");
  try {
    const models = await listProviderModels({ base_url: provider.base_url, api_key: null, provider_id: providerId });
    updateConfig((c) => {
      const saved = c.cloud_providers.find((p) => p.id === providerId);
      if (saved) saved.check = { ok: true, at: nowSecs(), model_count: models.length, error: null };
    });
    auditCloud(provider.id, provider.base_url, "", "test", `${models.length} models`, true);
  } catch (e) {
    updateConfig((c) => {
      const saved = c.cloud_providers.find((p) => p.id === providerId);
      if (saved) saved.check = { ok: false, at: nowSecs(), model_count: 0, error: e.message ?? String(e) };
    });
    auditCloud(provider.id, provider.base_url, "", "test", String(e?.message ?? e), false);
    throw e;
  }
  return cloudConfigStatus();
}

// ---- Presets (the starting points the Settings form can prefill) ------

const preset = (id, label, base_url, model, key_url, note, needs_key, local, free) => ({
  id,
  label,
  base_url,
  model,
  key_url,
  note,
  needs_key,
  local,
  free,
  public_key: "",
});

export const PROVIDER_PRESETS = [
  preset("kilo", "Kilo Code (free, no key)", "https://api.kilo.ai/api/openrouter", "kilo-auto/free", "https://kilo.ai", "Kilo's gateway serves free models without a key or sign-up. \"kilo-auto/free\" picks the best free model for you; press Load models to choose one yourself. Free models change over time.", false, false, true),
  { public_key: "public", ...preset("opencode-zen", "OpenCode Zen (free models)", "https://opencode.ai/zen/v1", "", "https://opencode.ai/zen", "OpenCode's model gateway. Its free models (names ending in -free, and big-pickle) work with the shared key \"public\", which is filled in for you. Press Load models and pick one. Paste your own Zen key to use the paid models.", false, false, true) },
  preset("pollinations", "Pollinations (free, no key)", "https://text.pollinations.ai/v1", "openai", "https://enter.pollinations.ai", "Works without a key or sign-up. Free use is limited to one request about every 45 seconds per computer, and answers may be filtered. Add a Pollinations key for more models and no wait.", false, false, true),
  preset("9router", "9Router (on this computer)", "http://localhost:20128/v1", "", "https://github.com/decolua/9router", "A free router you run yourself. It switches between free providers when one runs out. Start it, then press Load models. Paste a key only if you turned keys on in its dashboard.", false, true, true),
  preset("omniroute", "OmniRoute (on this computer)", "http://localhost:20128/v1", "", "https://github.com/diegosouzapw/OmniRoute", "A free router you run yourself, with automatic fallback across many providers. Start it, then press Load models. Paste a key only if you set one in OmniRoute.", false, true, true),
  preset("ollama-local", "Ollama (on this computer)", "http://127.0.0.1:11434/v1", "", "", "Your own Ollama server. No key needed. Press Load models to see what you have pulled.", false, true, true),
  preset("lmstudio-local", "LM Studio (on this computer)", "http://127.0.0.1:1234/v1", "", "", "LM Studio's local server. Turn it on in LM Studio, then press Load models.", false, true, true),
  preset("llamacpp-local", "llama.cpp server (on this computer)", "http://127.0.0.1:8080/v1", "", "https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md", "Run llama-server with a GGUF model, then press Load models. No key needed.", false, true, true),
  preset("vllm-local", "vLLM (on this computer)", "http://127.0.0.1:8000/v1", "", "https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html", "vLLM's OpenAI-compatible server. Start it with `vllm serve`, then press Load models.", false, true, true),
  preset("mlx-local", "MLX (on this computer)", "http://127.0.0.1:8080/v1", "", "https://github.com/ml-explore/mlx-lm", "Apple's MLX server (`mlx_lm.server`), for Macs with Apple silicon. Press Load models once it is running.", false, true, true),
  preset("localai-local", "LocalAI (on this computer)", "http://127.0.0.1:8080/v1", "", "https://localai.io/basics/getting_started/", "Your own LocalAI server. No key needed. Press Load models to see what it serves.", false, true, true),
  preset("litellm-local", "LiteLLM proxy (on this computer)", "http://127.0.0.1:4000/v1", "", "https://docs.litellm.ai/docs/proxy/quick_start", "One LiteLLM proxy in front of every provider you have. Start it with `litellm --config`, then press Load models. Paste your proxy master key if you set one.", false, true, true),
  preset("textgen-local", "text-generation-webui (on this computer)", "http://127.0.0.1:5000/v1", "", "https://github.com/oobabooga/text-generation-webui", "Start text-generation-webui with the `--api` flag, then press Load models.", false, true, true),
  preset("jan-local", "Jan (on this computer)", "http://127.0.0.1:1337/v1", "", "https://jan.ai", "Jan's local API server. Turn it on in Jan's settings, then press Load models.", false, true, true),
  preset("koboldcpp-local", "KoboldCpp (on this computer)", "http://127.0.0.1:5001/v1", "", "https://github.com/LostRuins/koboldcpp", "KoboldCpp's OpenAI-compatible endpoint. Launch it with a GGUF model, then press Load models.", false, true, true),
  preset("gpt4all-local", "GPT4All (on this computer)", "http://127.0.0.1:4891/v1", "", "https://docs.gpt4all.io/gpt4all_desktop/settings.html", "GPT4All's local API server, enabled in its settings. Press Load models once it runs.", false, true, true),
  preset("tabby-local", "Tabby (on this computer)", "http://127.0.0.1:8080/v1", "", "https://tabby.tabbyml.com/docs/models/", "Tabby's local server, mostly for code models. Press Load models to see what it serves.", false, true, true),
  preset("sglang-local", "SGLang (on this computer)", "http://127.0.0.1:30000/v1", "", "https://docs.sglang.ai/backend/openai_api_completions.html", "SGLang's OpenAI-compatible server, fast for long prompts and batch work. Start it with `python -m sglang.launch_server`, then press Load models.", false, true, true),
  preset("aphrodite-local", "Aphrodite (on this computer)", "http://127.0.0.1:2242/v1", "", "https://github.com/PygmalionAI/aphrodite-engine", "Aphrodite Engine's OpenAI-compatible server, built for roleplay and long context. Start it, then press Load models.", false, true, true),
  preset("docker-models-local", "Docker Model Runner (on this computer)", "http://127.0.0.1:12434/engines/v1", "", "https://docs.docker.com/ai/model-runner/", "Docker Desktop's built-in model server. Turn on Model Runner, pull a model, then press Load models.", false, true, true),
  preset("xinference-local", "Xinference (on this computer)", "http://127.0.0.1:9997/v1", "", "https://inference.readthedocs.io/en/latest/getting_started/using_xinference.html", "Xinference's local server, which can host chat, embedding and rerank models. Start it, then press Load models.", false, true, true),
  preset("gemini", "Google Gemini", "https://generativelanguage.googleapis.com/v1beta/openai", "gemini-flash-latest", "https://aistudio.google.com/apikey", "Free key from Google AI Studio, with daily limits. On the free tier Google may use your prompts to improve its products.", true, false, true),
  preset("openrouter", "OpenRouter", "https://openrouter.ai/api/v1", "", "https://openrouter.ai/keys", "One key for hundreds of models. Models marked Free cost nothing, with a daily request limit.", true, false, true),
  preset("groq", "Groq", "https://api.groq.com/openai/v1", "llama-3.3-70b-versatile", "https://console.groq.com/keys", "Very fast. Free key with strict rate limits.", true, false, true),
  preset("cerebras", "Cerebras", "https://api.cerebras.ai/v1", "", "https://cloud.cerebras.ai", "Free key with a daily token cap.", true, false, true),
  preset("huggingface", "Hugging Face Inference", "https://router.huggingface.co/v1", "", "https://huggingface.co/settings/tokens", "Open models from many hosts with one Hugging Face token. Free accounts get small monthly credits.", true, false, true),
  preset("nvidia", "NVIDIA NIM", "https://integrate.api.nvidia.com/v1", "", "https://build.nvidia.com", "Free key, about 40 requests per minute.", true, false, true),
  preset("mistral", "Mistral", "https://api.mistral.ai/v1", "mistral-small-latest", "https://console.mistral.ai", "Free Experiment tier. Mistral may train on free-tier prompts.", true, false, true),
  preset("llm7", "LLM7", "https://api.llm7.io/v1", "", "https://llm7.io", "Community router. Get a free token, no card needed.", true, false, true),
  preset("chutes", "Chutes", "https://llm.chutes.ai/v1", "", "https://chutes.ai", "Open models with per-model quotas.", true, false, true),
  preset("openai", "OpenAI", "https://api.openai.com/v1", "", "https://platform.openai.com/api-keys", "Paid by use.", true, false, false),
  preset("deepseek", "DeepSeek", "https://api.deepseek.com/v1", "deepseek-chat", "https://platform.deepseek.com/api_keys", "Paid by use, low prices.", true, false, false),
  preset("together", "Together AI", "https://api.together.xyz/v1", "", "https://api.together.ai/settings/api-keys", "Paid by use, many open models.", true, false, false),
];

// ---- The audit: what left this computer --------------------------------

/** One row in the cloud audit, with the host only (no path or key). */
function auditRow(providerId, baseUrl, kind, summary, bytes, ok) {
  let host = "";
  try {
    host = new URL(baseUrl).host;
  } catch {
    host = baseUrl;
  }
  run(
    `INSERT INTO cloud_audit (timestamp, host, base_url, provider_id, kind, summary, bytes, ok, detail)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    Date.now(), host, baseUrl, providerId ?? "", kind ?? "", String(summary ?? ""), Math.max(0, Math.round(bytes) || 0), ok ? 1 : 0,
  );
}

export const auditCloud = auditRow;

const rowToCall = (row) => ({
  id: row.id,
  timestamp: row.timestamp,
  provider: row.provider_id,
  host: row.host,
  model: "",
  kind: row.kind,
  preview: row.summary,
  bytes: row.bytes,
  status: row.ok === 1 ? "ok" : "failed",
  detail: row.detail,
});

export function auditList(limit = 200) {
  const keep = Math.max(1, Math.min(1000, Math.round(Number(limit) || 200)));
  return all(`SELECT * FROM cloud_audit ORDER BY timestamp DESC, id DESC LIMIT ?`, keep).map(rowToCall);
}

export function auditClear() {
  const info = run(`DELETE FROM cloud_audit`);
  return info.changes;
}
