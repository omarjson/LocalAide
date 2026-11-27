// Downloads: model files, the llama.cpp engine, any pinned tool. A download
// writes to `<name>.part`, resumes with an HTTP Range request, hashes
// SHA-256 while it streams, verifies before the atomic rename, and keeps
// the part file on Pause or Cancel so "Resume" appears in the UI. Ported
// from services/models/download.rs, verify.rs, refresh.rs and
// services/externals.rs.

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { appDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { log } from "../core/log.mjs";
import { emitEvent } from "../core/channels.mjs";

/** Extra space kept free after a download, so the disk never fills mid-write. */
const DISK_MARGIN_BYTES = 512 * 1024 * 1024;
const CANCEL_POLL_MS = 200;
const PROGRESS_EVERY_MS = 200;

const gigabytes = (bytes) => (bytes / 1_000_000_000).toFixed(1);

/** Free bytes on the disk that holds `path`, or null when unknown. */
async function freeSpaceBytes(file) {
  try {
    const stats = await fs.promises.statfs(path.dirname(file));
    return stats.bavail * stats.bsize;
  } catch {
    return null;
  }
}

/**
 * Streams one HTTP body with a size cap. Reads in chunks so a slow server
 * keeps the progress events moving; resolves with the full Buffer.
 */
async function fetchCapped(url, { headers = {}, cap, onChunk, signal } = {}) {
  let response;
  try {
    response = await fetch(url, { headers, signal });
  } catch (e) {
    throw AppError.internal(`Network request to ${url} failed: ${e}`);
  }
  if (!response.ok) {
    throw AppError.internal(`Download returned HTTP status: ${response.status}`);
  }
  const reader = response.body.getReader();
  const chunks = [];
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (cap && total > cap) {
      await reader.cancel().catch(() => {});
      throw AppError.invalidInput(`Download exceeds the size limit of ${Math.round(cap / 1024)} KB`);
    }
    chunks.push(value);
    onChunk?.(value);
  }
  return { buffer: Buffer.concat(chunks), status: response.status };
}

/** SHA-256 of a file, streamed in 64 KB chunks (bounded RAM, multi-GB safe). */
function computeSha256(file) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = fs.createReadStream(file, { highWaterMark: 65536 });
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
    stream.on("error", reject);
  });
}

/** Verifies a finished .part file, then moves it into place. A bad file dies. */
async function finishPartialFile(partFile, dest, expectedSha256) {
  let actual;
  try {
    actual = await computeSha256(partFile);
  } catch (e) {
    fs.rmSync(partFile, { force: true });
    throw AppError.internal(`Could not hash the downloaded file: ${e}`);
  }
  if (expectedSha256 && expectedSha256.trim().toLowerCase() !== actual.toLowerCase()) {
    fs.rmSync(partFile, { force: true });
    throw new AppError(
      "CHECKSUM_MISMATCH",
      "The download did not pass its checksum, so it was removed. Start the download again.",
      { detail: `expected ${expectedSha256}, got ${actual}` },
    );
  }
  fs.renameSync(partFile, dest);
  return dest;
}

/**
 * One atomic download: Range resume, streaming hash is done at verify time
 * from the part file (which holds every byte). Cancel/pause goes through
 * the AbortSignal; the part file stays for resume.
 */
