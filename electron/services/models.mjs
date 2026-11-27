// The model registry, minimal-but-compatible: the curated catalog plus the
// GGUF files actually on disk under ~/.aiassistant/models. Everything the
// picker, the Models screen and chat need to load and choose a model.
// The downloader is a later port; model_list still shows catalog entries
// with installed flags, so screens render honestly.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { catalogCachePath, customModelsPath, modelsDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { loadConfig, updateConfig } from "../core/config.mjs";
import { engineStatus } from "./engine.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const BUNDLED_CATALOG = path.join(here, "..", "..", "catalog", "models.json");

/** Hard limits on any manifest entering the app. */
export const MAX_MANIFEST_BYTES = 512 * 1024;
export const MAX_MODELS = 200;

const KNOWN_KINDS = new Set(["chat", "code", "embedding", "reranker", "stt"]);

/** Model ids become folder names under models/, so this must hold. */
export function isSafeModelId(id) {
  return (
    typeof id === "string" &&
    id.length > 0 &&
    id.length <= 128 &&
    !id.startsWith(".") &&
    /^[A-Za-z0-9_.\-]+$/.test(id)
  );
}

/** A file name inside a model folder: .gguf for most kinds. */
export function fileNameOk(kind, name) {
  if (!name || name.includes("/") || name.includes("\\") || name.includes("..") || name.includes(":")) {
    return false;
  }
  const lower = name.toLowerCase();
  if (kind === "stt") return /\.(onnx|ort|txt)$/.test(lower);
  return lower.endsWith(".gguf");
}

// ---- Capabilities, guessed from the model's name ----------------------

const hasAny = (id, needles) => needles.some((n) => id.includes(n));

export function guessReasoning(id) {
  const v = id.toLowerCase();
  if (hasAny(v, ["-instruct-2507", "non-reasoning", "nothink", "no-think"])) return false;
  return (
    hasAny(v, [
      "deepseek-r1", "deepseek-reasoner", "r1-distill", "qwq", "qwen3", "gpt-oss", "gpt-5",
      "o1", "o3", "o4-mini", "thinking", "reasoning", "magistral", "phi-4-reasoning",
      "phi-4-mini-reasoning", "glm-4.5", "glm-4.6", "glm-4.7", "glm-5", "kimi-k2-thinking",
      "kimi-k2.5", "minimax-m2", "gemini-2.5", "gemini-3", "claude-3-7", "claude-sonnet-4",
      "claude-opus-4", "claude-haiku-4", "grok-3-mini", "grok-4", "seed-oss", "hunyuan-a13b",
      "exaone-deep", "granite-4", "nemotron", "big-pickle",
    ]) && !hasAny(v, ["gpt-5-chat", "o1-mini-2024"])
  );
}

export function guessVision(id) {
  const v = id.toLowerCase();
  return hasAny(v, [
    "vision", "-vl", "vl-", "llava", "pixtral", "gpt-4o", "gpt-4.1", "gpt-5", "o3", "o4-mini",
    "gemini", "gemma-3", "gemma3", "gemma-4", "claude", "llama-4", "llama4", "mistral-small-3",
    "mistral-medium", "qwen2.5-vl", "qwen3-vl", "qwen3.5", "minicpm-v", "moondream", "grok-4",
    "kimi-k2.5", "glm-4.5v", "glm-4.6v", "phi-4-multimodal", "smolvlm", "internvl",
  ]);
}

export function guessTools(id) {
  const v = id.toLowerCase();
  return !hasAny(v, [
    "embed", "rerank", "whisper", "tts", "dall-e", "image", "moderation", "guard", "-base",
    "davinci", "babbage",
  ]);
}

export function guessContext(id) {
  const v = id.toLowerCase();
  if (hasAny(v, ["gemini"])) return 1_048_576;
  if (hasAny(v, ["gpt-4.1", "llama-4"])) return 1_000_000;
  if (hasAny(v, ["gpt-5", "claude"])) return 200_000;
  if (hasAny(v, ["gpt-4o", "o3", "o4-mini", "gpt-oss", "deepseek", "qwen3", "llama-3.1", "llama-3.3", "mistral", "kimi", "glm"])) return 128_000;
  return null;
}

