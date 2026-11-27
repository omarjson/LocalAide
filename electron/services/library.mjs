// Library: the local document index. Keyword search runs over the FTS5
// table; the semantic (meaning) engine is a later port, so search_mode is
// keyword and `reason` says so in plain words, like the Rust build with no
// embedding model running.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { dbPath, appDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { all, get, run, openDb } from "../core/db.mjs";
import { loadConfig } from "../core/config.mjs";
import { allEntries } from "./models.mjs";

/** Wraps a user query so FTS5 operators cannot break the match. */
function ftsQuery(query) {
  const tokens = String(query ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 24);
  return tokens.map((t) => `"${t.replace(/"/g, '""')}"`).join(" ");
}

const SNIPPET_CHARS = 220;

/** Reads a window of the file around the first match, as the snippet. */
function snippetFor(file, query, max = SNIPPET_CHARS) {
  let text = "";
  try {
    text = fs.readFileSync(file, "utf8");
  } catch {
    return "";
  }
  const at = text.toLowerCase().indexOf(String(query).toLowerCase());
  if (at < 0) return text.slice(0, max).replace(/\s+/g, " ").trim();
  const start = Math.max(0, at - 60);
  return text.slice(start, start + max).replace(/\s+/g, " ").trim();
}

/** Keyword search over the indexed documents. `mode` semantic falls back. */
export function searchLibrary(query, limit = 8, mode = "keyword") {
  const q = ftsQuery(query);
  if (!q) return [];
  let rows = [];
  try {
    rows = all(
      `SELECT path, title, body, rank FROM index_fts WHERE index_fts MATCH ? ORDER BY rank LIMIT ?`,
      q, Math.max(1, Math.min(100, limit)),
    );
  } catch {
    rows = [];
  }
  return rows.map((row) => ({
    path: row.path,
    title: row.title || path.basename(row.path),
    snippet: snippetFor(row.path, query) || String(row.body ?? "").slice(0, SNIPPET_CHARS),
    score: 1 + row.rank * -0.01,
    modified: 0,
    size: 0,
  }));
}

/** The Search screen's search command: keyword, semantic, or hybrid. */
export async function searchIndex({ query, mode, limit }) {
  const wanted = Math.max(1, Math.min(100, Math.round(Number(limit) || 30)));
  const { embeddingRunning, vectorSearch } = await import("./embedding.mjs");

  let hits;
  if (mode === "semantic" && embeddingRunning()) {
    hits = await vectorSearch(query, wanted);
  } else if (mode === "hybrid" && embeddingRunning()) {
    // Both lists, interleaved by score; keyword keeps exact-word recall,
    // meaning catches the paraphrases.
    const [keyword, semantic] = await Promise.all([
      searchLibrary(query, wanted, "keyword"),
      vectorSearch(query, wanted),
    ]);
    const byPath = new Map();
    for (const hit of [...keyword, ...semantic]) {
      const existing = byPath.get(hit.path);
      if (!existing || hit.score > existing.score) byPath.set(hit.path, hit);
    }
    hits = [...byPath.values()].sort((a, b) => b.score - a.score).slice(0, wanted);
  } else {
    hits = searchLibrary(query, wanted, "keyword");
  }

  // The reranker re-orders the fused candidates when it is running; a
  // rerank failure keeps the fused order (never fails a search).
  const { rerankCandidates } = await import("./rerank.mjs");
  hits = await rerankCandidates(query, hits);

  // Fill the file facts the UI shows from the docs table when present.
  return hits.map((hit) => {
    const doc = get(`SELECT size_bytes, indexed_at, title FROM index_docs WHERE path = ?`, hit.path);
    return {
      ...hit,
      title: hit.title || doc?.title || path.basename(hit.path),
      modified: doc?.indexed_at ?? hit.modified,
      size: doc?.size_bytes ?? hit.size,
    };
  });
}

/** One page of the library, newest first. */
export function listIndex({ limit, offset }) {
  const keep = Math.max(1, Math.min(500, Math.round(Number(limit) || 200)));
  const skip = Math.max(0, Math.round(Number(offset) || 0));
  return all(
    `SELECT path, title, size_bytes, indexed_at FROM index_docs ORDER BY indexed_at DESC LIMIT ? OFFSET ?`,
    keep, skip,
  ).map((row) => ({
    path: row.path,
    title: row.title || path.basename(row.path),
    snippet: "",
    score: 0,
    modified: row.indexed_at,
    size: row.size_bytes,
  }));
}

export function removeIndexedDoc(pathStr) {
  run(`DELETE FROM index_docs WHERE path = ?`, String(pathStr));
  run(`DELETE FROM index_fts WHERE path = ?`, String(pathStr));
}

