// The local reranker engine: a third llama-server instance started with
// `--rerank`, serving /v1/rerank for the installed reranker model. It
// re-orders the fused search candidates by query-document relevance. A
// rerank failure keeps the fused order. Ported from
// services/library/rerank.rs, sharing the engine-sidecar pattern.

import crypto from "node:crypto";
import fs from "node:fs";
import net from "node:net";
import { AppError } from "../core/errors.mjs";
import { log } from "../core/log.mjs";
import { loadConfig } from "../core/config.mjs";
import { activeBinary } from "./engine.mjs";
import { getEntry, modelFilePath, allEntries } from "./models.mjs";
import { spawn } from "node:child_process";

/** How many top search results the reranker sees as candidates. */
export const RERANK_CANDIDATES = 40;

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

let running = null;

export function rerankStatus() {
  return {
    model_id: running?.modelId ?? loadConfig().rerank_model ?? null,
    ready: Boolean(running),
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

/** The smallest installed reranker model, or the requested one. */
function pickModel(preferred) {
  const entries = allEntries().filter((e) => e.kind === "reranker");
  if (preferred) {
    const wanted = entries.find((e) => e.id === preferred);
    if (wanted) return wanted;
    const entry = getEntry(preferred);
    if (entry?.kind === "reranker") return entry;
    throw AppError.invalidInput("That is not a reranker model.");
  }
  const smallest = entries
    .filter((e) => modelFilePath(e.id) !== null)
    .sort((a, b) => (a.file_size ?? 0) - (b.file_size ?? 0))[0];
  if (smallest) return smallest;
  throw new AppError("RERANK_NO_MODEL", "No reranker model is installed. Download one from the Models screen first.");
}

/** Starts the server on the requested reranker model, or the smallest one. */
export async function startRerankEngine(modelId) {
  if (running && (!modelId || running.modelId === modelId)) return rerankStatus();
  stopRerankEngine();
  const entry = pickModel(modelId);
  const file = modelFilePath(entry.id);
  if (!file) {
    throw new AppError("RERANK_NO_MODEL", "No reranker model is installed. Download one from the Models screen first.");
  }
  const binary = activeBinary();
  if (!binary) {
    throw new AppError("ENGINE_MISSING", "The model engine is not downloaded yet. Open Settings → Performance and download it.");
  }
  const port = await freePort();
  const apiKey = crypto.randomBytes(24).toString("hex");
  const child = spawnQuiet(binary, [
    "-m", file, "--rerank", "--host", "127.0.0.1", "--port", String(port),
  ]);
  child.on("exit", () => {
    if (running?.child === child) running = null;
  });
  running = { child, port, apiKey, modelId: entry.id };
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      running = null;
      throw new AppError("ENGINE_FAILED", "The reranker stopped while loading. See the engine log in Settings → Activity.");
    }
    if (await healthOk(port)) {
      log("info", "reranker ready", `port=${port} model=${entry.id}`);
      if (loadConfig().rerank_model !== entry.id) {
        const { updateConfig } = await import("../core/config.mjs");
        updateConfig((c) => {
          c.rerank_model = entry.id;
        });
      }
      return rerankStatus();
    }
    await new Promise((resolve) => setTimeout(resolve, 400));
  }
  stopRerankEngine();
  throw new AppError("ENGINE_TIMEOUT", "The reranker took too long to load.");
}


export function stopRerankEngine() {
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

/**
 * Score each document against the query. Returns hits sorted by score,
 * descending, one per input document, index-preserving.
 */
export async function rerank(query, documents) {
  if (!running) {
    throw new AppError("RERANK_NOT_READY", "The reranker engine is not running");
  }
  const response = await fetch(`http://127.0.0.1:${running.port}/v1/rerank`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${running.apiKey}`,
    },
    body: JSON.stringify({ model: running.modelId, query, documents }),
    signal: AbortSignal.timeout(60_000),
  });
  if (!response.ok) {
    throw AppError.internal(`Rerank endpoint returned HTTP ${response.status}`);
  }
  const body = await response.json().catch(() => null);
  if (!body || !Array.isArray(body.results)) {
    throw AppError.internal("Rerank response was invalid");
  }
  return body.results
    .filter((r) => typeof r?.index === "number" && typeof r?.score === "number")
    .map((r) => ({ index: r.index, score: r.score }))
    .sort((a, b) => b.score - a.score);
}

/** Reorder docs by rerank hits; unscored documents keep their tail order. */
export function reorder(docs, hits) {
  const seen = new Array(docs.length).fill(false);
  const out = [];
  for (const hit of hits) {
    if (hit.index < docs.length && !seen[hit.index]) {
      seen[hit.index] = true;
      out.push(docs[hit.index]);
    }
  }
  docs.forEach((doc, i) => {
    if (!seen[i]) out.push(doc);
  });
  return out;
}

/**
 * The search-time path: rerank the top candidates when the engine is up,
 * otherwise hand them through unchanged. Never fails a search.
 */
export async function rerankCandidates(query, candidates) {
  if (!running || candidates.length < 2) return candidates;
  try {
    const top = candidates.slice(0, RERANK_CANDIDATES);
    const hits = await rerank(query, top.map((c) => `${c.title}\n${c.snippet}`));
    const ordered = reorder(top, hits);
    return [...ordered, ...candidates.slice(RERANK_CANDIDATES)];
  } catch (e) {
    log("warn", "rerank failed, keeping the fused order", String(e?.message ?? e));
    return candidates;
  }
}

export const shutdownRerankEngine = () => stopRerankEngine();