/** Effort rungs the picker offers, lowest first, by family. */
export function effortLevels(id, local) {
  const v = id.toLowerCase();
  const owned = (list) => [...list];
  if (hasAny(v, ["gpt-oss"])) return owned(["low", "medium", "high"]);
  if (hasAny(v, ["gpt-5", "o1", "o3", "o4-mini"])) return owned(["minimal", "low", "medium", "high"]);
  if (hasAny(v, ["deepseek-r1", "deepseek-reasoner", "qwq", "r1-distill"])) return [];
  if (hasAny(v, ["grok-4", "grok-5"])) return owned(["minimal", "low", "medium", "high", "xhigh"]);
  if (local) return owned(["off", "on"]);
  return owned(["low", "medium", "high"]);
}

export function capabilities(id, local) {
  const reasoning = guessReasoning(id);
  return {
    reasoning,
    vision: guessVision(id),
    tools: guessTools(id),
    context: guessContext(id),
    efforts: reasoning ? effortLevels(id, local) : [],
  };
}

// ---- Catalog -----------------------------------------------------------

let catalogCache = null;

/** The active catalog: a fetched cache when valid, else the bundled one. */
export function manifest() {
  if (catalogCache) return catalogCache;
  for (const file of [catalogCachePath(), BUNDLED_CATALOG]) {
    try {
      const text = fs.readFileSync(file, "utf8");
      if (Buffer.byteLength(text) > MAX_MANIFEST_BYTES) continue;
      const parsed = JSON.parse(text);
      if (Array.isArray(parsed?.models) && parsed.models.length <= MAX_MODELS) {
        catalogCache = parsed;
        return parsed;
      }
    } catch {
      /* try the next source */
    }
  }
  catalogCache = { version: 0, updated_at: 0, models: [] };
  return catalogCache;
}

export function catalogStatus() {
  const man = manifest();
  const cached = man.updated_at > 0 && fs.existsSync(catalogCachePath());
  return {
    version: man.version ?? 0,
    updated_at: man.updated_at ?? 0,
    model_count: (man.models ?? []).length,
    source: cached ? "refreshed" : "built-in",
    fetched_at: cached ? man.updated_at : null,
    url: "",
  };
}

export function resetCatalogCache() {
  try {
    fs.unlinkSync(catalogCachePath());
  } catch {
    /* already gone */
  }
  catalogCache = null;
}

