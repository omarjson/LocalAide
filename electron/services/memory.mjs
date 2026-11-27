// Memory: markdown notes under ~/.aiassistant/memory, mirrored into the
// `notes` table for fast lists, plus the model's suggested notes waiting in
// ~/.aiassistant/note_suggestions until the user accepts or dismisses them.
// Ported from services/memory/{notes,suggest,mod}.rs.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { memoryDir, notesSuggestionsDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { all, get, run } from "../core/db.mjs";
import { searchLibrary } from "./library.mjs";

const RESERVED = new Set(["CON", "PRN", "AUX", "NUL"]);

/** Windows reserves these file names whatever the extension. */
function avoidReservedName(stem) {
  const upper = stem.toUpperCase();
  const numberedPort =
    upper.length === 4 &&
    (upper.startsWith("COM") || upper.startsWith("LPT")) &&
    upper.charCodeAt(3) >= 49 && upper.charCodeAt(3) <= 57;
  if (numberedPort || RESERVED.has(upper)) return `${stem}_note`;
  return stem;
}

/** Only a .md file directly inside the notes folder is addressable. */
function ensureNotePath(pathStr) {
  const resolved = path.resolve(pathStr);
  const dir = path.resolve(memoryDir());
  if (path.dirname(resolved) !== dir || !resolved.toLowerCase().endsWith(".md")) {
    throw AppError.invalidInput(`Not a memory note: ${pathStr}`);
  }
  return resolved;
}

const modifiedSecs = (file) => {
  try {
    return Math.floor(fs.statSync(file).mtimeMs / 1000);
  } catch {
    return 0;
  }
};

/** A unique id for a new note row, like the Rust note_{nanos}_{count}. */
function newNoteId() {
  return `note_${Date.now() * 1e6}_${crypto.randomInt(1, 1_000_000)}`;
}

const rowToNote = (row) => ({
  id: row.id,
  path: row.path,
  title: row.title,
  content: row.content,
  tags: JSON.parse(row.tags || "[]"),
  size: row.size,
  modified: row.modified,
  pinned: row.pinned === 1,
});

/** Reads a note file's title and tags from its `# Title` / tags comment. */
function parseNoteFile(raw) {
  const lines = raw.split("\n");
  const title = (lines[0] ?? "").startsWith("# ") ? lines[0].slice(2).trim() : "";
  const tagLine = lines.find((l) => l.startsWith("<!-- tags: ") && l.endsWith(" -->"));
  const tags = tagLine
    ? tagLine.slice(11, -4).split(",").map((t) => t.trim()).filter(Boolean)
    : [];
  return { title, tags };
}

/**
 * Fills the notes table from the folder, for notes written before this
 * table existed (or by hand). Never removes rows.
 */
function syncNotesFromDisk() {
  const dir = memoryDir();
  let entries;
  try {
    entries = fs.readdirSync(dir);
  } catch {
    return;
  }
  for (const name of entries) {
    if (!name.toLowerCase().endsWith(".md")) continue;
    const file = path.join(dir, name);
    const row = get(`SELECT id FROM notes WHERE path = ?`, file);
    if (row) continue;
    let raw = "";
    try {
      raw = fs.readFileSync(file, "utf8");
    } catch {
      continue;
    }
    const { title, tags } = parseNoteFile(raw);
    const content = raw.split("\n").slice(1).join("\n").replace(/^<!--[^>]*-->\n?/, "").trim();
    run(
      `INSERT OR REPLACE INTO notes (id, path, title, content, tags, size, modified, created_at, pinned)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)`,
      newNoteId(), file, title || name.replace(/\.md$/i, ""), content, JSON.stringify(tags),
      Buffer.byteLength(raw), modifiedSecs(file), modifiedSecs(file),
    );
  }
}

/** Writes the markdown file for a note: `# Title`, the tags comment, body. */
function renderNoteFile(title, tags, content) {
  const tagsHeader = tags.length ? `<!-- tags: ${tags.join(", ")} -->\n` : "";
  return `# ${title}\n${tagsHeader}\n${content}\n`;
}

/** Creates a note file named after the title; duplicates get _2, _3… */
function writeNewNoteFile(stem, markdown) {
  const dir = memoryDir();
  fs.mkdirSync(dir, { recursive: true });
  for (let n = 1; n <= 1000; n += 1) {
    const name = n === 1 ? `${stem}.md` : `${stem}_${n}.md`;
    const file = path.join(dir, name);
    try {
      const fd = fs.openSync(file, "wx");
      try {
        fs.writeFileSync(fd, markdown, "utf8");
      } finally {
        fs.closeSync(fd);
      }
      return file;
    } catch (e) {
      if (e?.code === "EEXIST") continue;
      throw AppError.internal(`Failed to create memory note: ${e}`);
    }
  }
  throw AppError.internal("Too many notes share this title. Use a different title.");
}

