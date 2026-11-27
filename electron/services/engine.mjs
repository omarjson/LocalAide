// The model engine. llama-server is spawned on 127.0.0.1 with a random
// port and a random API key (so nothing else on the machine can drive it),
// exactly like the Rust sidecar. The binary lives at
// ~/.aiassistant/tools/<tool>-<build>/llama-server, where the downloader
// unpacks it — the same layout the Tauri build used.

import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { appDir, logsDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { log } from "../core/log.mjs";
import { loadConfig } from "../core/config.mjs";
import { engineBinaryOf, ensureEngine, enginePinFor } from "./download.mjs";

/** llama.cpp release LocalAide runs models with. */
export const LLAMA_BUILD = "b12359";

const exeName = process.platform === "win32" ? "llama-server.exe" : "llama-server";

/** The release file for this computer and the chosen graphics build. */
export function enginePin(accel = "cpu") {
  const pinned = enginePinFor(accel) ?? enginePinFor("cpu");
  return {
    url: pinned?.url ?? "",
    file: (pinned?.url ?? "").split("/").pop() ?? "",
    dir: `${pinned?.tool ?? "llama.cpp"}-${LLAMA_BUILD}`,
    size: pinned?.size ?? null,
    sha256: pinned?.sha256 ?? "",
  };
}

/** The build in use: the chosen graphics build, else the CPU one. */
export function activeBinary() {
  const accel = loadConfig().engine?.accel || "cpu";
  if (accel !== "cpu") {
    const flavored = engineBinaryOf(`llama.cpp-${accel}`);
    if (flavored) return flavored;
  }
  return engineBinaryOf("llama.cpp");
}

const binaryPath = () => activeBinary() ?? path.join(appDir(), "tools", `llama.cpp-${LLAMA_BUILD}`, exeName);

export const engineInstalled = () => activeBinary() !== null;

/** A free loopback port, so two engines never collide. */
const freePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });

const newApiKey = () => crypto.randomBytes(24).toString("hex");

/** The running engine, when one is loaded. */
let running = null;
/** Extra llama-server flags from Settings → Engine flags. */
let extraArgs = [];
let logStream = null;

export const engineStatus = () => ({
  loaded: !!running,
  model: running?.model ?? null,
  port: running?.port ?? null,
  context: running?.context ?? null,
  started_at: running?.startedAt ?? null,
});

/** The endpoint a chat request goes to, when a local model is loaded. */
export const localEndpoint = () =>
  running ? { url: `http://127.0.0.1:${running.port}/v1/chat/completions`, bearer: running.apiKey } : null;

export function setExtraArgs(args) {
  extraArgs = Array.isArray(args) ? args.filter((a) => typeof a === "string") : [];
}

// ---- EngineInfo: what the Settings engine picker shows -----------------

/** The CPU line, like the Rust build's cpu string. */
function cpuLine() {
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  return `${arch} · generic`;
}

/** What Settings → Performance → Engine shows about the local engine. */
export function engineInfo() {
  const cfg = loadConfig();
  const accel = cfg.engine?.accel || "cpu";
  const cpuBinary = engineBinaryOf("llama.cpp");
  const accelBinary = accel === "cpu" ? null : engineBinaryOf(`llama.cpp-${accel}`);
  const pinFor = (id) => enginePinFor(id);
  const bytesFor = (id) => {
    const onThisMachine = { cpu: { "windows|x64": 25_097_509, "windows|arm64": 13_455_326, "macos|arm64": 16_907_811, "macos|x64": 12_661_621, "linux|x64": 23_311_084, "linux|arm64": 18_949_862 }, vulkan: { "windows|x64": 38_090_643, "windows|arm64": 26_027_698, "linux|x64": 36_153_377, "linux|arm64": 29_435_221 }, rocm: { "windows|x64": 280_132_437, "linux|x64": 270_816_049 } };
    return onThisMachine[id]?.[`${process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux"}|${process.arch === "arm64" ? "arm64" : "x64"}`] ?? null;
  };
  return {
    installed: engineInstalled(),
    build: LLAMA_BUILD,
    cpu: cpuLine(),
    download_bytes: cpuBinary ? null : bytesFor("cpu"),
    custom: false,
    path: activeBinary(),
    accel,
    accel_installed: accel === "cpu" ? true : accelBinary !== null,
    accel_bytes: accel === "cpu" ? null : bytesFor(accel),
    builds: ["cpu", "vulkan", "rocm"].map((id) => ({
      id,
      bytes: bytesFor(id),
      available: pinFor(id) !== undefined,
    })),
    gpu: gpuCache ?? null,
  };
}

// ---- EngineConfig validation and flag mapping (config/mod.rs) ----------

const CACHE_TYPES = ["f16", "f32", "bf16", "q8_0", "q4_0", "q4_1"];

/** Rejects values llama-server would choke on; blank cache types drop. */
export function validatedEngineConfig(cfg) {
  const out = { ...cfg };
  out.accel = ["vulkan", "rocm"].includes(String(cfg.accel ?? "").trim().toLowerCase())
    ? String(cfg.accel).trim().toLowerCase()
    : "";
  for (const key of ["cache_type_k", "cache_type_v"]) {
    const value = cfg[key] === null || cfg[key] === undefined ? "" : String(cfg[key]).trim().toLowerCase();
    if (!value) out[key] = null;
    else if (CACHE_TYPES.includes(value)) out[key] = value;
    else {
      throw AppError.invalidInput(
        `${value} is not a cache type. Use one of: ${CACHE_TYPES.join(", ")}.`,
      );
    }
  }
  if (String(cfg.extra_args ?? "").length > 2000) {
    throw AppError.invalidInput("Extra engine flags are too long.");
  }
  return out;
}