/** The user's own model entries (~/.aiassistant/custom-models.json). */
function customEntries() {
  try {
    const parsed = JSON.parse(fs.readFileSync(customModelsPath(), "utf8"));
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

// ---- Installed files ---------------------------------------------------

/**
 * Model files sit flat in models/, named by the entry's `dest` (or the
 * catalog file's own name), exactly like the Rust build. A model id never
 * becomes a folder, so an existing install keeps loading.
 */
const modelFile = (entry) => path.join(modelsDir(), String(entry.dest ?? entry.filename ?? `${entry.id}.gguf`));

/** The primary GGUF file path for a model id, when it is on disk. */
export function modelFilePath(id) {
  const entry = getEntry(id);
  if (!entry) return null;
  const file = modelFile(entry);
  return fs.existsSync(file) ? file : null;
}

export function isInstalled(id) {
  return modelFilePath(id) !== null;
}

/** The on-disk name of the primary file (dest wins over the URL name). */
function installedFile(entry) {
  const file = modelFile(entry);
  if (fs.existsSync(file)) return path.basename(file);
  // Split GGUFs and hand-renamed files: the first flat .gguf that starts
  // with the model id.
  try {
    for (const name of fs.readdirSync(modelsDir())) {
      if (name.toLowerCase().endsWith(".gguf") && name.toLowerCase().startsWith(entry.id.toLowerCase())) {
        return name;
      }
    }
  } catch {
    /* not installed */
  }
  return null;
}

/** The catalog (or custom) entry for one id. */
export function getEntry(id) {
  if (!isSafeModelId(id)) return null;
  const man = manifest();
  const found = (man.models ?? []).find((m) => m.id === id);
  if (found) {
    const url = String(found.url ?? "");
    return {
      id: found.id,
      name: found.name ?? found.id,
      kind: KNOWN_KINDS.has(found.kind) ? found.kind : "chat",
      license: found.license ?? "",
      file_size: found.file_size ?? 0,
      ram_estimate_gb: found.ram_estimate_gb ?? 0,
      quant: found.quant ?? "",
      description: found.description ?? "",
      sha256: found.sha256 ?? "",
      url,
      filename: url.split("/").pop() ?? "",
      dest: found.dest ?? null,
      companions: found.companions ?? [],
    };
  }
  const custom = customEntries().find((m) => m.id === id);
  if (custom) {
    return {
      id: custom.id,
      name: custom.name ?? custom.id,
      kind: KNOWN_KINDS.has(custom.kind) ? custom.kind : "chat",
      license: custom.license ?? "",
      file_size: custom.file_size ?? 0,
      ram_estimate_gb: custom.ram_estimate_gb ?? 0,
      quant: custom.quant ?? "",
      description: custom.description ?? "",
      sha256: custom.sha256 ?? "",
      url: custom.url ?? "",
      filename: (custom.url ?? "").split("/").pop() ?? custom.filename ?? "",
      dest: custom.dest ?? null,
      companions: custom.companions ?? [],
    };
  }
  return null;
}

/** Every entry, catalog first, then the user's own and local-only files. */
export function allEntries() {
  const man = manifest();
  const entries = (man.models ?? []).map((m) => getEntry(m.id)).filter(Boolean);
  for (const custom of customEntries()) {
    if (!entries.some((e) => e.id === custom.id)) {
      const entry = getEntry(custom.id);
      if (entry) entries.push(entry);
    }
  }
  // Local-only models: a flat .gguf in models/ with no catalog entry.
  // Companion files (vision projectors) belong to their model, so they
  // never list as a model of their own.
  const companionFiles = new Set(
    entries.flatMap((e) => (e.companions ?? []).map((c) => String(c.dest ?? "").toLowerCase())),
  );
  try {
    for (const name of fs.readdirSync(modelsDir())) {
      if (!name.toLowerCase().endsWith(".gguf") || name.endsWith(".part")) continue;
      if (/mmproj/i.test(name) || companionFiles.has(name.toLowerCase())) continue;
      const stem = name.replace(/\.gguf$/i, "");
      if (entries.some((e) => e.id === stem || installedFile(e) === name)) continue;
      const file = path.join(modelsDir(), name);
      entries.push({
        id: stem,
        name: stem,
        kind: "chat",
        license: "",
        file_size: fs.statSync(file).size,
        ram_estimate_gb: Math.round((fs.statSync(file).size / 1024 ** 3) * 1.15 * 10) / 10,
        quant: "",
        description: "",
        sha256: "",
        url: "",
        filename: name,
        dest: null,
        companions: [],
      });
    }
  } catch {
    /* no models dir yet */
  }
  return entries;
}

/** The catalog's native context hint, else null. */
function contextHint(entry) {
  return entry?.context_tokens ?? entry?.context ?? null;
}

/** The context the engine actually gets: the model's hint caps the setting. */
export function effectiveContext(id, saved) {
  const hint = contextHint(getEntry(id));
  if (hint && hint > 0) return Math.min(saved, hint);
  return saved;
}

// ---- ModelInfo ----------------------------------------------------------

export function modelInfo(entry) {
  const cfg = loadConfig();
  const status = engineStatus();
  const filename = installedFile(entry) ?? entry.filename;
  let sizeBytes = 0;
  let partialBytes = 0;
  try {
    const primary = path.join(modelsDir(), filename);
    if (fs.existsSync(primary)) {
      sizeBytes = fs.statSync(primary).size;
      for (const c of entry.companions ?? []) {
        const dest = path.join(modelsDir(), String(c.dest ?? "").split("/").pop() ?? "");
        if (fs.existsSync(dest)) sizeBytes += fs.statSync(dest).size;
      }
    } else {
      const part = path.join(modelsDir(), `${filename}.part`);
      if (fs.existsSync(part)) partialBytes = fs.statSync(part).size;
    }
  } catch {
    /* stat failures read as "not installed" */
  }
  const vision = (entry.companions ?? []).some((c) => String(c.dest ?? "").includes("mmproj"));
  const active = status.model === filename || (!status.model && cfg.selected_models[0] === entry.id);
  return {
    id: entry.id,
    name: entry.name,
    filename,
    kind: entry.kind,
    size_bytes: sizeBytes || entry.file_size,
    ram_estimate_gb: entry.ram_estimate_gb,
    quant: entry.quant,
    license: entry.license,
    sha256: entry.sha256,
    download_url: entry.url,
    source: entry.url ? "catalog" : "community",
    vision,
    installed: sizeBytes > 0,
    verified: Boolean(entry.sha256) && verifiedCache.get(entry.id) === true,
    active,
    download_progress: null,
    partial_bytes: partialBytes,
    total_bytes: (entry.file_size ?? 0) + (entry.companions ?? []).reduce((n, c) => n + (c.file_size ?? 0), 0),
  };
}

/** model_list: every known model with its installed state. */
export function modelList() {
  const entries = allEntries();
  return entries.map((entry) => modelInfo(entry));
}

/** model_presets, verbatim id lists from the Rust model_manager. */
export function modelPresets() {
  const man = manifest();
  const byId = new Map((man.models ?? []).map((m) => [m.id, m]));
  const pick = (ids) => ids.filter((id) => byId.has(id));
  return [
    { id: "tiny", name: "Tiny", description: "Fits 4 GB+", models: pick(["qwen3.5-0.8b-q4", "embeddinggemma-300m-qat-q4"]), ram_gb: 1.8 },
    { id: "small", name: "Small", description: "Fits 8 GB+", models: pick(["qwen3.5-2b-q4", "gemma4-e2b-qat-q4", "embeddinggemma-300m-qat-q4"]), ram_gb: 6.8 },
    { id: "balanced", name: "Balanced", description: "Fits 16 GB+", models: pick(["qwen3.5-9b-q4", "gemma4-e4b-qat-q4", "embeddinggemma-300m-q8"]), ram_gb: 14.5 },
    { id: "advanced", name: "Advanced", description: "Fits 32 GB", models: pick(["gemma4-26b-a4b-q4", "qwen3.8-27b-q4", "nomic-embed-v2-moe-q8"]), ram_gb: 21.2 },
  ].filter((p) => p.models.length > 0);
}

const verifiedCache = new Map();

/** Removes a model's files from models/ (the flat layout). */
export function removeModel(id) {
  const entry = getEntry(id);
  if (!entry) return;
  const names = new Set([modelFile(entry), `${modelFile(entry)}.part`]);
  for (const c of entry.companions ?? []) {
    if (c.dest) names.add(path.join(modelsDir(), String(c.dest)));
  }
  // A hand-renamed primary file that starts with the id goes too.
  try {
    for (const name of fs.readdirSync(modelsDir())) {
      if (name.toLowerCase().endsWith(".gguf") && name.toLowerCase().startsWith(id.toLowerCase())) {
        names.add(path.join(modelsDir(), name));
      }
    }
  } catch {
    /* nothing to add */
  }
  for (const file of names) {
    try {
      fs.unlinkSync(file);
    } catch {
      /* already gone */
    }
  }
}

/** SHA-256 of a model file, cached per id per file mtime. */
export function verifyModel(id) {
  const entry = getEntry(id);
  if (!entry) throw AppError.notFound("That model is not in the catalog.");
  const file = modelFile(entry);
  if (!fs.existsSync(file)) {
    return { id, verified: false, expected_sha256: entry.sha256, actual_sha256: null, error: "The model file is not installed." };
  }
  try {
    const hash = crypto.createHash("sha256");
    hash.update(fs.readFileSync(file));
    const actual = hash.digest("hex");
    const verified = !entry.sha256 || actual === entry.sha256;
    verifiedCache.set(id, verified);
    return {
      id,
      verified,
      expected_sha256: entry.sha256,
      actual_sha256: actual,
      error: verified ? null : "The file does not match its catalog checksum.",
    };
  } catch (e) {
    return { id, verified: false, expected_sha256: entry.sha256, actual_sha256: null, error: String(e) };
  }
}

/** Marks a model as the one to load next (config.selected_models). */
export function setActiveModel(id) {
  updateConfig((cfg) => {
    cfg.selected_models = [id];
  });
}