export async function downloadAtomic(url, dest, expectedSize, expectedSha256, { signal, onProgress } = {}) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  const partFile = `${dest}.part`;

  let offset = 0;
  try {
    offset = fs.statSync(partFile).size;
  } catch {
    /* no part yet */
  }

  // Refuse early when the disk cannot hold the rest of the file.
  const needed = Math.max(0, expectedSize - offset) + DISK_MARGIN_BYTES;
  const free = await freeSpaceBytes(dest);
  if (free !== null && free < needed) {
    throw AppError.internal(
      `Not enough free disk space. This model needs about ${gigabytes(needed)} GB, and ${gigabytes(free)} GB are free.`,
    );
  }

  const headers = {};
  if (offset > 0) headers.Range = `bytes=${offset}-`;

  let response;
  try {
    response = await fetch(url, { headers, signal });
  } catch (e) {
    if (e?.name === "AbortError") throw cancelled();
    throw AppError.internal(`Network request to ${url} failed: ${e}`);
  }

  // 416 to a Range request: the part file already holds every byte. Check it
  // and move it into place, instead of writing the server's error page.
  if (response.status === 416 && offset > 0) {
    return finishPartialFile(partFile, dest, expectedSha256);
  }
  if (!response.ok) {
    throw AppError.internal(`Download returned HTTP status: ${response.status}`);
  }

  let totalSize = null;
  const length = response.headers.get("content-length");
  if (length !== null) {
    totalSize = response.status === 206 ? Number(length) + offset : Number(length);
  }
  if (response.status === 200) {
    // The server ignored the Range header and sent the whole file.
    offset = 0;
  }

  const flags = offset > 0 ? "r+" : "w";
  const file = fs.openSync(partFile, flags);
  let downloaded = offset;
  let lastProgress = 0;
  let lastTime = Date.now();
  let bytesSince = 0;

  const report = (force = false) => {
    const now = Date.now();
    if (!force && now - lastTime < PROGRESS_EVERY_MS) return;
    const speedBps = bytesSince / ((now - lastTime) / 1000 || 1);
    lastTime = now;
    bytesSince = 0;
    const total = totalSize ?? Math.max(downloaded, expectedSize);
    const percent = total > 0 ? (downloaded / total) * 100 : 0;
    onProgress?.({ bytes_downloaded: downloaded, bytes_total: total, percent, speed_bps: speedBps });
  };

  const reader = response.body.getReader();
  try {
    let position = offset;
    while (true) {
      if (signal?.aborted) {
        fs.closeSync(file);
        report(true);
        throw cancelled();
      }
      const chunk = await Promise.race([
        reader.read(),
        new Promise((resolve) => setTimeout(() => resolve(null), CANCEL_POLL_MS)),
      ]);
      if (chunk === null) continue; // silent network: poll the cancel flag
      const { done, value } = chunk;
      if (done) break;
      fs.writeSync(file, value, 0, value.byteLength, position);
      position += value.byteLength;
      downloaded += value.byteLength;
      bytesSince += value.byteLength;
      report();
    }
    fs.closeSync(file);
  } catch (e) {
    try {
      fs.closeSync(file);
    } catch {
      /* already closed */
    }
    if (e?.name === "AbortError" || signal?.aborted) throw cancelled();
    throw AppError.internal(`Network stream error during download: ${e}`);
  }

  report(true);
  return finishPartialFile(partFile, dest, expectedSha256);
}

const cancelled = () => {
  const error = new AppError("CANCELLED", "The download was stopped. The part file stays, so it can resume.");
  return error;
};

/** A "cancelled" failure: pause and cancel both land here. */
export const isCancelled = (e) => e instanceof AppError && e.code === "CANCELLED";

// ---- Download jobs (model_download and friends) -------------------------

/** Running model downloads, keyed by model id, so cancel/pause can reach them. */
const modelJobs = new Map();

/**
 * Downloads one catalog entry: the primary file, then its companions.
 * Progress events carry the model id, the aggregate byte counts and status.
 */
async function downloadEntry(entry, signal, onProgress) {
  const targets = [
    { url: entry.url, sha256: entry.sha256, size: entry.file_size, dest: entry.dest ?? entry.filename ?? `${entry.id}.gguf` },
    ...(entry.companions ?? []).map((c) => ({
      url: c.url,
      sha256: c.sha256,
      size: c.file_size,
      dest: c.dest ?? (c.url.split("/").pop() ?? ""),
    })),
  ];
  const grandTotal = targets.reduce((n, t) => n + (t.size || 0), 0);
  let doneBytes = 0;
  for (const target of targets) {
    const dest = path.join(appDir(), "models", target.dest);
    const onChunkProgress = (p) => {
      onProgress?.({
        model_id: entry.id,
        downloaded_bytes: doneBytes + p.bytes_downloaded,
        total_bytes: grandTotal,
        percent: grandTotal > 0 ? ((doneBytes + p.bytes_downloaded) / grandTotal) * 100 : 0,
        speed_bps: p.speed_bps,
        status: "downloading",
      });
    };
    await downloadAtomic(target.url, dest, target.size || 0, target.sha256, {
      signal,
      onProgress: onChunkProgress,
    });
    doneBytes += target.size || 0;
    onProgress?.({
      model_id: entry.id,
      downloaded_bytes: doneBytes,
      total_bytes: grandTotal,
      percent: grandTotal > 0 ? (doneBytes / grandTotal) * 100 : 0,
      speed_bps: 0,
      status: targets.indexOf(target) === targets.length - 1 ? "done" : "downloading",
    });
  }
}