/** The llama-server command line for the flags the user set, in order. */
export function engineConfigToArgs(cfg) {
  const args = [];
  if (cfg.gpu_layers !== null && cfg.gpu_layers !== undefined) {
    args.push("-ngl", String(cfg.gpu_layers));
  }
  if (cfg.threads !== null && cfg.threads !== undefined && cfg.threads > 0) {
    args.push("-t", String(cfg.threads));
  }
  if (cfg.flash_attn === true || cfg.flash_attn === false) {
    args.push("-fa", cfg.flash_attn ? "on" : "off");
  }
  if (cfg.mmap === true || cfg.mmap === false) {
    args.push(cfg.mmap ? "--mmap" : "--no-mmap");
  }
  if (cfg.mlock === true) args.push("--mlock");
  for (const [flag, value] of [["-ctk", cfg.cache_type_k], ["-ctv", cfg.cache_type_v]]) {
    const v = value?.trim();
    if (v) args.push(flag, v);
  }
  if (cfg.parallel !== null && cfg.parallel !== undefined && cfg.parallel > 0) {
    args.push("-np", String(cfg.parallel));
  }
  if (cfg.batch_size !== null && cfg.batch_size !== undefined && cfg.batch_size > 0) {
    args.push("-b", String(cfg.batch_size));
  }
  if (cfg.rope_freq_scale !== null && cfg.rope_freq_scale !== undefined && cfg.rope_freq_scale > 0) {
    args.push("--rope-freq-scale", String(cfg.rope_freq_scale));
  }
  args.push(...String(cfg.extra_args ?? "").split(/\s+/).filter(Boolean));
  return args;
}

/** The saved flags → the arguments the next model load uses. */
export function setEngineConfig(cfg) {
  const valid = validatedEngineConfig(cfg);
  extraArgs = engineConfigToArgs(valid);
  return valid;
}

// The GPU name is shared with services/probe.mjs, which fills it in.
let gpuCache = null;
export const setGpuName = (name) => {
  gpuCache = name;
};

const healthOk = async (port, timeoutMs = 800) => {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/health`, {
      signal: AbortSignal.timeout(timeoutMs),
    });
    return response.ok;
  } catch {
    return false;
  }
};

/** What llama-server says about the loaded model: template, tools, vision. */
export async function engineProps() {
  if (!running) return null;
  try {
    const response = await fetch(`http://127.0.0.1:${running.port}/props`, {
      headers: { Authorization: `Bearer ${running.apiKey}` },
      signal: AbortSignal.timeout(3000),
    });
    return response.ok ? await response.json() : null;
  } catch {
    return null;
  }
}

/** Starts llama-server with one model file. Resolves once it is healthy. */
export async function loadEngine({ modelPath, context = 8192, mmproj = null }) {
  // A missing build is downloaded first, for the build the user chose (the
  // CPU build is the fallback when no graphics build was picked).
  let binary = activeBinary();
  if (!binary) {
    const accel = loadConfig().engine?.accel || "cpu";
    await ensureEngine(accel);
    binary = activeBinary();
  }
  if (!binary || !fs.existsSync(binary)) {
    throw new AppError("ENGINE_MISSING", "The model engine is not downloaded yet. Open Settings → Performance and download it.");
  }
  if (!fs.existsSync(modelPath)) {
    throw AppError.notFound("That model file is not on this computer any more.");
  }
  unloadEngine();

  const port = await freePort();
  const apiKey = newApiKey();
  const args = [
    "-m", modelPath,
    "--host", "127.0.0.1",
    "--port", String(port),
    "-c", String(context),
    "--jinja",
    ...(mmproj && fs.existsSync(mmproj) ? ["--mmproj", mmproj] : []),
    ...extraArgs,
  ];

  fs.mkdirSync(logsDir(), { recursive: true });
  logStream = fs.createWriteStream(path.join(logsDir(), "engine.log"), { flags: "a" });

  const child = spawn(binary, args, {
    cwd: path.dirname(binary),
    env: { ...process.env, LLAMA_API_KEY: apiKey },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });

  child.stdout.on("data", (chunk) => logStream?.write(chunk));
  child.stderr.on("data", (chunk) => logStream?.write(chunk));
  child.on("exit", (code, signal) => {
    log("warn", "engine exited", `code=${code} signal=${signal}`);
    if (running?.child === child) running = null;
  });

  running = {
    child,
    port,
    apiKey,
    model: path.basename(modelPath),
    context,
    startedAt: Date.now(),
  };

  // Wait for health: a large model can take a while to map its weights.
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      running = null;
      throw new AppError("ENGINE_FAILED", "The model engine stopped while loading. See the engine log in Settings → Activity.");
    }
    if (await healthOk(port)) {
      log("info", "engine ready", `port=${port} model=${path.basename(modelPath)}`);
      return engineStatus();
    }
    await new Promise((resolve) => setTimeout(resolve, 400));
  }
  unloadEngine();
  throw new AppError("ENGINE_TIMEOUT", "The model took too long to load. Try a smaller model or a smaller context.");
}

export function unloadEngine() {
  if (!running) return;
  const { child } = running;
  running = null;
  try {
    if (process.platform === "win32") {
      spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    } else {
      child.kill("SIGTERM");
    }
  } catch (e) {
    log("warn", "could not stop the engine", String(e));
  }
  logStream?.end();
  logStream = null;
}

/** Called on quit: never leave an engine running behind the app. */
export const shutdownEngine = () => unloadEngine();

