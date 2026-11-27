// The document indexer: walk a folder, read the text files, split them into
// overlapping chunks, and fill the FTS index. Keyword search then runs over
// the chunks. Limits, ignore rules and chunking mirror services/library/
// ingest.rs, so an existing library behaves the same.

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { AppError } from "../core/errors.mjs";
import { all, get, run, tx } from "../core/db.mjs";

export const MAX_FILE_BYTES = 5 * 1024 * 1024;
export const TARGET_CHUNK_CHARS = 1500;
export const OVERLAP_CHARS = 250;
export const MAX_TEXT_BYTES = 8 * 1024 * 1024;
export const MAX_FOLDER_DEPTH = 32;
export const MAX_FOLDER_FILES = 20_000;

export const SUPPORTED_EXTENSIONS = new Set([
  "md", "markdown", "txt", "text", "log", "rs", "py", "js", "ts", "svelte", "html", "css",
  "json", "csv", "toml", "yaml", "yml", "sh", "sql", "c", "cpp", "h", "go", "pdf",
]);

const IGNORED_DIRS = new Set(["node_modules", "target", "dist", "vendor"]);

const isIgnoredName = (name) => name.startsWith(".") || IGNORED_DIRS.has(name);

export const isSupportedExtension = (file) => SUPPORTED_EXTENSIONS.has(path.extname(file).slice(1).toLowerCase());

/** Reads bytes and says whether they look like text (BOM counts as text). */
function looksBinary(bytes) {
  const probe = bytes.subarray(0, Math.min(8192, bytes.length));
  let suspicious = 0;
  for (const b of probe) {
    if (b === 0) return true;
    if (b < 7 || (b > 13 && b < 32 && b !== 27)) suspicious += 1;
  }
  return suspicious > probe.length * 0.05;
}