/**
 * model_download / model_download_preset: one job per model id, streamed
 * over the channel as DownloadProgressEvent. Resolves when the job ends.
 */
export function startModelDownload(entry, channelId, send) {
  const existing = modelJobs.get(entry.id);
  if (existing) {
    throw AppError.invalidInput("That model is already downloading.");
  }
  const controller = new AbortController();
  const job = { controller, cancelled: false };
  modelJobs.set(entry.id, job);
  void (async () => {
    try {
      await downloadEntry(entry, controller.signal, (event) => send(event));
    } catch (e) {
      const stopped = isCancelled(e);
      send({
        model_id: entry.id,
        downloaded_bytes: 0,
        total_bytes: 0,
        percent: 0,
        speed_bps: 0,
        status: stopped ? (job.cancelled ? "paused" : "cancelled") : "failed",
      });
      if (!stopped) {
        log("error", "model download failed", `${entry.id}: ${e?.message ?? e}`);
        send({
          model_id: entry.id,
          downloaded_bytes: 0,
          total_bytes: 0,
          percent: 0,
          speed_bps: 0,
          status: "failed",
        });
      }
    } finally {
      modelJobs.delete(entry.id);
    }
  })();
  return entry.id;
}

/** True when a download was running and is now stopping (pause keeps the part). */
export function pauseModelDownload(id) {
  const job = modelJobs.get(String(id));
  if (!job) return false;
  job.cancelled = true;
  job.controller.abort();
  return true;
}

export const cancelModelDownload = pauseModelDownload;

/** Cancel every in-flight download of a preset. */
export function cancelPresetDownload(presetModels) {
  let stopped = false;
  for (const id of presetModels) {
    const job = modelJobs.get(id);
    if (job) {
      job.cancelled = true;
      job.controller.abort();
      stopped = true;
    }
  }
  return stopped;
}

export const downloadRunning = (id) => modelJobs.has(String(id));

// ---- Catalog refresh (ADR-0008) -----------------------------------------

/** Where refreshed catalogs come from: this project's own repository. */
export const CATALOG_URL =
  "https://raw.githubusercontent.com/omarjson/LocalAide/main/catalog/models.json";
export const MAX_MANIFEST_BYTES = 512 * 1024;
export const MAX_MODELS = 200;

const KNOWN_KINDS = new Set(["chat", "code", "embedding", "reranker", "stt"]);
const SAFE_ID = /^[A-Za-z0-9_.\-]{1,128}$/;

/**
 * Digest of the catalog entries: detects accidental edits to the list. Not
 * a publisher signature — the model files themselves are pinned by SHA-256.
 */
export function computeCatalogSignature(models) {
  const hash = createHash("sha256");
  for (const m of models) {
    hash.update(String(m.id));
    hash.update(String(m.sha256));
    const size = Buffer.alloc(8);
    size.writeBigUInt64LE(BigInt(Math.max(0, Math.round(m.file_size ?? 0))));
    hash.update(size);
    for (const c of m.companions ?? []) {
      hash.update(String(c.sha256));
      const csize = Buffer.alloc(8);
      csize.writeBigUInt64LE(BigInt(Math.max(0, Math.round(c.file_size ?? 0))));
      hash.update(csize);
    }
  }
  return hash.digest("hex");
}

const isSha256Hex = (s) => /^[a-fA-F0-9]{64}$/.test(String(s ?? ""));

function fileNameOk(kind, name) {
  if (!name || name.includes("/") || name.includes("\\") || name.includes("..") || name.includes(":")) return false;
  const lower = String(name).toLowerCase();
  if (kind === "stt") return /\.(onnx|ort|txt)$/.test(lower);
  return lower.endsWith(".gguf");
}