function safeTitle(title) {
  const safe = String(title ?? "")
    .slice(0, 64)
    .split("")
    .map((c) => (/[\p{L}\p{N}\-_]/u.test(c) ? c : "_"))
    .join("");
  return safe.trim() ? avoidReservedName(safe) : `note_${Math.floor(Date.now() / 1000)}`;
}

export function createNote(title, content, tags) {
  const cleanTags = Array.isArray(tags) ? tags.map(String) : [];
  const stem = safeTitle(title);
  const file = writeNewNoteFile(stem, renderNoteFile(title, cleanTags, content));
  const size = Buffer.byteLength(String(content ?? ""));
  const modified = modifiedSecs(file);
  const id = newNoteId();
  const now = Math.floor(Date.now() / 1000);
  run(
    `INSERT OR REPLACE INTO notes (id, path, title, content, tags, size, modified, created_at, pinned)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)`,
    id, file, String(title ?? ""), String(content ?? ""), JSON.stringify(cleanTags), size, modified, now,
  );
  return { id, path: file, title: String(title ?? ""), content: String(content ?? ""), tags: cleanTags, size, modified, pinned: false };
}

export function updateNote(pathStr, content, tags) {
  const file = ensureNotePath(pathStr);
  if (!fs.existsSync(file)) {
    throw AppError.invalidInput(`Memory note not found: ${pathStr}`);
  }
  const existing = get(`SELECT * FROM notes WHERE path = ?`, file);
  const title = existing?.title ?? path.basename(file, ".md");
  const cleanTags = Array.isArray(tags)
    ? tags.map(String)
    : JSON.parse(existing?.tags ?? "[]");
  fs.writeFileSync(file, renderNoteFile(title, cleanTags, content), "utf8");
  run(
    `INSERT OR REPLACE INTO notes (id, path, title, content, tags, size, modified, created_at, pinned)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    existing?.id ?? newNoteId(), file, title, String(content ?? ""), JSON.stringify(cleanTags),
    Buffer.byteLength(String(content ?? "")), modifiedSecs(file),
    existing?.created_at ?? Math.floor(Date.now() / 1000), existing?.pinned ?? 0,
  );
}

export function readNote(pathStr) {
  const file = ensureNotePath(pathStr);
  try {
    return fs.readFileSync(file, "utf8");
  } catch {
    throw AppError.invalidInput(`Memory note not found: ${pathStr}`);
  }
}

export function deleteNote(pathStr) {
  const file = ensureNotePath(pathStr);
  try {
    fs.unlinkSync(file);
  } catch {
    /* the row still goes */
  }
  const info = run(`DELETE FROM notes WHERE path = ?`, file);
  return info.changes > 0;
}

export function listNotes() {
  syncNotesFromDisk();
  return all(`SELECT * FROM notes ORDER BY modified DESC`).map(rowToNote);
}

export function setPinned(pathStr, pinned) {
  const file = ensureNotePath(pathStr);
  const info = run(`UPDATE notes SET pinned = ? WHERE path = ?`, pinned ? 1 : 0, file);
  if (info.changes === 0) throw AppError.invalidInput(`Memory note not found: ${pathStr}`);
  return rowToNote(get(`SELECT * FROM notes WHERE path = ?`, file));
}

export function forgetAll() {
  let count = 0;
  const dir = memoryDir();
  try {
    for (const name of fs.readdirSync(dir)) {
      const file = path.join(dir, name);
      if (fs.statSync(file).isFile()) {
        fs.unlinkSync(file);
        count += 1;
      }
    }
  } catch {
    /* best effort */
  }
  run(`DELETE FROM notes`);
  return count;
}

/** The pinned notes, in the order the user pinned them. */
function pinnedNotes() {
  return all(`SELECT * FROM notes WHERE pinned = 1 ORDER BY created_at ASC`).map(rowToNote);
}

/**
 * The block that rides in every chat's context: the notes the user pinned.
 * Empty when nothing is pinned.
 */
export function pinnedContext() {
  const notes = pinnedNotes();
  if (notes.length === 0) return "";
  const lines = [
    "Notes the user pinned - always in context; use them when relevant and say you remembered them.",
  ];
  for (const note of notes) {
    lines.push(`- ${note.title}: ${String(note.content ?? "").trim()}`);
  }
  return lines.join("\n");
}

/** Keyword search over the notes, as SearchResultDoc shapes. */
export function searchNotes(query, queryVector) {
  const hits = searchLibrary(query, 20);
  const wanted = String(query ?? "").toLowerCase();
  const notes = listNotes().filter(
    (n) =>
      n.title.toLowerCase().includes(wanted) ||
      String(n.content ?? "").toLowerCase().includes(wanted),
  );
  const byPath = new Map(hits.map((h) => [h.path, h]));
  for (const note of notes) {
    if (!byPath.has(note.path)) {
      const body = String(note.content ?? "");
      const at = body.toLowerCase().indexOf(wanted);
      byPath.set(note.path, {
        path: note.path,
        title: note.title,
        snippet: at >= 0 ? body.slice(Math.max(0, at - 80), at + 160).trim() : body.slice(0, 160).trim(),
        score: 1,
        modified: note.modified,
        size: note.size,
      });
    }
  }
  return [...byPath.values()].slice(0, 20);
}

/**
 * The recall block for a chat: pinned notes always, then the best keyword
 * matches from the library and the notes, capped to a few hundred words.
 */
export function memoryContext(query) {
  const blocks = [];
  const pinned = pinnedContext();
  if (pinned) blocks.push(pinned);
  if (query.trim()) {
    try {
      const hits = searchLibrary(query, 6);
      if (hits.length) {
        blocks.push(
          `Notes and documents that matched "${query}":\n` +
            hits
              .map((h, i) => `[${i + 1}] ${h.title} (${h.path}): ${h.snippet}`)
              .join("\n"),
        );
      }
    } catch {
      /* recall must never break the chat */
    }
  }
  return blocks.join("\n\n");
}

// ---- Suggested notes (the review queue) ------------------------------

/** Only a plain .md file name inside the suggestions folder is accepted. */
function resolveSuggestion(file) {
  if (
    typeof file !== "string" ||
    file.includes("/") ||
    file.includes("\\") ||
    !file.toLowerCase().endsWith(".md") ||
    path.basename(file) !== file
  ) {
    throw AppError.invalidInput(`Not a suggested note: ${file}`);
  }
  return path.join(notesSuggestionsDir(), file);
}

function parseSuggestion(file, raw, modified) {
  const lines = raw.split("\n");
  const title = (lines[0] ?? "").startsWith("# ")
    ? lines[0].slice(2).trim()
    : file.replace(/\.md$/i, "");
  const content = lines.slice(1).join("\n").trim();
  return { file, title: title || file.replace(/\.md$/i, ""), content, modified };
}

/** Saves a suggestion, naming the file after the title like notes do. */
export function suggestNote(title, content) {
  fs.mkdirSync(notesSuggestionsDir(), { recursive: true });
  const safe = String(title ?? "")
    .slice(0, 64)
    .split("")
    .map((c) => (/[\p{L}\p{N}\-_]/u.test(c) ? c : "_"))
    .join("");
  const stem = safe.trim() ? safe : `suggestion_${Math.floor(Date.now() / 1000)}`;
  const markdown = `# ${title}\n\n${String(content ?? "").trim()}\n`;
  for (let n = 1; n <= 1000; n += 1) {
    const name = n === 1 ? `${stem}.md` : `${stem}_${n}.md`;
    const file = path.join(notesSuggestionsDir(), name);
    try {
      const fd = fs.openSync(file, "wx");
      try {
        fs.writeFileSync(fd, markdown, "utf8");
      } finally {
        fs.closeSync(fd);
      }
      return { file: name, title: String(title ?? "").trim(), content: String(content ?? "").trim(), modified: modifiedSecs(file) };
    } catch (e) {
      if (e?.code === "EEXIST") continue;
      throw AppError.internal(`Failed to write the suggestion: ${e}`);
    }
  }
  throw AppError.internal("Too many suggestions share this title. Use a different title.");
}

export function pendingSuggestions() {
  const dir = notesSuggestionsDir();
  let names;
  try {
    names = fs.readdirSync(dir).sort();
  } catch {
    return [];
  }
  const out = [];
  for (const name of names) {
    if (!name.toLowerCase().endsWith(".md")) continue;
    try {
      const raw = fs.readFileSync(path.join(dir, name), "utf8");
      out.push(parseSuggestion(name, raw, modifiedSecs(path.join(dir, name))));
    } catch {
      /* unreadable suggestions are skipped */
    }
  }
  return out;
}

function readSuggestion(file) {
  const file2 = resolveSuggestion(file);
  const raw = fs.readFileSync(file2, "utf8");
  return parseSuggestion(file, raw, 0);
}

/** Accepts a suggestion: it becomes a real note and leaves the queue. */
export function acceptSuggestion(file, tags) {
  const suggestion = readSuggestion(file);
  const note = createNote(suggestion.title, suggestion.content, tags?.length ? tags : null);
  try {
    fs.unlinkSync(resolveSuggestion(file));
  } catch {
    /* the note exists either way */
  }
  return note;
}

export function dismissSuggestion(file) {
  const file2 = resolveSuggestion(file);
  if (fs.existsSync(file2)) {
    fs.unlinkSync(file2);
    return true;
  }
  return false;
}
