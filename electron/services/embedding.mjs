// The embedding ("meaning") engine: a second llama-server instance running
// an embedding GGUF with --embedding, so search can understand similar
// wording. Vectors are float32 blobs on each chunk row. Ported from
// services/library/embedding.rs and the engine sidecar pattern.

import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import { AppError } from "../core/errors.mjs";
import { log } from "../core/log.mjs";
import { loadConfig, updateConfig } from "../core/config.mjs";
import { getEntry, modelFilePath } from "./models.mjs";
import { activeBinary } from "./engine.mjs";
import { chunkCoverage, missingEmbeddings, storeEmbedding, embeddedChunks } from "./indexer.mjs";

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

/** The running embedding sidecar, when one is up. */
let running = null;

export function embeddingStatus() {
  const coverage = chunkCoverage();
  return {
    engine: running ? "llama-server" : "not-running",
    model_id: running?.modelId ?? loadConfig().embedding_model ?? null,
    dimension: running?.dimension ?? null,
    ready: Boolean(running),
    total_chunks: coverage.total_chunks,
    embedded_chunks: coverage.embedded_chunks,
  };
}

const healthOk = async (port, timeoutMs = 800) => {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/health`, { signal: AbortSignal.timeout(timeoutMs) });
    return response.ok;
  } catch {
    return false;
  }
};

/** Starts the sidecar with an installed embedding model. */
export async function startEmbeddingEngine(modelId) {
  const cfg = loadConfig();
  const id = modelId ?? cfg.embedding_model;
  if (!id) {
    throw AppError.invalidInput("Choose an embedding model first — the Search screen lists them.");
  }
  const entry = getEntry(id);
  if (!entry || entry.kind !== "embedding") {
    throw AppError.invalidInput("That is not an embedding model.");
  }
  const file = modelFilePath(id);
  if (!file) {
    throw AppError.notFound("That embedding model is not downloaded yet. Install it on the Models screen first.");
  }
  if (running?.modelId === id) return embeddingStatus();
  stopEmbeddingEngine();

  const binary = activeBinary();
  if (!binary) {
    throw new AppError("ENGINE_MISSING", "The model engine is not downloaded yet. Open Settings → Performance and download it.");
  }
  const port = await freePort();
  const apiKey = crypto.randomBytes(24).toString("hex");
  const child = spawn(
    binary,
    ["-m", file, "--embedding", "--host", "127.0.0.1", "--port", String(port)],
    { cwd: undefined, env: { ...process.env, LLAMA_API_KEY: apiKey }, stdio: ["ignore", "pipe", "pipe"], windowsHide: true },
  );
  child.on("exit", () => {
    if (running?.child === child) running = null;
  });
  running = { child, port, apiKey, modelId: id, dimension: null };

  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      running = null;
      throw new AppError("ENGINE_FAILED", "The embedding model stopped while loading. See the engine log in Settings → Activity.");
    }
    if (await healthOk(port)) {
      // One probe fixes the dimension, so vector maths can trust the blobs.
      const [vector] = await embedTexts(["dimension probe"]);
      running.dimension = vector?.length ?? null;
      log("info", "embedding engine ready", `port=${port} model=${id} dim=${running.dimension}`);
      if (cfg.embedding_model !== id) {
        updateConfig((c) => {
          c.embedding_model = id;
        });
      }
      return embeddingStatus();
    }
    await new Promise((resolve) => setTimeout(resolve, 400));
  }
  stopEmbeddingEngine();
  throw new AppError("ENGINE_TIMEOUT", "The embedding model took too long to load.");
}

export function stopEmbeddingEngine() {
  if (!running) return;
  const { child } = running;
  running = null;
  try {
    if (process.platform === "win32") {
      spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
    } else {
      child.kill("SIGTERM");
    }
  } catch {
    /* already gone */
  }
}

export const embeddingRunning = () => running !== null;

/** One embeddings call: N texts in, N vectors out. */
export async function embedTexts(texts) {
  if (!running) {
    throw AppError.invalidInput("The meaning model is not running. Start it on the Search screen.");
  }
  const response = await fetch(`http://127.0.0.1:${running.port}/v1/embeddings`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${running.apiKey}`,
    },
    body: JSON.stringify({ model: running.modelId, input: texts }),
    signal: AbortSignal.timeout(60_000),
  });
  if (!response.ok) {
    throw AppError.internal(`The embedding engine answered ${response.status}.`);
  }
  const body = await response.json();
  const vectors = (Array.isArray(body?.data) ? body.data : [])
    .sort((a, b) => (a.index ?? 0) - (b.index ?? 0))
    .map((d) => d.embedding);
  if (vectors.length !== texts.length) {
    throw AppError.internal("The embedding engine sent the wrong number of vectors back.");
  }
  return vectors;
}

/** Fills in missing vectors, in batches. Returns the coverage after. */
export async function backfillEmbeddings({ batchSize = 64, maxBatches = 20 } = {}) {
  if (!running) {
    throw AppError.invalidInput("The meaning model is not running. Start it on the Search screen.");
  }
  let updated = 0;
  for (let batch = 0; batch < maxBatches; batch += 1) {
    const pending = missingEmbeddings(batchSize);
    if (pending.length === 0) break;
    const vectors = await embedTexts(pending.map((chunk) => chunk.text));
    for (const [i, vector] of vectors.entries()) {
      storeEmbedding(pending[i].id, vector);
      updated += 1;
    }
  }
  return { updated, ...chunkCoverage() };
}

/**
 * Switches the meaning model: remember it, start it, and clear vectors from
 * any other model, because they cannot be compared.
 */
export async function selectEmbeddingModel(modelId) {
  const cfg = loadConfig();
  const previous = cfg.embedding_model;
  if (previous !== modelId) clearEmbeddings();
  updateConfig((c) => {
    c.embedding_model = modelId;
  });
  await startEmbeddingEngine(modelId);
  void backfillEmbeddings().catch((e) => log("warn", "embedding backfill failed", String(e)));
  return embeddingStatus();
}

// ---- Vector search ------------------------------------------------------

function cosine(aBytes, bVector) {
  let dot = 0;
  let normA = 0;
  let normB = 0;
  const n = Math.min(aBytes.length / 4, bVector.length);
  for (let i = 0; i < n; i += 1) {
    const a = aBytes.readFloatLE(i * 4);
    const b = bVector[i];
    dot += a * b;
    normA += a * a;
    normB += b * b;
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

/** The chunks most similar to the query, as search hits. */
export async function vectorSearch(query, limit = 20) {
  if (!running) return [];
  const [queryVector] = await embedTexts([query]);
  const chunks = embeddedChunks();
  const scored = [];
  for (const chunk of chunks) {
    const score = cosine(chunk.embedding, queryVector);
    if (score > 0.25) scored.push({ chunk, score });
  }
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, limit).map(({ chunk, score }) => ({
    path: chunk.doc_path,
    title: "",
    snippet: String(chunk.text ?? "").slice(0, 220),
    score,
    modified: 0,
    size: 0,
  }));
}

/** Called on quit: never leave the second server running behind the app. */
export const shutdownEmbeddingEngine = () => stopEmbeddingEngine();
