// The command table: every frontend command the Electron backend answers.
// One ipcMain channel, a name→handler table, AppError out. Unimplemented
// commands throw NOT_IMPLEMENTED so the UI shows one honest error instead
// of a blank screen.

import { ipcMain, dialog, shell, BrowserWindow } from "electron";
import fs from "node:fs";
import path from "node:path";
import { guard, AppError } from "./core/errors.mjs";
import { channel } from "./core/channels.mjs";
import {
  loadConfig, updateConfig, onlineAllowed, memoryPercentForModels,
  MIN_CONTEXT_TOKENS, MAX_CONTEXT_TOKENS, MAX_APPROVAL_TIMER_SECS,
  MIN_MODEL_MEMORY_PERCENT, MAX_MODEL_MEMORY_PERCENT,
} from "./core/config.mjs";
import { log, tail } from "./core/log.mjs";
import { all, get, run } from "./core/db.mjs";
import { appDir, workspaceDir, backupDir, dbPath, customModelsPath, modelsDir, catalogCachePath } from "./core/paths.mjs";
import * as downloads from "./services/download.mjs";

import { startChat, cancelJob, steerJob, decideTool, summarize as summarizeText } from "./services/chat.mjs";
import { loadEngine, unloadEngine, engineStatus, engineInfo, validatedEngineConfig, setEngineConfig } from "./services/engine.mjs";
import { ensureThread } from "./services/threads.mjs";
import { listThreads, loadThread, saveThread, deleteThread, exportThread, fileStem } from "./services/threads.mjs";
import { summary as usageSummary, ledgerAdd, ledgerList } from "./services/usage.mjs";
import { runTool } from "./services/tools.mjs";
import * as memory from "./services/memory.mjs";
import * as library from "./services/library.mjs";
import * as websearch from "./services/websearch.mjs";
import * as models from "./services/models.mjs";
import * as cloud from "./services/cloud.mjs";
import * as assist from "./services/assist.mjs";
import * as attach from "./services/attach.mjs";
import * as skills from "./services/skills.mjs";
import * as goal from "./services/goal.mjs";
import * as doctor from "./services/doctor.mjs";
import * as probe from "./services/probe.mjs";
import * as updates from "./services/updates.mjs";
import * as automations from "./services/automations.mjs";
import * as developer from "./services/developer.mjs";
import * as indexer from "./services/indexer.mjs";
import * as mcp from "./services/mcp.mjs";
import * as opencode from "./services/opencode.mjs";
import * as embedding from "./services/embedding.mjs";
import * as voice from "./services/voice.mjs";
import { runSelfTest } from "./services/selftest.mjs";
import * as share from "./services/share.mjs";
import * as backup from "./services/backup.mjs";
import * as rerank from "./services/rerank.mjs";
import { capabilities } from "./services/models.mjs";
import { complete, buildBody } from "./services/stream.mjs";

const num = (value) => (Number.isFinite(Number(value)) ? Number(value) : null);
/** The running library index job, so Cancel can reach it. */
let indexJob = null;

const notImplemented = (cmd) => () => {
  throw new AppError("NOT_IMPLEMENTED", `${cmd} is not available in this build yet.`);
};

/** Channel args arrive as { __channelId } objects, not numbers. */
const ch = (args, key = "channel") => {
  const raw = args?.[key];
  const id = raw?.__channelId ?? raw;
  if (id === undefined || id === null) {
    throw AppError.invalidInput("This action needs a progress channel.");
  }
  return channel(id);
};

// ---- Small builders the table shares -----------------------------------

function configDto() {
  const cfg = loadConfig();
  return {
    selected_models: cfg.selected_models ?? [],
    setup_complete: cfg.setup_complete === true,
    has_online_providers: (cfg.cloud_providers ?? []).length > 0,
  };
}

/** remote_caps: the provider's own word first, then the name. */
function remoteCaps(m) {
  const caps = capabilities(String(m.id ?? ""), false);
  if (typeof m.reasoning === "boolean") {
    caps.reasoning = m.reasoning;
    caps.efforts = m.reasoning ? capabilities(String(m.id ?? ""), false).efforts : [];
  }
  if (typeof m.vision === "boolean") caps.vision = m.vision;
  if (typeof m.tools === "boolean") caps.tools = m.tools;
  if (m.context_length) caps.context = m.context_length;
  return caps;
}

/** The chat's model picker: local models plus every enabled provider's. */
function buildChatChoices() {
  const cfg = loadConfig();
  const favorites = new Set(cfg.favorite_models ?? []);
  const status = engineStatus();
  const online = onlineAllowed(cfg);

  const cloudActive =
    cfg.cloud_llm_enabled === true && online && cfg.active_cloud_provider
      ? (cfg.cloud_providers ?? []).find((p) => p.id === cfg.active_cloud_provider && p.enabled !== false)
      : null;
  const chosenRemoteModel = cloudActive
    ? (cfg.active_cloud_model?.trim() || cloudActive.model || "")
    : null;
  const activeKey = cloudActive
    ? `${cloudActive.id}:${chosenRemoteModel || cloudActive.model || ""}`
    : (cfg.selected_models ?? [])[0]
      ? `local:${(cfg.selected_models ?? [])[0]}`
      : null;

  const modelsOut = [];
  const seen = new Set();
  for (const entry of models.allEntries()) {
    if (entry.kind !== "chat" && entry.kind !== "code") continue;
    const installed = models.isInstalled(entry.id);
    if (!installed) continue;
    const key = `local:${entry.id}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const caps = capabilities(`${entry.id} ${entry.name}`, true);
    caps.vision = (entry.companions ?? []).some((c) => String(c.dest ?? "").includes("mmproj")) || caps.vision;
    caps.tools = true;
    caps.context = models.effectiveContext(entry.id, cfg.context_tokens);
    modelsOut.push({
      active: activeKey === key,
      favorite: favorites.has(key),
      key,
      provider_id: null,
      provider_label: "On this computer",
      model: entry.id,
      name: entry.name,
      local: true,
      reasoning: caps.reasoning,
      vision: caps.vision,
      tools: caps.tools,
      context: caps.context,
      efforts: caps.efforts,
      free: true,
      size_bytes: entry.file_size,
    });
  }

  const groups = [];
  for (const p of cfg.cloud_providers ?? []) {
    let localRouter = false;
    try {
      const parsed = new URL(p.base_url);
      localRouter = ["localhost", "127.0.0.1", "::1"].includes(parsed.hostname) || /^127\./.test(parsed.hostname);
    } catch {
      /* a bad address still shows as a group */
    }
    groups.push({
      provider_id: p.id,
      label: p.label,
      enabled: p.enabled !== false,
      local_router: localRouter,
      needs_refresh: (p.models ?? []).length === 0,
    });
    if (p.enabled === false || !online) continue;
    const chosen = cfg.active_cloud_provider === p.id ? cfg.active_cloud_model : null;
    for (const m of cloud.providerModels(p, chosen)) {
      const key = `${p.id}:${m.id}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const caps = remoteCaps(m);
      modelsOut.push({
        active: activeKey === key,
        favorite: favorites.has(key),
        key,
        provider_id: p.id,
        provider_label: p.label,
        name: m.name || m.id,
        model: m.id,
        local: localRouter,
        reasoning: caps.reasoning,
        vision: caps.vision,
        tools: caps.tools,
        context: caps.context,
        efforts: caps.efforts,
        free: m.free === true || localRouter,
        size_bytes: null,
      });
    }
  }

  return {
    models: modelsOut,
    groups,
    active_key: activeKey,
    local_loaded: !!status.loaded,
    online_allowed: online,
    budget_note: null,
  };
}