function validateEntry(entry) {
  if (!SAFE_ID.test(String(entry.id ?? "")) || String(entry.id).startsWith(".")) {
    throw AppError.invalidInput(`Catalog model id '${entry.id}' is not a safe name`);
  }
  if (!KNOWN_KINDS.has(String(entry.kind))) {
    throw AppError.invalidInput(`Catalog model '${entry.id}' has an unknown kind`);
  }
  if (!isSha256Hex(entry.sha256)) {
    throw AppError.invalidInput(`Catalog model '${entry.id}' is not pinned by a SHA-256`);
  }
  if (!String(entry.url ?? "").toLowerCase().startsWith("https://")) {
    throw AppError.invalidInput(`Catalog model '${entry.id}' must point at an https address`);
  }
  for (const c of entry.companions ?? []) {
    if (!isSha256Hex(c.sha256) || !String(c.url ?? "").toLowerCase().startsWith("https://") || !fileNameOk(entry.kind, c.dest)) {
      throw AppError.invalidInput(`Catalog model '${entry.id}' has an invalid companion`);
    }
  }
}

/** Verify + structural validation for any manifest entering the app. */
export function verifyManifest(manifest, minVersion) {
  const models = manifest?.models;
  if (!Array.isArray(models) || models.length === 0) {
    throw AppError.invalidInput("Catalog has no models");
  }
  if (models.length > MAX_MODELS) {
    throw AppError.invalidInput(`Catalog lists ${models.length} models; the limit is ${MAX_MODELS}`);
  }
  if (typeof manifest.version === "number" && typeof minVersion === "number" && manifest.version < minVersion) {
    throw AppError.invalidInput(
      `Catalog version ${manifest.version} is older than the active version ${minVersion}; refusing rollback`,
    );
  }
  const expected = computeCatalogSignature(models);
  if (manifest.signature !== expected) {
    throw new AppError("CATALOG_UNVERIFIED", "Model catalog integrity signature mismatch");
  }
  const seen = new Set();
  for (const entry of models) {
    if (!entry.id || seen.has(entry.id)) {
      throw AppError.invalidInput(`Catalog model id '${entry.id}' is empty or duplicated`);
    }
    seen.add(entry.id);
    validateEntry(entry);
  }
  return manifest;
}

/** Fetch the newest catalog from the repository, capped and verified. */
export async function fetchCatalog(minVersion) {
  const { buffer } = await fetchCapped(CATALOG_URL, { cap: MAX_MANIFEST_BYTES });
  let manifest;
  try {
    manifest = JSON.parse(buffer.toString("utf8"));
  } catch {
    throw AppError.invalidInput("Catalog is not a valid manifest");
  }
  verifyManifest(manifest, minVersion);
  return manifest;
}

// ---- Pinned tool downloads (the llama.cpp engine) -----------------------

/** The llama.cpp release LocalAide runs models with. */
export const LLAMA_BUILD = "b12359";
const RELEASE = `https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_BUILD}`;

const ARCH = process.arch === "arm64" ? "arm64" : "x64";
const OS_NAME = process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux";
const EXT = process.platform === "win32" ? "exe" : "";
const exeName = EXT ? `llama-server.${EXT}` : "llama-server";

/** One pinned build: url, sha256, size, archive kind, and the tool folder. */
const pin = (tool, url, sha256, size, archive, root) => ({ tool, version: LLAMA_BUILD, url, sha256, size, archive, root });