function decodeText(bytes) {
  const hasBom =
    (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) ||
    (bytes[0] === 0xff && bytes[1] === 0xfe) ||
    (bytes[0] === 0xfe && bytes[1] === 0xff);
  if (bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder("utf-16le").decode(bytes);
  if (bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder("utf-16be").decode(bytes);
  return new TextDecoder(hasBom ? "utf-8" : "utf-8", { fatal: false }).decode(bytes);
}

/**
 * Minimal PDF text extraction: inflate the content streams and read the
 * text-showing operators. Not a full PDF engine, but it covers the simple
 * text PDFs the library exists for; anything it cannot read is skipped
 * with a reason rather than indexed as garbage.
 */
function pdfText(bytes) {
  const parts = [];
  const raw = bytes.toString("latin1");
  const streamRegex = /stream\r?\n?([\s\S]*?)endstream/g;
  let match;
  while ((match = streamRegex.exec(raw)) !== null) {
    const body = Buffer.from(match[1], "latin1");
    let text = null;
    try {
      text = zlib.inflateSync(body).toString("latin1");
    } catch {
      text = match[1];
    }
    // Tj and TJ operators carry the page's text.
    const tjRegex = /\((?:\\.|[^\\()])*\)\s*Tj|\[(?:[\s\S]*?)\]\s*TJ/g;
    for (const op of text.match(tjRegex) ?? []) {
      const stringRegex = /\((?:\\.|[^\\()])*\)/g;
      for (const literal of op.match(stringRegex) ?? []) {
        const decoded = literal
          .slice(1, -1)
          .replace(/\\([nrtbf()\\])/g, (_, c) => ({ n: "\n", r: "\r", t: "\t", b: "\b", f: "\f" }[c] ?? c))
          .replace(/\\([0-7]{1,3})/g, (_, oct) => String.fromCharCode(parseInt(oct, 8)));
        if (decoded.trim()) parts.push(decoded);
      }
      parts.push("\n");
    }
  }
  return parts.join(" ").replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").trim();
}

/** Splits text into overlapping chunks, cutting at whitespace when it can. */
export function chunkText(text) {
  const chunks = [];
  if (!text) return chunks;
  const normalized = text.replace(/\r\n/g, "\n");
  let start = 0;
  while (start < normalized.length) {
    let end = Math.min(start + TARGET_CHUNK_CHARS, normalized.length);
    if (end < normalized.length) {
      const window = normalized.slice(start, end);
      const cut = Math.max(window.lastIndexOf("\n"), window.lastIndexOf(". "), window.lastIndexOf(" "));
      if (cut > TARGET_CHUNK_CHARS / 2) end = start + cut + 1;
    }
    const piece = normalized.slice(start, end).trim();
    if (piece) chunks.push(piece);
    if (end >= normalized.length) break;
    start = Math.max(end - OVERLAP_CHARS, start + 1);
  }
  return chunks.slice(0, 4000);
}

const docIdFor = (file) => `doc_${createHash("sha256").update(file).digest("hex").slice(0, 16)}`;

const rowToDoc = (row) => ({
  path: row.path,
  title: row.title,
  snippet: String(row.body ?? "").slice(0, 200),
  score: 0,
  modified: row.indexed_at,
  size: row.size_bytes,
});

/** Reads one file into { document, chunks }; throws with a reason on skip. */
export function ingestFile(file) {
  const stat = fs.statSync(file);
  if (!stat.isFile()) throw new Error("not a regular file");
  if (stat.size === 0) throw new Error("the file is empty");
  if (stat.size > MAX_FILE_BYTES) throw new Error("larger than the 5 MB per-file limit");
  const bytes = fs.readFileSync(file);
  if (bytes.length > MAX_FILE_BYTES) throw new Error("larger than the 5 MB per-file limit");
  const isPdf = path.extname(file).toLowerCase() === ".pdf";
  if (!isPdf && looksBinary(bytes)) throw new Error("a binary file");
  let text = isPdf ? pdfText(bytes) : decodeText(bytes);
  if ([...text].length > MAX_TEXT_BYTES) text = [...text].slice(0, MAX_TEXT_BYTES).join("");
  if (!text.trim()) throw new Error("no extractable text");
  const chunks = chunkText(text);
  if (chunks.length === 0) throw new Error("no extractable text");
  return {
    document: {
      path: file,
      title: path.basename(file),
      kind: isPdf ? "pdf" : ["md", "markdown"].includes(path.extname(file).slice(1).toLowerCase()) ? "markdown" : "text",
      size_bytes: stat.size,
      content_hash: createHash("sha256").update(bytes).digest("hex"),
      indexed_at: Math.floor(Date.now() / 1000),
      text,
    },
    chunks,
  };
}

/** Inserts or replaces one document and its FTS + chunk rows, atomically. */
export function saveDocument(document, chunks) {
  return tx(() => {
    run(
      `INSERT OR REPLACE INTO index_docs (path, title, body, size_bytes, indexed_at, content_hash, kind)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
      document.path, document.title, document.text, document.size_bytes, document.indexed_at,
      document.content_hash, document.kind,
    );
    run(`DELETE FROM index_fts WHERE path = ?`, document.path);
    run(`DELETE FROM index_chunks WHERE doc_path = ?`, document.path);
    for (const [index, chunk] of chunks.entries()) {
      run(`INSERT INTO index_fts (path, title, body) VALUES (?, ?, ?)`, document.path, document.title, chunk);
      run(`INSERT INTO index_chunks (doc_path, chunk_index, text) VALUES (?, ?, ?)`, document.path, index, chunk);
    }
  });
}

/** Indexes one file, replacing whatever it had before. */
export function indexFile(file) {
  const { document, chunks } = ingestFile(file);
  saveDocument(document, chunks);
  return {
    path: document.path,
    title: document.title,
    snippet: document.text.slice(0, 200),
    score: 0,
    modified: document.indexed_at,
    size: document.size_bytes,
  };
}

/** Unchanged files skip the write, so a rescan is fast. */
function unchanged(file, contentHash) {
  const row = get(`SELECT content_hash FROM index_docs WHERE path = ?`, file);
  return row && row.content_hash && row.content_hash === contentHash && contentHash !== "";
}

/** Chunk counts for the embedding coverage the Search screen shows. */
export function chunkCoverage() {
  const total = get(`SELECT COUNT(*) AS n FROM index_chunks`)?.n ?? 0;
  const embedded = get(`SELECT COUNT(*) AS n FROM index_chunks WHERE embedding IS NOT NULL`)?.n ?? 0;
  return { total_chunks: total, embedded_chunks: embedded };
}

/** Chunks that still need a vector, oldest first. */
export function missingEmbeddings(limit) {
  return all(`SELECT id, text FROM index_chunks WHERE embedding IS NULL ORDER BY id LIMIT ?`, limit);
}

/** One vector written back, as float32 bytes. */
export function storeEmbedding(id, vector) {
  const bytes = Buffer.alloc(vector.length * 4);
  for (const [i, v] of vector.entries()) bytes.writeFloatLE(v, i * 4);
  run(`UPDATE index_chunks SET embedding = ? WHERE id = ?`, bytes, id);
}

/** Every embedded chunk's vector and text, for vector search. */
export function embeddedChunks(limit = 4000) {
  return all(`SELECT id, doc_path, chunk_index, text, embedding FROM index_chunks WHERE embedding IS NOT NULL ORDER BY id LIMIT ?`, limit);
}

/** Vectors from a different model cannot be compared; a switch clears them. */
export function clearEmbeddings() {
  run(`UPDATE index_chunks SET embedding = NULL`);
}

// ---- The folder walk -----------------------------------------------------

/**
 * Walks a folder to the same limits as the Rust build (20,000 files,
 * 32 levels), indexing what it can read and reporting every skip.
 */
export function indexFolder(folder, { cancel, onEvent } = {}) {
  const started = Date.now();
  const root = path.resolve(folder);
  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
    throw AppError.invalidInput(`Directory does not exist: ${folder}`);
  }

  let seen = 0;
  let indexed = 0;
  let skipped = 0;
  let limited = false;
  const reasons = new Map();
  const skippedFiles = [];

  const skip = (file, reason) => {
    skipped += 1;
    reasons.set(reason, (reasons.get(reason) ?? 0) + 1);
    if (skippedFiles.length < 200) skippedFiles.push({ path: file, reason });
    onEvent?.({ kind: "skipped", file, reason });
  };

  const walk = (dir, depth) => {
    if (cancel?.aborted) return;
    if (depth > MAX_FOLDER_DEPTH) {
      limited = true;
      skip(dir, "nested more than 32 levels deep");
      return;
    }
    if (seen >= MAX_FOLDER_FILES) {
      limited = true;
      return;
    }
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      skip(dir, "This folder could not be opened");
      return;
    }
    for (const entry of entries) {
      if (cancel?.aborted) return;
      if (seen >= MAX_FOLDER_FILES) {
        limited = true;
        return;
      }
      const full = path.join(dir, entry.name);
      if (isIgnoredName(entry.name)) {
        continue;
      }
      if (entry.isDirectory()) {
        walk(full, depth + 1);
        continue;
      }
      if (!entry.isFile()) continue;
      seen += 1;
      if (!isSupportedExtension(full)) continue;
      onEvent?.({ kind: "reading", file: full });
      try {
        const stat = fs.statSync(full);
        if (stat.size === 0) throw new Error("the file is empty");
        if (stat.size > MAX_FILE_BYTES) throw new Error("larger than the 5 MB per-file limit");
        const bytes = fs.readFileSync(full);
        const isPdf = path.extname(full).toLowerCase() === ".pdf";
        if (!isPdf && looksBinary(bytes)) throw new Error("a binary file");
        let text = isPdf ? pdfText(bytes) : decodeText(bytes);
        if ([...text].length > MAX_TEXT_BYTES) text = [...text].slice(0, MAX_TEXT_BYTES).join("");
        if (!text.trim()) throw new Error("no extractable text");
        const hash = createHash("sha256").update(bytes).digest("hex");
        if (unchanged(full, hash)) {
          indexed += 1;
          onEvent?.({ kind: "indexed", file: full });
          continue;
        }
        const chunks = chunkText(text);
        if (chunks.length === 0) throw new Error("no extractable text");
        saveDocument(
          {
            path: full,
            title: entry.name,
            kind: isPdf ? "pdf" : ["md", "markdown"].includes(path.extname(full).slice(1).toLowerCase()) ? "markdown" : "text",
            size_bytes: stat.size,
            content_hash: hash,
            indexed_at: Math.floor(Date.now() / 1000),
            text,
          },
          chunks,
        );
        indexed += 1;
        onEvent?.({ kind: "indexed", file: full });
      } catch (e) {
        skip(full, String(e?.message ?? e));
      }
    }
  };

  walk(root, 0);
  return {
    indexed,
    skipped,
    limited,
    cancelled: cancel?.aborted === true,
    ms: Date.now() - started,
    reasons: [...reasons.entries()].sort((a, b) => b[1] - a[1]),
    skipped_files: skippedFiles,
  };
}

// ---- Folder registry and the watch --------------------------------------

export function saveFolderScan(folder, indexed, skipped, limited) {
  run(
    `INSERT OR REPLACE INTO library_folders (path, added_at, last_scan_at, indexed, skipped, limited)
     VALUES (?, ?, ?, ?, ?, ?)`,
    folder, Math.floor(Date.now() / 1000), Math.floor(Date.now() / 1000), indexed, skipped, limited ? 1 : 0,
  );
}

export function listFolders() {
  return all(`SELECT * FROM library_folders ORDER BY added_at ASC`);
}

// The watcher: one live folder, re-indexing changed files a moment after
// they settle, like the Rust watch.rs.

let watchState = null;

export function startWatch(folder) {
  const root = path.resolve(String(folder).trim());
  if (!fs.existsSync(root)) throw AppError.invalidInput(`There is no folder at ${root}.`);
  stopWatch();
  let timer = null;
  const pending = new Set();
  const watcher = fs.watch(root, { recursive: true }, (_event, name) => {
    if (!name || !isSupportedExtension(name)) return;
    pending.add(path.join(root, name));
    clearTimeout(timer);
    timer = setTimeout(async () => {
      const files = [...pending];
      pending.clear();
      for (const file of files) {
        try {
          if (fs.existsSync(file) && fs.statSync(file).isFile()) indexFile(file);
        } catch {
          /* a half-written file just waits for the next change */
        }
      }
    }, 2000);
  });
  watchState = { root, watcher, timer };
  return { folder: root, status: "watching" };
}

export function stopWatch() {
  if (!watchState) return;
  clearTimeout(watchState.timer);
  watchState.watcher.close();
  watchState = null;
}

export const watchingFolder = () => watchState?.root ?? null;