/** ContextInfo: the saved value, the slider max, and what is loaded. */
function contextInfo() {
  const cfg = loadConfig();
  const status = engineStatus();
  const FALLBACK_MAX = 131_072;
  const activeId = (cfg.selected_models ?? [])[0] ?? null;
  const hintFor = (id) => {
    const entry = id ? models.getEntry(id) : null;
    return entry ? models.effectiveContext(id, 1_048_576) : null;
  };
  let sourceId = activeId;
  let sourceHint = hintFor(activeId);
  if (!sourceHint) {
    for (const entry of models.allEntries()) {
      if (entry.kind !== "chat" && entry.kind !== "code") continue;
      if (!models.isInstalled(entry.id)) continue;
      const hint = models.guessContext(entry.id);
      if (hint && (!sourceHint || hint > sourceHint)) {
        sourceHint = hint;
        sourceId = entry.id;
      }
    }
  }
  const max = Math.min(MAX_CONTEXT_TOKENS, Math.max(MIN_CONTEXT_TOKENS * 4, sourceHint ?? FALLBACK_MAX));
  return {
    value: cfg.context_tokens,
    min: MIN_CONTEXT_TOKENS,
    max,
    model_id: sourceId,
    model_name: sourceId ? models.getEntry(sourceId)?.name ?? null : null,
    loaded: status.loaded ? status.context : null,
    active_id: status.loaded ? activeId : null,
  };
}

/** The aggregate health snapshot. */
function systemHealth() {
  const status = engineStatus();
  const stats = library.indexerStats();
  const logDir = path.join(appDir(), "logs");
  const sumDir = (dir) => {
    let total = 0;
    try {
      for (const name of fs.readdirSync(dir)) {
        const file = path.join(dir, name);
        if (fs.statSync(file).isFile()) total += fs.statSync(file).size;
      }
    } catch {
      /* missing dir reads as empty */
    }
    return total;
  };
  const firstHealth = systemHealth.startedAt ?? (systemHealth.startedAt = Date.now());
  return {
    app_version: updates.appVersion(),
    catalog_version: doctor.catalogVersion(),
    engine: {
      loaded: status.loaded,
      active_model: status.model,
      context_size: status.context ?? 0,
      sidecar_binary_found: engineInfo().installed,
    },
    embedding: { ready: false, model_id: loadConfig().embedding_model ?? null, dimension: null, total_chunks: 0, embedded_chunks: 0 },
    rerank: { ready: false, model_id: null },
    library: { doc_count: stats.doc_count },
    performance: {
      uptime_secs: Math.floor((Date.now() - firstHealth) / 1000),
      rss_bytes: process.memoryUsage().rss,
      logs_bytes: sumDir(logDir),
      library_db_bytes: fs.existsSync(dbPath()) ? fs.statSync(dbPath()).size : 0,
    },
    config_ok: true,
  };
}

// ---- The command table ---------------------------------------------------