/** Every pinned llama.cpp build, verbatim from services/externals.rs. */
export const ENGINE_PINS = {
  cpu: {
    "windows|x64": pin("llama.cpp", `${RELEASE}/llama-b12359-bin-win-cpu-x64.zip`, "837c1655e87df3b66989f4be33a792a1399c785bf259038dc75c900096683e69", 25_097_509, "zip", ""),
    "windows|arm64": pin("llama.cpp", `${RELEASE}/llama-b12359-bin-win-cpu-arm64.zip`, "d27564c754478d2832789dfcebfc5ace018e36a224c41d810b7cbc1ba8243ec0", 13_455_326, "zip", ""),
    "macos|arm64": pin("llama.cpp", `${RELEASE}/llama-b12359-bin-macos-arm64.tar.gz`, "82b00abc740160e1b9980e6cc79c117345fa34af058c80e6c0a69aa8ddccde75", 16_907_811, "targz", "llama-b12359"),
    "macos|x64": pin("llama.cpp", `${RELEASE}/llama-b12359-bin-macos-x64.tar.gz`, "25320d529077ef5a3d16e7b5da26438f221d88c8c0930a32b0c79457f5eede56", 12_661_621, "targz", "llama-b12359"),
    "linux|x64": pin("llama.cpp", `${RELEASE}/llama-b12359-bin-ubuntu-x64.tar.gz`, "0bc1f091e8916102b6d8f2cf944233a620e90a0f856c72d0ce41150c65c2e72e", 23_311_084, "targz", "llama-b12359"),
    "linux|arm64": pin("llama.cpp", `${RELEASE}/llama-b12359-bin-ubuntu-arm64.tar.gz`, "dc7aa17a67884619b7223870aca9fd6addbc01717713b04a714add1c31137359", 18_949_862, "targz", "llama-b12359"),
  },
  vulkan: {
    "windows|x64": pin("llama.cpp-vulkan", `${RELEASE}/llama-b12359-bin-win-vulkan-x64.zip`, "e8d18c6e4210cca87e7307c2ea78363ac8d158e4e1397358ec96ab70b53a0d5e", 38_090_643, "zip", ""),
    "windows|arm64": pin("llama.cpp-vulkan", `${RELEASE}/llama-b12359-bin-win-vulkan-arm64.zip`, "62cd50089cffdd825524a91c5a8819041eb75d8e2beacd6eca68194dba50574f", 26_027_698, "zip", ""),
    "linux|x64": pin("llama.cpp-vulkan", `${RELEASE}/llama-b12359-bin-ubuntu-vulkan-x64.tar.gz`, "19b60f00395a3f4954e46a5037695bc1d843404fc9d3124f82a612ecd8fbe9f0", 36_153_377, "targz", "llama-b12359"),
    "linux|arm64": pin("llama.cpp-vulkan", `${RELEASE}/llama-b12359-bin-ubuntu-vulkan-arm64.tar.gz`, "3f8ae812a58ac92e5a596ca9fbc988f52854d147c5a7fe1264dddc90c1d085b4", 29_435_221, "targz", "llama-b12359"),
  },
  rocm: {
    "windows|x64": pin("llama.cpp-rocm", `${RELEASE}/llama-b12359-bin-win-rocm-10.0-x64.zip`, "d5021cebbd3bd50a7cab3b9f121a887e45df76c360ecfa927940aac3bb64a982", 280_132_437, "zip", ""),
    "linux|x64": pin("llama.cpp-rocm", `${RELEASE}/llama-b12359-bin-ubuntu-rocm-10.0-x64.tar.gz`, "2d934c367e613ec67a5f78dffa450ee30bc3a6024f559b70c207896ee4bc9108", 270_816_049, "targz", "llama-b12359"),
  },
};

/** The pin for a build on this computer, or null when none ships. */
export function enginePinFor(accel = "cpu") {
  const table = ENGINE_PINS[accel === "vulkan" || accel === "rocm" ? accel : "cpu"];
  return table?.[`${OS_NAME}|${ARCH}`] ?? null;
}

/** Where a fetched tool is unpacked: app_dir/tools/<tool>-<version>. */
const toolDir = (tool) => path.join(appDir(), "tools", `${tool}-${LLAMA_BUILD}`);

/** The llama-server program for a build, when it is on this computer. */
export function engineBinaryOf(tool) {
  const file = path.join(toolDir(tool), exeName);
  return fs.existsSync(file) ? file : null;
}

let engineJob = null;

/**
 * Downloads and unpacks the llama.cpp build for this computer (or the
 * chosen graphics build). One download at a time; progress rides the
 * engine-download app event.
 */
