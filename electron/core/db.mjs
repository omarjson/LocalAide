// The local database: chat threads, tool audits, usage, memory and the
// document index. Node's built-in SQLite (node:sqlite, Node 22+) replaces
// rusqlite, so there is no native module to compile or ship.

import { DatabaseSync } from "node:sqlite";
import fs from "node:fs";
import path from "node:path";
import { dbPath } from "./paths.mjs";

let db = null;

const SCHEMA = `
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS threads (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  message_count INTEGER NOT NULL DEFAULT 0,
  folder TEXT,
  messages TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS threads_updated ON threads(updated_at DESC);

CREATE TABLE IF NOT EXISTS tool_audits (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp INTEGER NOT NULL,
  tool_name TEXT NOT NULL,
  action TEXT NOT NULL DEFAULT '',
  path TEXT,
  approved INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'ok',
  details TEXT
);
CREATE INDEX IF NOT EXISTS audits_time ON tool_audits(timestamp DESC);

CREATE TABLE IF NOT EXISTS usage (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp INTEGER NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  cost_usd REAL NOT NULL DEFAULT 0,
  thread_id TEXT
);
CREATE INDEX IF NOT EXISTS usage_time ON usage(timestamp DESC);

CREATE TABLE IF NOT EXISTS cloud_audit (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp INTEGER NOT NULL,
  host TEXT NOT NULL,
  base_url TEXT NOT NULL,
  provider_id TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT '',
  summary TEXT NOT NULL DEFAULT '',
  bytes INTEGER NOT NULL DEFAULT 0,
  ok INTEGER NOT NULL DEFAULT 0,
  detail TEXT
);
CREATE INDEX IF NOT EXISTS cloud_audit_time ON cloud_audit(timestamp DESC);

CREATE TABLE IF NOT EXISTS index_docs (
  path TEXT PRIMARY KEY,
  title TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL DEFAULT '',
  size_bytes INTEGER NOT NULL DEFAULT 0,
  indexed_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS index_chunks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  doc_path TEXT NOT NULL,
  chunk_index INTEGER NOT NULL DEFAULT 0,
  text TEXT NOT NULL DEFAULT '',
  embedding BLOB,
  FOREIGN KEY (doc_path) REFERENCES index_docs(path) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS index_chunks_doc ON index_chunks(doc_path);
CREATE INDEX IF NOT EXISTS index_chunks_missing ON index_chunks(doc_path) WHERE embedding IS NULL;
CREATE VIRTUAL TABLE IF NOT EXISTS index_fts USING fts5(
  path UNINDEXED, title, body, tokenize = 'porter unicode61'
);

CREATE TABLE IF NOT EXISTS goals (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL DEFAULT '',
  objective TEXT NOT NULL DEFAULT '',
  budget INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'active',
  created_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS goal_steps (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  goal_id TEXT NOT NULL,
  seq INTEGER NOT NULL,
  progress TEXT NOT NULL,
  created_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS goal_steps_goal ON goal_steps(goal_id, seq);

CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  tags TEXT NOT NULL DEFAULT '[]',
  size INTEGER NOT NULL DEFAULT 0,
  modified INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT 0,
  pinned INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS notes_modified ON notes(modified DESC);

CREATE TABLE IF NOT EXISTS library_folders (
  path TEXT PRIMARY KEY,
  added_at INTEGER NOT NULL,
  last_scan_at INTEGER NOT NULL DEFAULT 0,
  indexed INTEGER NOT NULL DEFAULT 0,
  skipped INTEGER NOT NULL DEFAULT 0,
  limited INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS automations (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL DEFAULT '',
  prompt TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'daily',
  hour INTEGER,
  minute INTEGER,
  interval_minutes INTEGER,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  last_run_at INTEGER
);

CREATE TABLE IF NOT EXISTS automation_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  automation_id TEXT NOT NULL,
  name TEXT NOT NULL DEFAULT '',
  at INTEGER NOT NULL,
  ok INTEGER NOT NULL DEFAULT 1,
  answer TEXT NOT NULL DEFAULT '',
  ms INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS automation_runs_time ON automation_runs(at DESC);

CREATE TABLE IF NOT EXISTS usage_ledger (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at INTEGER NOT NULL,
  provider TEXT NOT NULL DEFAULT '',
  model TEXT NOT NULL DEFAULT '',
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  estimated_cost_usd REAL NOT NULL DEFAULT 0,
  thread_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS usage_ledger_time ON usage_ledger(at DESC);
`;

export function openDb() {
  if (db) return db;
  const file = dbPath();
  fs.mkdirSync(path.dirname(file), { recursive: true });
  db = new DatabaseSync(file);
  db.exec(SCHEMA);
  migrate(db);
  return db;
}

/** Columns and tables added after the first schema shipped; additive only. */
function migrate(database) {
  const columns = (table) =>
    database.prepare(`PRAGMA table_info(${table})`).all().map((c) => c.name);
  const docs = columns("index_docs");
  if (!docs.includes("content_hash")) {
    database.exec("ALTER TABLE index_docs ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''");
  }
  if (!docs.includes("kind")) {
    database.exec("ALTER TABLE index_docs ADD COLUMN kind TEXT NOT NULL DEFAULT 'text'");
  }
  // index_chunks arrived after index_docs/index_fts; create it if missing.
  database.exec(`CREATE TABLE IF NOT EXISTS index_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    doc_path TEXT NOT NULL,
    chunk_index INTEGER NOT NULL DEFAULT 0,
    text TEXT NOT NULL DEFAULT '',
    embedding BLOB,
    FOREIGN KEY (doc_path) REFERENCES index_docs(path) ON DELETE CASCADE
  )`);
  database.exec("CREATE INDEX IF NOT EXISTS index_chunks_doc ON index_chunks(doc_path)");
}

export const closeDb = () => {
  if (db) {
    db.close();
    db = null;
  }
};

/** Runs a statement and returns every row. */
export const all = (sql, ...params) => openDb().prepare(sql).all(...params);

/** Runs a statement and returns the first row, or undefined. */
export const get = (sql, ...params) => openDb().prepare(sql).get(...params);

/** Runs a statement; returns { changes, lastInsertRowid }. */
export const run = (sql, ...params) => openDb().prepare(sql).run(...params);

/** Runs several statements in one transaction. */
export function tx(fn) {
  const database = openDb();
  database.exec("BEGIN");
  try {
    const result = fn(database);
    database.exec("COMMIT");
    return result;
  } catch (e) {
    database.exec("ROLLBACK");
    throw e;
  }
}