const commands = {
  // Boot and config
  config_load: guard(async () => configDto()),
  config_setup_complete: guard(async () => {
    updateConfig((cfg) => {
      cfg.setup_complete = true;
    });
  }),
  system_probe: guard(async () => probe.systemProbe()),
  system_health: guard(async () => systemHealth()),
  update_check: guard(async () => updates.updateCheck()),
  update_install: guard(async (args) => updates.updateInstall(args?.channel)),
  ui_scale_get: guard(async () => assist.uiScaleGet()),
  ui_scale_set: guard(async (args) => assist.uiScaleSet(args.scale)),

  // Settings
  settings_context: guard(async () => loadConfig().context_tokens),
  settings_set_context: guard(async (args) => {
    const clamped = Math.min(MAX_CONTEXT_TOKENS, Math.max(MIN_CONTEXT_TOKENS, Math.round(Number(args.tokens) || MIN_CONTEXT_TOKENS)));
    updateConfig((cfg) => {
      cfg.context_tokens = clamped;
    });
    return clamped;
  }),
  settings_context_info: guard(async () => contextInfo()),
  settings_compact_prompt: guard(async () => loadConfig().use_compact_prompt === true),
  settings_set_compact_prompt: guard(async (args) => {
    updateConfig((cfg) => {
      cfg.use_compact_prompt = args.on === true;
    });
    return loadConfig().use_compact_prompt === true;
  }),
  settings_engine_flags: guard(async () => loadConfig().engine),
  settings_set_engine_flags: guard(async (args) => {
    const valid = validatedEngineConfig(args.flags ?? {});
    updateConfig((cfg) => {
      cfg.engine = valid;
    });
    setEngineConfig(valid);
    return valid;
  }),
  settings_approval_timers: guard(async () => {
    const cfg = loadConfig();
    return {
      plan_auto_accept_secs: cfg.plan_auto_accept_secs ?? 0,
      terminal_approval_secs: cfg.terminal_approval_secs ?? 60,
    };
  }),
  settings_set_approval_timers: guard(async (args) => {
    const timers = args.timers ?? {};
    updateConfig((cfg) => {
      cfg.plan_auto_accept_secs = Math.min(MAX_APPROVAL_TIMER_SECS, Math.max(0, Math.round(Number(timers.plan_auto_accept_secs) || 0)));
      cfg.terminal_approval_secs = Math.min(MAX_APPROVAL_TIMER_SECS, Math.max(0, Math.round(Number(timers.terminal_approval_secs) || 0)));
    });
    const cfg = loadConfig();
    return { plan_auto_accept_secs: cfg.plan_auto_accept_secs, terminal_approval_secs: cfg.terminal_approval_secs };
  }),
  settings_model_memory: guard(async () => memoryPercentForModels(loadConfig())),
  settings_set_model_memory: guard(async (args) => {
    const applied = Math.min(MAX_MODEL_MEMORY_PERCENT, Math.max(MIN_MODEL_MEMORY_PERCENT, Math.round(Number(args.percent) || 0)));
    updateConfig((cfg) => {
      cfg.model_memory_percent = applied;
    });
    return applied;
  }),
  settings_theme: guard(async () => loadConfig().theme ?? null),
  settings_set_theme: guard(async (args) => {
    const value = args.theme === "dark" || args.theme === "light" ? args.theme : null;
    updateConfig((cfg) => {
      cfg.theme = value;
    });
  }),
  chat_prefs_get: guard(async () => {
    const cfg = loadConfig();
    return {
      system_prompt: cfg.chat_system_prompt ?? null,
      temperature: cfg.chat_temperature ?? null,
      max_tokens: cfg.chat_max_tokens ?? null,
      persona: cfg.chat_persona ?? null,
    };
  }),
  chat_prefs_set: guard(async (args) => {
    const prefs = args.prefs ?? {};
    updateConfig((cfg) => {
      const prompt = prefs.system_prompt?.trim();
      cfg.chat_system_prompt = prompt ? prompt : null;
      cfg.chat_temperature = num(prefs.temperature) === null ? null : Math.min(2, Math.max(0, Number(prefs.temperature)));
      cfg.chat_max_tokens = num(prefs.max_tokens) === null ? null : Math.min(8192, Math.max(64, Math.round(Number(prefs.max_tokens))));
      const persona = prefs.persona?.trim();
      cfg.chat_persona = persona ? persona : null;
    });
    const prompt = loadConfig().chat_system_prompt;
    if (prompt) skills.recordPromptUsage(prompt);
  }),

  // Developer
  developer_get: guard(async () => developer.developerGet()),
  developer_set: guard(async (args) => developer.developerSet(args.update)),
  developer_reset: guard(async () => developer.developerReset()),

  // Models
  model_list: guard(async () => models.modelList()),
  model_catalog_status: guard(async () => models.catalogStatus()),
  model_catalog_refresh: guard(async () => {
    const active = models.catalogStatus().version;
    const manifest = await downloads.fetchCatalog(active);
    // Persist the verified manifest next to the config, atomically.
    const cache = catalogCachePath();
    fs.mkdirSync(path.dirname(cache), { recursive: true });
    const tmp = `${cache}.tmp`;
    fs.writeFileSync(tmp, JSON.stringify({ manifest, fetched_at: Math.floor(Date.now() / 1000) }, null, 2), "utf8");
    fs.renameSync(tmp, cache);
    models.resetCatalogCache();
    return models.catalogStatus();
  }),
  model_catalog_reset: guard(async () => {
    models.resetCatalogCache();
    return models.catalogStatus();
  }),
  model_activate: guard(async (args) => {
    models.setActiveModel(String(args.id ?? ""));
  }),
  model_remove: guard(async (args) => {
    const id = String(args.id ?? "");
    if (!models.isSafeModelId(id)) throw AppError.invalidInput("That model id is not valid.");
    models.removeModel(id);
  }),
  model_verify: guard(async (args) => models.verifyModel(String(args.id ?? ""))),
  model_presets: guard(async () => models.modelPresets()),
  model_download: guard(async (args) => {
    const entry = models.getEntry(String(args.id ?? ""));
    if (!entry) throw AppError.notFound("That model is not in the catalog.");
    if (models.isInstalled(entry.id)) return;
    const send = (event) => ch(args).send(event);
    return downloads.startModelDownload(entry, ch(args).id, send);
  }),
  model_download_preset: guard(async (args) => {
    const preset = models.modelPresets().find((p) => p.id === String(args.id ?? ""));
    if (!preset) throw AppError.invalidInput(`Unknown preset: ${args.id}`);
    // One model at a time, in the preset's order; a paused one resumes.
    const send = (event) => ch(args).send(event);
    const channelId = ch(args).id;
    void (async () => {
      for (const id of preset.models) {
        const entry = models.getEntry(id);
        if (!entry || models.isInstalled(id)) continue;
        await new Promise((resolve) => {
          const jobSend = (event) => {
            send(event);
            if (event.status === "done" || event.status === "failed" || event.status === "paused" || event.status === "cancelled") {
              resolve();
            }
          };
          try {
            downloads.startModelDownload(entry, channelId, jobSend);
          } catch (e) {
            if (!String(e?.message ?? "").includes("already downloading")) resolve();
            else resolve();
          }
        });
      }
    })();
  }),
  model_download_cancel: guard(async (args) => {
    downloads.cancelModelDownload(args.id);
  }),
  model_download_pause: guard(async (args) => downloads.pauseModelDownload(args.id)),
  model_preset_cancel: guard(async (args) => {
    const preset = models.modelPresets().find((p) => p.id === String(args.id ?? ""));
    if (preset) downloads.cancelPresetDownload(preset.models);
  }),

  // Hugging Face (the community model search)
  hf_search: guard(async (args) => {
    const query = String(args.query ?? "").trim();
    const sort = ["trending", "downloads", "likes", "newest"].includes(args.sort) ? args.sort : "downloads";
    const url = new URL("https://huggingface.co/api/models");
    if (query) url.searchParams.set("search", query);
    url.searchParams.set("sort", sort === "newest" ? "lastModified" : sort);
    url.searchParams.set("direction", "-1");
    url.searchParams.set("limit", "30");
    const response = await fetch(url, { headers: { Accept: "application/json" }, signal: AbortSignal.timeout(15_000) });
    if (!response.ok) throw AppError.internal(`The Hugging Face search answered ${response.status}.`);
    const body = await response.json();
    return (Array.isArray(body) ? body : []).map((repo) => ({
      id: String(repo.id ?? ""),
      downloads: repo.downloads ?? 0,
      likes: repo.likes ?? 0,
      gated: repo.gated === true,
      license: repo.card?.license ?? null,
      uncensored: /uncensored|abliterated/i.test(String(repo.id ?? "")),
      created_at: repo.createdAt ?? repo.lastModified ?? null,
    }));
  }),
  hf_repo_files: guard(async (args) => {
    const repo = String(args.repo ?? "").trim();
    const response = await fetch(`https://huggingface.co/api/models/${encodeURIComponent(repo)}/tree/main`, {
      headers: { Accept: "application/json" },
      signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok) throw AppError.internal(`That repository answered ${response.status}.`);
    const body = await response.json();
    return (Array.isArray(body) ? body : [])
      .filter((f) => /\.(gguf|onnx|txt)$/i.test(String(f.path ?? "")))
      .map((f) => ({
        path: String(f.path ?? ""),
        size: f.size ?? 0,
        sha256: f.lfs?.oid ?? f.oid ?? "",
        is_mmproj: /mmproj/i.test(String(f.path ?? "")),
      }));
  }),
  hf_custom_add: guard(async (args) => {
    const repo = String(args.repo ?? "").trim();
    const filePath = String(args.path ?? "").trim();
    const kind = ["chat", "code", "embedding", "reranker", "stt"].includes(args.kind) ? args.kind : "chat";
    if (!repo || !filePath) throw AppError.invalidInput("A community model needs its repository and file.");
    const url = `https://huggingface.co/${repo}/resolve/main/${filePath}`;
    const id = filePath.replace(/\.gguf$/i, "").replace(/[^A-Za-z0-9_.-]+/g, "-").slice(0, 100) || "custom-model";
    let customs = [];
    try {
      customs = JSON.parse(fs.readFileSync(customModelsPath(), "utf8"));
      if (!Array.isArray(customs)) customs = [];
    } catch {
      customs = [];
    }
    const existing = customs.findIndex((m) => m.id === id);
    const entry = {
      id,
      name: id,
      kind,
      license: args.license ?? "",
      file_size: 0,
      ram_estimate_gb: 0,
      quant: "",
      description: "",
      sha256: "",
      url,
      filename: filePath.split("/").pop(),
      companions: args.withMmproj ? [] : [],
      source: "community",
    };
    if (existing >= 0) customs[existing] = entry;
    else customs.push(entry);
    fs.writeFileSync(customModelsPath(), JSON.stringify(customs, null, 2), "utf8");
    return { id };
  }),

  // Inference
  inference_status: guard(async () => ({ loaded: engineStatus().loaded, loading: false })),
  inference_load: guard(async (args) => {
    const id = String(args.id ?? "");
    const entry = models.getEntry(id);
    if (!entry) throw AppError.notFound(`No model called ${id} is in the catalog.`);
    const modelPath = models.modelFilePath(id);
    if (!modelPath) throw AppError.notFound(`That model is not downloaded yet. Install ${id} first.`);
    const cfg = loadConfig();
    const context = models.effectiveContext(id, cfg.context_tokens);
    // A vision projector rides along when the model ships one. Companion
    // files sit flat in models/ under their dest name.
    let mmproj = null;
    for (const c of entry.companions ?? []) {
      if (String(c.dest ?? "").includes("mmproj")) {
        const candidate = path.join(modelsDir(), String(c.dest));
        if (fs.existsSync(candidate)) mmproj = candidate;
      }
    }
    await loadEngine({ modelPath, context, mmproj });
    models.setActiveModel(id);
    return id;
  }),
  inference_unload: guard(async () => {
    unloadEngine();
  }),
  engine_info: guard(async () => {
    void probe.gpuName();
    return engineInfo();
  }),
  engine_fetch: guard(async () => {
    const accel = loadConfig().engine?.accel || "cpu";
    await downloads.ensureEngine(accel);
    return engineInfo();
  }),

  // Chat
  chat_stream: guard(async (args) => {
    const req = args.req ?? {};
    if (req.thread_id) ensureThread(String(req.thread_id));
    return startChat(req, ch(args).id);
  }),
  chat_cancel: guard(async (args) => {
    cancelJob(String(args.jobId ?? ""));
  }),
  chat_steer: guard(async (args) => steerJob(String(args.req?.job_id ?? ""), args.req?.text)),
  chat_tool_decide: guard(async (args) => decideTool(String(args.req?.approval_id ?? ""), args.req?.decision)),
  summarize: guard(async (args) => {
    const req = args.req ?? {};
    const text = String(req.text ?? "").trim();
    if (!text) throw AppError.invalidInput("Nothing to summarize yet.");
    return { summary: await summarizeText(text, req.max_words ?? 256) };
  }),
  chat_models: guard(async () => buildChatChoices()),
  chat_target_set: guard(async (args) => {
    const req = args.req ?? {};
    const previousContext = engineStatus().context ?? 0;
    if (req.provider_id) {
      if (!onlineAllowed(loadConfig())) {
        throw AppError.invalidInput("Online providers are switched off. Switch them on in Settings → Online providers.");
      }
      updateConfig((cfg) => {
        const provider = (cfg.cloud_providers ?? []).find((p) => p.id === req.provider_id);
        if (!provider) throw AppError.invalidInput("That provider no longer exists.");
        if (!String(req.model ?? "").trim()) throw AppError.invalidInput("Choose a model.");
        provider.enabled = true;
        cfg.cloud_llm_enabled = true;
        cfg.active_cloud_provider = provider.id;
        cfg.active_cloud_model = String(req.model).trim();
      });
    } else {
      const id = String(req.model ?? "");
      const entry = models.getEntry(id);
      if (!entry) throw AppError.notFound(`No model called ${id} is in the catalog.`);
      const file = models.modelFilePath(id);
      if (!file) throw AppError.notFound(`That model is not downloaded yet. Install ${id} first.`);
      const status = engineStatus();
      if (status.loaded && path.basename(file) === status.model) {
        models.setActiveModel(id);
      } else {
        const cfg = loadConfig();
        await loadEngine({ modelPath: file, context: models.effectiveContext(id, cfg.context_tokens) });
        models.setActiveModel(id);
      }
      updateConfig((cfg) => {
        cfg.cloud_llm_enabled = false;
      });
    }
    const choices = buildChatChoices();
    const newContext = engineStatus().context ?? 0;
    if (previousContext > 0 && previousContext !== newContext) {
      choices.budget_note = `Context budget recalculated for the new model: ${previousContext} → ${newContext} tokens. This conversation was kept.`;
    }
    return choices;
  }),
  chat_favorite_toggle: guard(async (args) => {
    const key = String(args.key ?? "").trim();
    if (!key || key.length > 300) throw AppError.invalidInput("That model key is not valid.");
    let favorites = [];
    updateConfig((cfg) => {
      const list = cfg.favorite_models ?? [];
      const at = list.indexOf(key);
      if (at >= 0) list.splice(at, 1);
      else list.push(key);
      cfg.favorite_models = list;
      favorites = [...list];
    });
    return favorites;
  }),
  chat_starters: guard(async () => assist.chatStarters()),
  chat_enhance_prompt: guard(async (args) => assist.enhancePrompt(args.text, args.target)),
  chat_suggest_next: guard(async (args) => assist.suggestNext(args.question, args.answer, args.target)),
  chat_title: guard(async (args) => assist.chatTitle(args.question, args.answer, args.target)),
  chat_attach_pick: guard(async () => attach.attachPick()),
  chat_attach_dropped: guard(async (args) => attach.attachDropped(args.paths)),

  // Threads
  chat_thread_list: guard(async () => listThreads()),
  chat_thread_load: guard(async (args) => loadThread(String(args.threadId ?? ""))),
  chat_thread_save: guard(async (args) => {
    saveThread({
      threadId: String(args.threadId ?? ""),
      title: args.title,
      systemPrompt: args.systemPrompt ?? null,
      messages: Array.isArray(args.messages) ? args.messages : [],
      folder: args.folder ?? null,
    });
  }),
  chat_thread_delete: guard(async (args) => {
    deleteThread(String(args.threadId ?? ""));
  }),
  chat_thread_export_save: guard(async (args) => {
    const req = args.req ?? {};
    const exportData = exportThread(String(req.thread_id ?? ""), req.format);
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const result = await dialog.showSaveDialog(win, {
      title: "Save chat",
      defaultPath: exportData.filename,
      filters: [exportData.filename.endsWith(".json")
        ? { name: "JSON", extensions: ["json"] }
        : { name: "Markdown", extensions: ["md"] }],
    });
    if (result.canceled || !result.filePath) return null;
    fs.writeFileSync(result.filePath, exportData.content, "utf8");
    return result.filePath;
  }),
  chat_export_text_save: guard(async (args) => {
    const content = String(args.content ?? "");
    if (content.length > 32 * 1024 * 1024) {
      throw AppError.invalidInput("This chat is too large to save as one file.");
    }
    const suggested = `${fileStem(String(args.fileName ?? "").replace(/\.md$/i, ""))}.md`;
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const result = await dialog.showSaveDialog(win, {
      title: "Save chat",
      defaultPath: suggested,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (result.canceled || !result.filePath) return null;
    fs.writeFileSync(result.filePath, content, "utf8");
    return result.filePath;
  }),

  // Desktop
  pick_folder: guard(async () => {
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const result = await dialog.showOpenDialog(win, { title: "Choose a folder", properties: ["openDirectory"] });
    return result.canceled || !result.filePaths[0] ? null : result.filePaths[0];
  }),
  pick_document: guard(async () => {
    const win = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const result = await dialog.showOpenDialog(win, {
      title: "Choose a document",
      properties: ["openFile"],
      filters: [{ name: "Documents", extensions: ["md", "txt", "pdf", "docx", "csv", "json", "html"] }],
    });
    return result.canceled || !result.filePaths[0] ? null : result.filePaths[0];
  }),
  open_link: guard(async (args) => {
    const raw = String(args.url ?? "").trim();
    let parsed;
    try {
      parsed = new URL(raw);
    } catch {
      throw AppError.invalidInput("That link is not a web address.");
    }
    if ((parsed.protocol !== "https:" && parsed.protocol !== "http:") || !parsed.hostname || parsed.username || parsed.password) {
      throw AppError.invalidInput("Only web links can open.");
    }
    await shell.openExternal(parsed.toString());
  }),
  files_open: guard(async (args) => assist.openFile(args.folder, args.path)),
  files_reveal: guard(async (args) => assist.revealFile(args.folder, args.path)),
  files_open_with_apps: guard(async () => assist.openWithApps()),
  files_open_with: guard(async (args) => assist.openFileWith(args.folder, args.path, args.appId)),

  // Library and search
  library_status: guard(async () => library.libraryStatus()),
  library_folder_presets: guard(async () => library.libraryFolderPresets()),
  library_folder_remove: guard(async (args) => library.libraryFolderRemove(args.path)),
  library_index_start: guard(async (args) => {
    const folder = String(args.folder ?? "").trim();
    if (!fs.existsSync(folder) || !fs.statSync(folder).isDirectory()) {
      throw AppError.invalidInput(`There is no folder at ${folder}.`);
    }
    const send = (progress) => {
      try {
        ch(args).send(progress);
      } catch {
        /* the channel may be gone before the walk ends */
      }
    };
    const controller = new AbortController();
    indexJob = controller;
    let seen = 0;
    let skippedCount = 0;
    try {
      const report = indexer.indexFolder(folder, {
        cancel: controller.signal,
        onEvent: (event) => {
          if (event.kind === "skipped") skippedCount += 1;
          seen += 1;
          send({
            folder,
            seen,
            indexed: seen - skippedCount,
            skipped: skippedCount,
            current: path.relative(folder, event.file ?? ""),
          });
        },
      });
      indexer.saveFolderScan(folder, report.indexed, report.skipped, report.limited);
      send({ folder, seen, indexed: report.indexed, skipped: report.skipped, current: "" });
      return {
        folder,
        indexed: report.indexed,
        skipped: report.skipped,
        limited: report.limited,
        cancelled: report.cancelled,
        ms: report.ms,
        reasons: report.reasons,
        skipped_files: report.skipped_files,
      };
    } finally {
      indexJob = null;
    }
  }),
  library_index_cancel: guard(async () => {
    if (indexJob) {
      indexJob.abort();
      return true;
    }
    return false;
  }),
  library_compact: guard(async () => library.compactLibrary()),
  indexer_stats: guard(async () => library.indexerStats()),
  search_index: guard(async (args) => library.searchIndex(args)),
  list_index: guard(async (args) => library.listIndex(args)),
  index_folder: guard(async (args) => {
    const folder = String(args.folder ?? "").trim();
    const report = indexer.indexFolder(folder);
    indexer.saveFolderScan(folder, report.indexed, report.skipped, report.limited);
    return {
      indexed: report.indexed,
      skipped: report.skipped,
      limited: report.limited,
    };
  }),
  index_file: guard(async (args) => indexer.indexFile(String(args.path ?? ""))),
  index_remove: guard(async (args) => {
    library.removeIndexedDoc(args.path);
  }),
  index_watch_start: guard(async (args) => indexer.startWatch(String(args.folder ?? ""))),
  index_watch_stop: guard(async () => {
    indexer.stopWatch();
  }),
  embedding_status: guard(async () => embedding.embeddingStatus()),
  embedding_start: guard(async (args) => embedding.startEmbeddingEngine(args.modelId ?? null)),
  embedding_stop: guard(async () => {
    embedding.stopEmbeddingEngine();
  }),
  embedding_backfill: guard(async () => embedding.backfillEmbeddings()),
  library_embedding_select: guard(async (args) => embedding.selectEmbeddingModel(String(args.modelId ?? ""))),
  rerank_status: guard(async () => ({ model_id: null, ready: false })),
  rerank_start: guard(async (args) => rerank.startRerankEngine(args.modelId ?? null)),
  rerank_stop: guard(async () => {}),

  // Usage
  usage_summary: guard(async (args) => usageSummary(args.days ?? 30, args.folder ?? null)),
  usage_record: guard(async (args) =>
    ledgerAdd({
      provider: args.provider,
      model: args.model,
      inputTokens: args.inputTokens,
      outputTokens: args.outputTokens,
      threadId: args.threadId,
      ratePerMillionUsd: args.ratePerMillionUsd ?? null,
    })),
  usage_list: guard(async (args) => ledgerList(args.threadId ?? null, args.limit ?? 200)),

  // Web search
  web_search: guard(async (args) => {
    const query = String(args.query ?? "").trim();
    if (!query) throw AppError.invalidInput("Enter something to search for.");
    const provider = args.provider || undefined;
    const cfg = loadConfig();
    const chosen = provider && provider !== "auto" ? provider : cfg.web_search?.provider || "auto";
    return websearch.searchWeb(chosen, query, args.limit ?? 8);
  }),
  web_crawl: guard(async (args) => websearch.readPage(String(args.url ?? ""), args.provider || "auto")),
  web_providers: guard(async () => websearch.webProviders()),
  web_set_key: guard(async (args) => websearch.setWebKey(String(args.provider ?? ""), args.key ?? null)),
  web_set_default: guard(async (args) => websearch.setWebDefault(String(args.provider ?? ""))),
  web_set_firecrawl_url: guard(async (args) => websearch.setFirecrawlUrl(args.url ?? null)),

  // Cloud providers
  cloud_config_get: guard(async () => cloud.cloudConfigStatus()),
  cloud_provider_presets: guard(async () => cloud.PROVIDER_PRESETS),
  cloud_provider_save: guard(async (args) => cloud.saveCloudProvider(args.req ?? {})),
  cloud_provider_delete: guard(async (args) => cloud.deleteCloudProvider(String(args.providerId ?? ""))),
  cloud_mode_set: guard(async (args) => cloud.setCloudMode(args.req ?? {})),
  cloud_online_set: guard(async (args) => cloud.setOnlineAllowed(args.enabled === true)),
  cloud_provider_set_enabled: guard(async (args) =>
    cloud.setProviderEnabled(String(args.req?.provider_id ?? ""), args.req?.enabled === true)),
  cloud_provider_test: guard(async (args) => cloud.testProvider(String(args.providerId ?? ""))),
  cloud_list_models: guard(async (args) => cloud.listProviderModels(args.req ?? {})),
  cloud_models_refresh: guard(async (args) => cloud.refreshProviderModels(String(args.providerId ?? ""))),
  cloud_audit_list: guard(async (args) => cloud.auditList(args.limit ?? 200)),
  cloud_audit_clear: guard(async () => cloud.auditClear()),

  // Local servers on this computer (Settings → the provider form's scan)
  local_servers_discover: guard(async () => discoverLocalServers()),

  // Memory
  memory_create: guard(async (args) => ({ path: memory.createNote(args.title, args.content, args.tags ?? null).path })),
  memory_update: guard(async (args) => {
    memory.updateNote(args.path, args.content, args.tags ?? null);
  }),
  memory_list: guard(async () => memory.listNotes()),
  memory_read: guard(async (args) => memory.readNote(String(args.path ?? ""))),
  memory_delete: guard(async (args) => {
    memory.deleteNote(String(args.path ?? ""));
  }),
  memory_pinned_set: guard(async (args) => memory.setPinned(String(args.path ?? ""), args.pinned === true)),
  memory_pending_list: guard(async () => memory.pendingSuggestions()),
  memory_pending_accept: guard(async (args) => memory.acceptSuggestion(String(args.file ?? ""), args.tags ?? null)),
  memory_pending_dismiss: guard(async (args) => memory.dismissSuggestion(String(args.file ?? ""))),
  memory_forget_all: guard(async () => ({ deleted_count: memory.forgetAll() })),
  memory_search: guard(async (args) => memory.searchNotes(String(args.query ?? ""))),

  // Skills
  skills_list: guard(async () => skills.listSkills()),
  skills_read: guard(async (args) => skills.readSkill(String(args.path ?? ""))),
  skills_create: guard(async (args) => skills.createSkill(args.name, args.description, args.tags ?? [], args.body)),
  skills_update: guard(async (args) => skills.updateSkill(args.path, args.name, args.description, args.tags ?? [], args.body)),
  skills_delete: guard(async (args) => {
    skills.deleteSkill(String(args.path ?? ""));
  }),
  skills_undo: guard(async (args) => skills.undoSkill(args.changeId)),
  skills_sources: guard(async () => skills.SOURCES),
  skills_discover: guard(async (args) => skills.discoverSkills(String(args.source ?? ""), args.repo ?? null)),
  skills_remote_describe: guard(async (args) => skills.describeSkills(String(args.source ?? ""), args.ids ?? [], args.repo ?? null)),
  skills_remote_detail: guard(async (args) => skills.remoteSkillDetail(String(args.source ?? ""), String(args.id ?? ""), args.repo ?? null)),
  skills_import: guard(async (args) => skills.importSkill(String(args.source ?? ""), String(args.id ?? ""), args.repo ?? null)),
  skills_suggestions: guard(async () => skills.skillSuggestions(3)),

  // Safe tools and audits
  tool_execute: guard(async (args) => {
    const req = args.req ?? {};
    const action = req.action ?? {};
    const approved = req.approved === true;
    const nameByAction = {
      list_dir: "list_files",
      read_file: "read_file",
      write_file: "write_file",
      search_files: "recall",
    };
    const name = nameByAction[action.action_type];
    if (!name) {
      return { success: false, output: "", truncated: false, requires_approval: false, error: `No tool action ${action.action_type}.` };
    }
    const toolArgs =
      name === "write_file"
        ? { path: action.path, content: action.content ?? "" }
        : name === "recall"
          ? { query: action.query ?? action.path ?? "" }
          : { path: action.path };
    const risky = { read_file: false, list_files: false, recall: false, write_file: true }[name];
    const requiresApproval = risky && !approved;
    if (requiresApproval) {
      return { success: false, output: "", truncated: false, requires_approval: true, error: null };
    }
    const result = await runTool(name, toolArgs, { approved, cwdRoot: null });
    return {
      success: result.ok,
      output: result.output || result.summary,
      truncated: result.output.length >= 60_000,
      requires_approval: false,
      error: result.ok ? null : result.summary,
    };
  }),
  tool_audits_list: guard(async (args) => {
    const keep = Math.max(1, Math.min(1000, Math.round(Number(args.limit) || 100)));
    return all(`SELECT * FROM tool_audits ORDER BY timestamp DESC, id DESC LIMIT ?`, keep).map((row) => ({
      id: row.id,
      timestamp: row.timestamp,
      tool_name: row.tool_name,
      action: row.action,
      path: row.path ?? "",
      approved: row.approved === 1,
      status: row.status,
      details: row.details ?? null,
    }));
  }),
  tool_audits_clear: guard(async () => {
    run(`DELETE FROM tool_audits`);
  }),

  // Goals
  goal_start: guard(async (args) => goal.goalStart(args.threadId, args.objective, args.budget)),
  goal_status: guard(async (args) => goal.goalStatus(args.id)),
  goal_step: guard(async (args) => goal.goalStep(args.id, args.progress)),
  goal_complete: guard(async (args) => goal.goalComplete(args.id)),
  goal_pause: guard(async (args) => goal.goalPause(args.id)),
  goal_block: guard(async (args) => goal.goalBlock(args.id)),
  goal_resume: guard(async (args) => goal.goalResume(args.id)),

  // Automations
  automations_list: guard(async () => automations.automationsList()),
  automations_save: guard(async (args) => automations.automationsSave(args.automation)),
  automations_delete: guard(async (args) => automations.automationsDelete(args.id)),
  automations_run_now: guard(async (args) => automations.automationsRunNow(args.id)),

  // Diagnostics
  logs_tail: guard(async (args) => tail(args.source === "app" ? "app" : "engine", Number(args.lines) || 200)),
  doctor_run: guard(async () => doctor.doctorRun()),
  doctor_fix: guard(async () => doctor.doctorFix()),
  selftest_run: guard(async (args) => runSelfTest((step) => ch(args).send(step))),

  // Session search and side-by-side runs
  session_search: guard(async (args) => sessionSearch(String(args.query ?? ""))),
  session_reindex: guard(async () => sessionReindex()),
  multi_run_chat: guard(async (args) => multiRun(args.targets ?? [], String(args.prompt ?? ""))),

  // Policy, agents, workspace context
  policy_config_get: guard(async () => policyGet()),
  policy_config_set: guard(async (args) => policySet(args.update ?? {})),
  agents_list: guard(async () => agentsList()),
  agents_save: guard(async (args) => agentsSave(args.name, args.model, args.tools, args.prompt)),
  context_files_list: guard(async (args) => contextFilesList(args.roots ?? null, args.maxBytes ?? null)),
  context_refs_resolve: guard(async (args) => contextRefsResolve(args.reference, args.roots ?? null, args.maxBytes ?? null)),

  // OpenCode link
  opencode_get: guard(async () => opencode.get()),
  opencode_set: guard(async (args) => opencode.set({ baseUrl: args.baseUrl, password: args.password })),
  opencode_check: guard(async () => opencode.check()),
  opencode_send: guard(async (args) => opencode.send(args.title, args.task)),

  // Backups: config and listing work; the encrypted archive is a later port.
  backup_config_get: guard(async () => {
    const backup = loadConfig().backup ?? {};
    return {
      target_dir: backup.target_dir ?? null,
      include_api_keys: backup.include_api_keys === true,
      last_backup_at: backup.last_backup_at ?? null,
    };
  }),
  backup_config_set: guard(async (args) => {
    updateConfig((cfg) => {
      cfg.backup.target_dir = args.targetDir ? String(args.targetDir) : null;
      cfg.backup.include_api_keys = args.includeApiKeys === true;
    });
  }),
  backup_run: guard(async (args) =>
    backup.backupRun({
      passphrase: args.passphrase,
      includeApiKeys: args.includeApiKeys === true,
      targetDir: args.targetDir ?? null,
    })),
  backup_list: guard(async (args) => {
    const dir = args.targetDir ? String(args.targetDir) : backupDir();
    let names;
    try {
      names = fs.readdirSync(dir);
    } catch {
      return [];
    }
    return names
      .filter((n) => n.endsWith(".labak"))
      .map((name) => {
        const stat = fs.statSync(path.join(dir, name));
        return { file_name: name, created_at: Math.floor(stat.mtimeMs / 1000), size_bytes: stat.size, app_version: "" };
      })
      .sort((a, b) => b.created_at - a.created_at);
  }),
  backup_restore: guard(async (args) =>
    backup.backupRestore({
      fileName: args.fileName,
      passphrase: args.passphrase,
      restoreConfig: args.restoreConfig === true,
      targetDir: args.targetDir ?? null,
    })),

  // Voice: settings live in the config; the engines are a later port.
  voice_settings_get: guard(async () => voiceSettingsGet()),
  voice_settings_set: guard(async (args) => {
    const update = args.update ?? {};
    updateConfig((cfg) => {
      for (const key of ["enabled", "cleanup", "auto_send", "read_aloud"]) {
        if (update[key] !== undefined) cfg.voice[key] = update[key] === true;
      }
      if (update.local_only !== undefined) cfg.voice.local_only = update.local_only === true;
      if (update.model !== undefined) cfg.voice.model = String(update.model);
      if (update.rate !== undefined) cfg.voice.rate = Math.min(2, Math.max(0.5, Number(update.rate) || 1));
      if (update.voice_name !== undefined) cfg.voice.voice_name = update.voice_name || null;
    });
    return voiceSettingsGet();
  }),
  voice_key_set: guard(async (args) => {
    updateConfig((cfg) => {
      const key = String(args.key ?? "").trim();
      if (key) cfg.voice.keys[String(args.provider)] = key;
      else delete cfg.voice.keys[String(args.provider)];
    });
    return voiceSettingsGet();
  }),
  voice_models: guard(async () => voice.voiceModels()),
  voice_engine_fetch: guard(async (args) =>
    voice.engineFetch((event) => {
      try {
        ch(args).send({ model_id: "voice-engine", downloaded_bytes: event.bytes_downloaded, total_bytes: event.bytes_total, percent: event.percent, speed_bps: event.speed_bps, status: "downloading" });
      } catch {
        /* the channel may close before the download ends */
      }
    })),
  voice_transcribe: guard(async (args) => voice.transcribe(args.audio)),
  voice_live_start: guard(async (args) => {
    const cfg = loadConfig();
    if (cfg.voice?.local_only) {
      throw AppError.invalidInput("Local only is on, so online live models are blocked.");
    }
    const parsed = voice.parseChoice(String(cfg.voice?.model ?? ""));
    if (!parsed?.online || parsed.delivery !== "live") {
      throw AppError.invalidInput("Pick a live model (one marked live) in Settings → Voice.");
    }
    const send = (event) => {
      try {
        ch(args).send(event);
      } catch {
        /* the channel may close before the session ends */
      }
    };
    return voice.liveStart(parsed.provider, parsed.model, ch(args).id, send);
  }),
  voice_live_push: guard(async (args) => {
    const pcm = Buffer.from(String(args.pcm ?? ""), "base64");
    if (pcm.length > 16_000 * 2 * 5) {
      throw AppError.invalidInput("Send audio in pieces of at most 5 seconds.");
    }
    return voice.livePush(String(args.session ?? ""), pcm);
  }),
  voice_live_stop: guard(async (args) => voice.liveStop(String(args.session ?? ""))),
  voice_cleanup: guard(async (args) => voice.cleanup(args.text)),

  // Share my assistant: a local server behind a Cloudflare quick tunnel.
  share_status: guard(async () => share.shareStatus()),
  share_start: guard(async (args) => share.startShare({ tunnel: args.tunnel !== false, lan: args.lan === true })),
  share_stop: guard(async () => share.stopShare()),
  share_pair: guard(async () => share.pairInfo()),
  share_cloudflared_fetch: guard(async (args) =>
    share.fetchCloudflared((event) => {
      try {
        ch(args).send({ model_id: "cloudflared", downloaded_bytes: event.bytes_downloaded, total_bytes: event.bytes_total, percent: event.percent, speed_bps: event.speed_bps, status: "downloading" });
      } catch {
        /* the channel may close first */
      }
    })),


  // MCP servers: config-backed, with live stdio and HTTP clients.
  mcp_servers_list: guard(async () => mcp.listServers()),
  mcp_presets: guard(async () => mcp.MCP_PRESETS),
  mcp_artcraft_presets: guard(async () => mcp.artcraftPresets()),
  mcp_server_save: guard(async (args) => mcp.saveServer(args.req ?? {})),
  mcp_server_toggle: guard(async (args) => mcp.toggleServer(String(args.req?.id ?? ""), args.req ?? {})),
  mcp_server_delete: guard(async (args) => mcp.deleteServer(String(args.id ?? ""))),
  mcp_server_test: guard(async (args) => mcp.testServer(String(args.id ?? ""))),
  mcp_import: guard(async (args) => mcp.importServers(args.text)),
  mcp_oauth_start: guard(async (args) => mcp.oauthStart(String(args.req?.server_id ?? args.serverId ?? ""))),
  mcp_oauth_finish: guard(async (args) =>
    mcp.oauthFinish({
      serverId: String(args.req?.server_id ?? args.serverId ?? ""),
      state: String(args.req?.state ?? args.state ?? ""),
      code: String(args.req?.code ?? args.code ?? ""),
    })),

  // HTML plugin panels: absent in this build, the panel shows an empty state.
  plugin_list: guard(async () => []),

  render_frame_put: guard(async (args) => {
    // The sandboxed frame writes the mockup HTML under the app folder.
    const id = `frame-${Date.now().toString(36)}`;
    const dir = path.join(appDir(), "frames");
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, `${id}.html`), String(args.html ?? ""), "utf8");
    return id;
  }),
};

// ---- Helpers the table leans on ----------------------------------------

function voiceSettingsGet() {
  const voice = loadConfig().voice ?? {};
  return {
    enabled: voice.enabled === true,
    model: voice.model ?? "stt-whisper-tiny",
    local_only: voice.local_only !== false,
    cleanup: voice.cleanup === true,
    auto_send: voice.auto_send !== false,
    read_aloud: voice.read_aloud === true,
    voice_name: voice.voice_name ?? null,
    rate: voice.rate ?? 1,
    key_hints: Object.fromEntries(
      Object.entries(voice.keys ?? {}).map(([k, v]) => [k, String(v).slice(-4) ? `…${String(v).slice(-4)}` : ""]),
    ),
  };
}

/** The local-server probe, ported from commands/probe.rs. */
const LOCAL_SERVER_PRESETS = [
  { preset_id: "ollama-local", label: "Ollama", port: 11434, base_path: "/v1" },
  { preset_id: "lmstudio-local", label: "LM Studio", port: 1234, base_path: "/v1" },
  { preset_id: "llamacpp-local", label: "llama.cpp server", port: 8080, base_path: "/v1" },
  { preset_id: "mlx-local", label: "MLX", port: 8080, base_path: "/v1" },
  { preset_id: "localai-local", label: "LocalAI", port: 8080, base_path: "/v1" },
  { preset_id: "tabby-local", label: "Tabby", port: 8080, base_path: "/v1" },
  { preset_id: "vllm-local", label: "vLLM", port: 8000, base_path: "/v1" },
  { preset_id: "litellm-local", label: "LiteLLM proxy", port: 4000, base_path: "/v1" },
  { preset_id: "textgen-local", label: "text-generation-webui", port: 5000, base_path: "/v1" },
  { preset_id: "jan-local", label: "Jan", port: 1337, base_path: "/v1" },
  { preset_id: "koboldcpp-local", label: "KoboldCpp", port: 5001, base_path: "/v1" },
  { preset_id: "gpt4all-local", label: "GPT4All", port: 4891, base_path: "/v1" },
  { preset_id: "9router", label: "9Router", port: 20128, base_path: "/v1" },
  { preset_id: "sglang-local", label: "SGLang", port: 30000, base_path: "/v1" },
  { preset_id: "aphrodite-local", label: "Aphrodite", port: 2242, base_path: "/v1" },
  { preset_id: "docker-models-local", label: "Docker Model Runner", port: 12434, base_path: "/engines/v1" },
  { preset_id: "xinference-local", label: "Xinference", port: 9997, base_path: "/v1" },
];

/** One probe request; a closed port or a slow answer reads as "nothing there". */
async function fetchJson(url, timeoutMs = 1500) {
  try {
    const response = await fetch(url, { signal: AbortSignal.timeout(timeoutMs), headers: { Accept: "application/json" } });
    if (!response.ok) return null;
    return await response.json().catch(() => null);
  } catch {
    return null;
  }
}

export async function discoverLocalServers() {
  const probes = LOCAL_SERVER_PRESETS.map(async (preset) => {
    const base = `http://127.0.0.1:${preset.port}${preset.base_path}`;
    const body = await fetchJson(`${base}/models`);
    if (!body || !Array.isArray(body.data)) return null;
    const found = {
      preset_id: preset.preset_id,
      label: preset.label,
      base_url: base,
      models: body.data.map((m) => String(m.id ?? "")).filter(Boolean),
    };
    // Ports several servers share get a fingerprint, so the label is right.
    if (preset.port === 8080) {
      const props = await fetchJson(`${base}/props`);
      if (props?.default_generation_settings) {
        found.label = "llama.cpp server";
      } else {
        try {
          const ready = await fetch(`${base.replace("/v1", "")}/readyz`, { signal: AbortSignal.timeout(1000) });
          if (ready.ok && (await ready.text()).trim()) {
            found.label = "LocalAI";
            found.preset_id = "localai-local";
          }
        } catch {
          /* not LocalAI */
        }
        try {
          const health = await fetchJson(`${base.replace("/v1", "")}/v1/health`);
          if (health && JSON.stringify(health).includes("chat_model")) {
            found.label = "Tabby";
            found.preset_id = "tabby-local";
          }
        } catch {
          /* not Tabby */
        }
      }
    }
    return found;
  });
  const settled = await Promise.all(probes);
  return settled.filter(Boolean);
}

/** Keyword search over saved chats: title and message bodies. */
function sessionSearch(query) {
  const wanted = query.toLowerCase().trim();
  if (!wanted) return [];
  const dir = path.join(appDir(), "conversations");
  const hits = [];
  let names;
  try {
    names = fs.readdirSync(dir);
  } catch {
    return [];
  }
  for (const name of names) {
    if (!name.endsWith(".json") || hits.length >= 20) continue;
    let stored;
    try {
      stored = JSON.parse(fs.readFileSync(path.join(dir, name), "utf8"));
    } catch {
      continue;
    }
    const message = (stored.messages ?? []).find((m) => String(m.content ?? "").toLowerCase().includes(wanted));
    if (!message) continue;
    const body = String(message.content ?? "");
    const at = body.toLowerCase().indexOf(wanted);
    const start = Math.max(0, at - 60);
    const excerpt = body.slice(start, start + 180).replace(/\s+/g, " ").trim();
    // The UI bolds the term; the hit marks it with [ ] around the match.
    const snippet = excerpt.replace(new RegExp(wanted.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "i"), `[${body.slice(at, at + wanted.length)}]`);
    hits.push({
      thread_id: stored.id,
      title: stored.title ?? "Chat",
      snippet,
      path: path.join(dir, name),
    });
  }
  return hits;
}

/** The thread index is derived on the fly; reindex reports the file count. */
function sessionReindex() {
  const dir = path.join(appDir(), "conversations");
  try {
    return fs.readdirSync(dir).filter((n) => n.endsWith(".json")).length;
  } catch {
    return 0;
  }
}

/** One prompt to several OpenAI-compatible endpoints at once. */
async function multiRun(targets, prompt) {
  if (!prompt.trim()) throw AppError.invalidInput("Write the prompt to send.");
  const runs = targets.slice(0, 8).map(async (t) => {
    try {
      const result = await complete(String(t.endpoint ?? "").replace(/\/+$/, "") + "/chat/completions", {
        bearer: t.api_key || null,
        body: buildBody({
          model: String(t.model ?? ""),
          messages: [{ role: "user", content: prompt }],
          maxTokens: Math.max(64, Math.min(8192, Math.round(Number(t.max_tokens) || 1024))),
          effort: { kind: "auto", level: null },
          tools: [],
        }),
      });
      return { label: String(t.label ?? t.endpoint ?? ""), text: result.content, error: null };
    } catch (e) {
      return { label: String(t.label ?? t.endpoint ?? ""), text: "", error: e?.message ?? String(e) };
    }
  });
  return Promise.all(runs);
}

function policyGet() {
  const cfg = loadConfig();
  return {
    permissions: cfg.permissions ?? [],
    hooks: cfg.hooks ?? { pre_tool: null, post_tool: null, timeout_secs: 0 },
    routing: cfg.routing ?? { provider_priority: [], key_pools: {} },
    provider_order: (cfg.cloud_providers ?? []).filter((p) => p.enabled !== false).map((p) => p.id),
    auto_approve_capabilities: cfg.auto_approve_capabilities,
  };
}

function policySet(update) {
  updateConfig((cfg) => {
    if (Array.isArray(update.permissions)) cfg.permissions = update.permissions;
    if (update.hooks) cfg.hooks = { ...(cfg.hooks ?? {}), ...update.hooks };
    if (update.routing) cfg.routing = { ...(cfg.routing ?? {}), ...update.routing };
    if (update.auto_approve_capabilities) {
      cfg.auto_approve_capabilities = { ...(cfg.auto_approve_capabilities ?? {}), ...update.auto_approve_capabilities };
    }
  });
  return policyGet();
}

// ---- Agents (saved personas) --------------------------------------------

const agentsDir = () => {
  const dir = path.join(appDir(), "agents");
  fs.mkdirSync(dir, { recursive: true });
  return dir;
};

function parseAgentFile(file) {
  const raw = fs.readFileSync(file, "utf8");
  const front = new Map();
  let body = raw;
  if (raw.trimStart().startsWith("---")) {
    const after = raw.trimStart().slice(3);
    const end = after.indexOf("\n---");
    if (end >= 0) {
      for (const line of after.slice(0, end).split("\n")) {
        const at = line.indexOf(":");
        if (at > 0) front.set(line.slice(0, at).trim(), line.slice(at + 1).trim());
      }
      body = after.slice(end + 4).trim();
    }
  }
  return {
    name: front.get("name") || path.basename(file, ".md"),
    model: front.get("model") || "",
    tools: (front.get("tools") ?? "").split(",").map((t) => t.trim()).filter(Boolean),
    prompt: body,
    path: file,
  };
}

function agentsList() {
  try {
    return fs
      .readdirSync(agentsDir())
      .filter((n) => n.endsWith(".md"))
      .map((n) => parseAgentFile(path.join(agentsDir(), n)));
  } catch {
    return [];
  }
}

function agentsSave(name, model, tools, prompt) {
  const clean = String(name ?? "").trim();
  if (!clean) throw AppError.invalidInput("Give the agent a name.");
  const front = ["---", `name: ${clean}`, model ? `model: ${model}` : null, `tools: ${(tools ?? []).join(", ")}`, "---", "", String(prompt ?? "").trim()]
    .filter((l) => l !== null)
    .join("\n");
  const existing = agentsList().find((a) => a.name.toLowerCase() === clean.toLowerCase());
  const file = existing?.path ?? path.join(agentsDir(), `${clean.replace(/[^\p{L}\p{N}_-]+/gu, "-").toLowerCase() || "agent"}.md`);
  fs.writeFileSync(file, front, "utf8");
  return parseAgentFile(file);
}

// ---- Workspace context files --------------------------------------------

function contextFilesList(roots, maxBytes) {
  const cap = Math.max(1024, Math.min(1024 * 1024, Math.round(Number(maxBytes) || 65536)));
  const list = (Array.isArray(roots) && roots.length ? roots : [workspaceDir()]).map(String);
  const files = [];
  let truncated = false;
  const chunks = [];
  for (const root of list) {
    let names;
    try {
      names = fs.readdirSync(root);
    } catch {
      continue;
    }
    for (const name of names) {
      if (!/^([A-Z]+\.md|AGENTS\.txt|localaide\.md)$/i.test(name)) continue;
      const file = path.join(root, name);
      try {
        const stat = fs.statSync(file);
        if (!stat.isFile()) continue;
        const text = fs.readFileSync(file, "utf8");
        const kept = text.length > cap ? text.slice(0, cap) : text;
        if (kept.length < text.length) truncated = true;
        files.push({ path: file, bytes: stat.size });
        chunks.push(`# ${name}\n${kept}`);
      } catch {
        /* unreadable files are skipped */
      }
    }
  }
  return { text: chunks.join("\n\n"), files, truncated };
}

function contextRefsResolve(reference, roots, maxBytes) {
  const cap = Math.max(512, Math.min(1024 * 1024, Math.round(Number(maxBytes) || 16384)));
  const list = (Array.isArray(roots) && roots.length ? roots : [workspaceDir()]).map(String);
  const relative = String(reference ?? "").replace(/^@/, "").trim();
  for (const root of list) {
    const file = path.resolve(root, relative);
    if (!file.startsWith(path.resolve(root) + path.sep) && file !== path.resolve(root)) continue;
    try {
      const text = fs.readFileSync(file, "utf8");
      const lines = text.split("\n");
      const totalLines = lines.length;
      const kept = text.length > cap ? text.slice(0, cap) : text;
      return {
        path: file,
        text: kept,
        start_line: 1,
        end_line: kept.split("\n").length,
        total_lines: totalLines,
        truncated: kept.length < text.length,
      };
    } catch {
      /* try the next root */
    }
  }
  throw AppError.notFound(`No file called ${reference} in the workspace.`);
}

// ---- Registration ---------------------------------------------------------

export function registerCommands(ipc) {
  ipc.handle("localaide:invoke", async (_event, { cmd, args }) => {
    const handler = commands[cmd];
    if (!handler) {
      log("warn", "unhandled command", cmd);
      throw new AppError("NOT_IMPLEMENTED", `${cmd} is not available in this build yet.`);
    }
    return handler(args ?? {});
  });
}