export async function ensureEngine(accel = "cpu") {
  const wanted = enginePinFor(accel);
  const pinToUse = wanted ?? enginePinFor("cpu");
  if (!pinToUse) {
    throw AppError.invalidInput(`There is no model engine build for this computer (${OS_NAME} ${ARCH}).`);
  }
  const binary = engineBinaryOf(pinToUse.tool);
  if (binary) return binary;

  if (engineJob) throw AppError.invalidInput("The engine is already downloading.");
  const controller = new AbortController();
  engineJob = controller;

  const emit = (state, downloaded, total, percent, message = null) =>
    emitEvent("engine-download", { state, downloaded_bytes: downloaded, total_bytes: total, percent, message });

  const archive = path.join(toolDir(pinToUse.tool), `${pinToUse.tool}-${LLAMA_BUILD}.part`);
  try {
    emit("downloading", 0, pinToUse.size, 0);
    await downloadAtomic(pinToUse.url, archive, pinToUse.size, pinToUse.sha256, {
      signal: controller.signal,
      onProgress: (p) => emit("downloading", p.bytes_downloaded, p.bytes_total, p.percent),
    });
    // The verified archive sits at the final name; unpack with the system
    // tar (bsdtar on Windows and macOS reads zip files too).
    const dir = toolDir(pinToUse.tool);
    fs.mkdirSync(dir, { recursive: true });
    await untar(archive, dir, pinToUse.archive);
    fs.rmSync(archive, { force: true });
    // Zip builds carry the binary at the archive root; tarball builds under
    // their root folder. The binary must end up at toolDir/llama-server.
    let binary2 = path.join(dir, exeName);
    if (!fs.existsSync(binary2) && pinToUse.root) {
      const nested = path.join(dir, pinToUse.root, exeName);
      if (fs.existsSync(nested)) {
        fs.copyFileSync(nested, binary2);
      }
    }
    if (process.platform !== "win32") {
      try {
        fs.chmodSync(binary2, 0o755);
      } catch {
        /* best effort */
      }
    }
    emit("ready", pinToUse.size, pinToUse.size, 100);
    log("info", "engine downloaded", `${pinToUse.tool} ${LLAMA_BUILD}`);
    return binary2;
  } catch (e) {
    const stopped = isCancelled(e);
    emit("failed", 0, pinToUse.size, 0, stopped ? "The download was stopped." : String(e?.message ?? e));
    throw e;
  } finally {
    engineJob = null;
  }
}

export const engineDownloadRunning = () => engineJob !== null;

export const cancelEngineDownload = () => {
  if (!engineJob) return false;
  engineJob.abort();
  engineJob = null;
  return true;
};

/** Unpack with the system's tar (built into macOS, Linux and Windows 10+). */
function untar(archive, into, kind) {
  return new Promise((resolve, reject) => {
    // bsdtar (Windows, macOS) reads zip files too; Linux needs the b/gz flags.
    const flag = kind === "tarbz2" ? "-xjf" : kind === "targz" ? "-xzf" : "-xf";
    const tar = process.platform === "win32" && process.env.SystemRoot
      ? path.join(process.env.SystemRoot, "System32", "tar.exe")
      : "tar";
    const child = spawn(tar, [flag, archive, "-C", into], { windowsHide: true });
    let stderr = "";
    child.stderr?.on("data", (c) => (stderr += c.toString()));
    child.on("close", (code) => {
      if (code === 0) resolve();
      else reject(new Error(String(stderr).trim() || `tar exited with ${code}`));
    });
    child.on("error", reject);
  });
}

// ---- Pinned tools (sherpa-onnx, cloudflared): download, verify, unpack --

const exeNameFor = (name) => (process.platform === "win32" ? `${name}.exe` : name);

const toolDirFor = (pin) => path.join(appDir(), "tools", `${pin.tool}-${pin.version}`);

/** The downloaded program for a pinned tool, when it is on this computer. */
export function pinnedBinaryOf(pin, program, subpath = "") {
  const file = path.join(toolDirFor(pin), subpath, exeNameFor(program));
  return fs.existsSync(file) ? file : null;
}

/**
 * Downloads and unpacks one pinned tool. Same staging dance as the Rust
 * build: download → checksum → unpack → flatten the archive's root folder →
 * atomic rename. One install at a time per tool.
 */