export function indexerStats() {
  const row = get(`SELECT COUNT(*) AS n, COALESCE(SUM(size_bytes), 0) AS bytes FROM index_docs`);
  return { doc_count: row?.n ?? 0, total_bytes: row?.bytes ?? 0, storage_path: dbPath() };
}

export function compactLibrary() {
  const before = fs.existsSync(dbPath()) ? fs.statSync(dbPath()).size : 0;
  openDb().exec("VACUUM");
  const after = fs.existsSync(dbPath()) ? fs.statSync(dbPath()).size : 0;
  return { bytes_before: before, bytes_after: after };
}

const EMBEDDING_CHOICES = [
  { id: "embeddinggemma-300m", name: "EmbeddingGemma 300M", size_bytes: 250_000_000 },
  { id: "nomic-embed-v2", name: "Nomic Embed v2", size_bytes: 650_000_000 },
];

/** Everything the Search screen's status card needs. */
export async function libraryStatus() {
  const cfg = loadConfig();
  const { embeddingStatus } = await import("./embedding.mjs");
  const embedding = embeddingStatus();
  const folders = all(`SELECT * FROM library_folders ORDER BY added_at ASC`).map((row) => ({
    path: row.path,
    added_at: row.added_at,
    last_scan_at: row.last_scan_at,
    indexed: row.indexed,
    skipped: row.skipped,
    limited: row.limited === 1,
    documents: get(`SELECT COUNT(*) AS n FROM index_docs WHERE path LIKE ?`, `${row.path}${path.sep}%`)?.n ?? 0,
  }));
  const stats = indexerStats();
  const choices = embeddingChoices();
  const installed = choices.some((c) => c.installed);
  const selected = cfg.embedding_model ?? null;
  return {
    folders,
    documents: stats.doc_count,
    total_bytes: stats.total_bytes,
    last_scan_at: folders.reduce((max, f) => Math.max(max, f.last_scan_at), 0) || null,
    search_mode: embedding.ready ? "semantic" : "keyword",
    reason: embedding.ready
      ? null
      : installed
        ? selected
          ? "The meaning model is not running, so search matches exact words only. Start it again below."
          : "A meaning model is installed but not chosen, so search matches exact words only. Choose it to search by meaning."
        : "No meaning model is installed, so search matches exact words only. Download one to also find related wording.",
    embedding_model: embedding.ready ? embedding.model_id : selected,
    embedding_ready: embedding.ready,
    embedded_chunks: embedding.embedded_chunks,
    total_chunks: embedding.total_chunks,
    choices,
    storage_path: dbPath(),
  };
}

/** Embedding models the user can pick, smallest first, installed from disk. */
function embeddingChoices() {
  const list = allEntries()
    .filter((entry) => entry.kind === "embedding")
    .map((entry) => ({
      id: entry.id,
      name: entry.name,
      // Model files sit flat in models/, like every other model.
      installed: fs.existsSync(path.join(appDir(), "models", `${entry.id}.gguf`)) ||
        fs.existsSync(path.join(appDir(), "models", entry.filename ?? "")),
      size_bytes: entry.file_size,
    }))
    .sort((a, b) => a.size_bytes - b.size_bytes);
  return list.length ? list : EMBEDDING_CHOICES.map((c) => ({ ...c, installed: false }));
}

/** The Documents/Desktop/Downloads shortcuts, as the Rust presets. */
export function libraryFolderPresets() {
  const home = os.homedir();
  const added = new Set(all(`SELECT path FROM library_folders`).map((r) => r.path));
  const preset = (id, label, dir) => {
    const full = dir ? path.join(dir) : null;
    return {
      id,
      label,
      path: full,
      exists: full ? fs.existsSync(full) : false,
      added: full ? added.has(full) : false,
    };
  };
  return [
    preset("documents", "Documents", path.join(home, "Documents")),
    preset("desktop", "Desktop", path.join(home, "Desktop")),
    preset("downloads", "Downloads", path.join(home, "Downloads")),
  ];
}

export function libraryFolderRemove(pathStr) {
  const row = get(`SELECT path FROM library_folders WHERE path = ?`, String(pathStr));
  if (!row) throw AppError.invalidInput("That folder is not in the library.");
  run(`DELETE FROM library_folders WHERE path = ?`, String(pathStr));
  return removeUnder(String(pathStr));
}

/** Removes every document under a folder; returns how many went. */
function removeUnder(folder) {
  const prefix = folder.endsWith(path.sep) ? folder : folder + path.sep;
  const docs = all(`SELECT path FROM index_docs WHERE path = ? OR path LIKE ?`, folder, `${prefix}%`);
  run(`DELETE FROM index_docs WHERE path = ? OR path LIKE ?`, folder, `${prefix}%`);
  run(`DELETE FROM index_fts WHERE path = ? OR path LIKE ?`, folder, `${prefix}%`);
  return docs.length;
}