const toolJobs = new Map();

export async function ensurePinnedTool(pin, program, subpath = "", { signal, onProgress } = {}) {
  const existing = pinnedBinaryOf(pin, program, subpath);
  if (existing) return existing;
  if (toolJobs.has(pin.tool)) {
    throw AppError.invalidInput(`${pin.tool} is already downloading.`);
  }
  toolJobs.set(pin.tool, true);
  const dir = toolDirFor(pin);
  const parent = path.dirname(dir);
  const fileName = pin.url.split("/").pop() ?? "download";
  const download = path.join(parent, `.${fileName}`);
  try {
    await downloadAtomic(pin.url, download, pin.size, pin.sha256, { signal, onProgress });
    const staging = path.join(parent, `.${pin.tool}-${pin.version}.unpack`);
    fs.rmSync(staging, { recursive: true, force: true });
    fs.mkdirSync(staging, { recursive: true });
    if (pin.archive === "plain") {
      const target = path.join(staging, exeNameFor(pin.tool));
      fs.renameSync(download, target);
      if (process.platform !== "win32") fs.chmodSync(target, 0o755);
    } else {
      await untar(download, staging, pin.archive);
      // Keep the archive's own folder layout (bin and lib), without its top folder.
      let inner = staging;
      if (pin.root) {
        inner = path.join(staging, pin.root);
        const moved = `${staging}.inner`;
        fs.rmSync(moved, { recursive: true, force: true });
        fs.renameSync(inner, moved);
        fs.rmSync(staging, { recursive: true, force: true });
        fs.renameSync(moved, staging);
      }
      fs.rmSync(path.join(staging, "include"), { recursive: true, force: true });
      // Libraries and the program must be runnable.
      for (const sub of ["bin", "lib", ""]) {
        const p = sub ? path.join(staging, sub) : path.join(staging, exeNameFor(program));
        if (fs.existsSync(p) && fs.statSync(p).isDirectory()) {
          for (const entry of fs.readdirSync(p)) {
            try {
              fs.chmodSync(path.join(p, entry), 0o755);
            } catch {
              /* best effort */
            }
          }
        } else if (fs.existsSync(p)) {
          fs.chmodSync(p, 0o755);
        }
      }
    }
    fs.rmSync(dir, { recursive: true, force: true });
    fs.renameSync(staging, dir);
    const binary = pinnedBinaryOf(pin, program, subpath);
    if (!binary) {
      throw AppError.internal(`${pin.tool} was unpacked but its program was not found.`);
    }
    return binary;
  } finally {
    toolJobs.delete(pin.tool);
    fs.rmSync(download, { force: true });
  }
}

// sherpa-onnx v2.0.2: the speech engine, shared libraries, no text-to-speech.
const sherpaPin = (url, sha256, size, root) => ({
  tool: "sherpa-onnx",
  version: "2.0.2",
  url,
  sha256,
  size,
  archive: "tarbz2",
  root,
});

export const SHERPA_PINS = {
  "linux|x64": sherpaPin("https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-linux-x64-shared-no-tts.tar.bz2", "c6725b8994a5e559678f72ddffbf1c14b41fc3f6f650618b98eeb61581b63da2", 27_230_798, "sherpa-onnx-v2.0.2-linux-x64-shared-no-tts"),
  "darwin|arm64": sherpaPin("https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-osx-arm64-shared-no-tts.tar.bz2", "1a803809572335ef545978b62b0271ad119969fef6f9153a94c174c3adc95608", 20_197_239, "sherpa-onnx-v2.0.2-osx-arm64-shared-no-tts"),
  "darwin|x64": sherpaPin("https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-osx-universal2-shared-no-tts.tar.bz2", "282ed64d37b0885505637521686a50c9a9d6171320d01566f87a3190817de4dd", 43_266_230, "sherpa-onnx-v2.0.2-osx-universal2-shared-no-tts"),
  "win32|x64": sherpaPin("https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-win-x64-shared-MT-MinSizeRel-no-tts.tar.bz2", "acfd6923924fd1eccc917e383742758cb972adc5a42ee151044a74032498ee16", 23_636_378, "sherpa-onnx-v2.0.2-win-x64-shared-MT-MinSizeRel-no-tts"),
  "win32|arm64": sherpaPin("https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.2/sherpa-onnx-v2.0.2-win-arm64-shared-MT-Release-no-tts.tar.bz2", "19f8bdf7871e6f57f71cf4e1a7221e499fa49fe9f95eb58c584faae585b9f201", 23_864_664, "sherpa-onnx-v2.0.2-win-arm64-shared-MT-Release-no-tts"),
};

/** The pin for this computer, or null when none ships. */
export const sherpaPinFor = () => SHERPA_PINS[`${process.platform}|${process.arch}`] ?? null;

/** The speech engine binary, when it is on this computer. */
export function sherpaBinary() {
  const pin = sherpaPinFor();
  return pin ? pinnedBinaryOf(pin, "sherpa-onnx-offline", "bin") : null;
}

export async function ensureSherpa({ signal, onProgress } = {}) {
  const pin = sherpaPinFor();
  if (!pin) {
    throw AppError.invalidInput("There is no voice engine build for this computer.");
  }
  return ensurePinnedTool(pin, "sherpa-onnx-offline", "bin", { signal, onProgress });
}

export function sherpaStatus() {
  const pin = sherpaPinFor();
  const binary = sherpaBinary();
  return {
    installed: binary !== null,
    path: binary,
    version: pin?.version ?? null,
    download_bytes: binary || !pin ? null : pin.size,
    available: pin !== null,
  };
}

// cloudflared 2026.11.1: the tunnel binary for Share my assistant. There is
// no ARM64 Windows build; Windows on ARM runs the x64 one under emulation.
const cloudflaredPin = (url, sha256, size, archive, root = "") => ({
  tool: "cloudflared",
  version: "2026.11.1",
  url,
  sha256,
  size,
  archive,
  root,
});

export const CLOUDFLARED_PINS = {
  "linux|x64": cloudflaredPin("https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-linux-amd64", "97bb3b02d23a5980fc9c15a1a270e692da1923f103f920d5b16d4931d5ab55e7", 40_346_301, "plain"),
  "darwin|arm64": cloudflaredPin("https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-darwin-arm64.tgz", "02fe4bcf08f2c09699cff30658228a739a0c47fb75f3653d1b8772a27ade4462", 19_898_712, "targz"),
  "darwin|x64": cloudflaredPin("https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-darwin-amd64.tgz", "5e0403b188dfee71fbd9b995e7dc9910d61b661c52371935ab6475d41185632f", 21_857_340, "targz"),
  win32: cloudflaredPin("https://github.com/cloudflare/cloudflared/releases/download/2026.11.1/cloudflared-windows-amd64.exe", "341deda3db5ec498a0482254d6c38c0b7cf01a30c37bb5dd95d2216d08955e35", 55_483_112, "plain"),
};

export const cloudflaredPinFor = () => CLOUDFLARED_PINS[`${process.platform}|${process.arch}`] ?? CLOUDFLARED_PINS[process.platform] ?? null;

/** The tunnel binary: fetched by LocalAide, or already on the PATH. */
export function cloudflaredBinary() {
  const pin = cloudflaredPinFor();
  const fetched = pin ? pinnedBinaryOf(pin, "cloudflared") : null;
  if (fetched) return fetched;
  const name = exeNameFor("cloudflared");
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    const file = path.join(dir, name);
    try {
      if (fs.existsSync(file)) return file;
    } catch {
      /* keep looking */
    }
  }
  return null;
}

export async function ensureCloudflared({ signal, onProgress } = {}) {
  const pin = cloudflaredPinFor();
  if (!pin) {
    throw AppError.invalidInput("There is no cloudflared build for this computer.");
  }
  return ensurePinnedTool(pin, "cloudflared", "", { signal, onProgress });
}

export function cloudflaredStatus() {
  const pin = cloudflaredPinFor();
  const binary = cloudflaredBinary();
  return {
    installed: binary !== null,
    path: binary,
    version: pin?.version ?? null,
    download_bytes: binary || !pin ? null : pin.size,
    available: pin !== null,
  };
}
