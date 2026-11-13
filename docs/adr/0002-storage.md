# ADR-0002: Unified Storage Engine with SQLite WAL, FTS5, and Vector Index

## Status
Accepted. The vector half is designed but not wired: see the honesty note in Decision 3.

## Context
The legacy application used a split storage model:
- `tantivy` for inverted index search in `~/.aiassistant/index`
- `sled` embedded KV store in `~/.aiassistant/index/store.sled`
- raw filesystem files in `~/.aiassistant/memory`
- `config.json` for settings
- `lmdb` listed as an unused dependency

This architecture had multiple failure modes:
1. Tantivy initialization panicked on second launch if the directory already contained an index (`Index::create_in_dir`).
2. Sled is unmaintained, prone to file corruption on ungraceful shutdown, and lacks transactional guarantees across tables.
3. No foreign key constraints or atomic transactions existed between document records, chunks, and metadata.
4. Semantic search required deserializing every document from sled to extract its embedding — an O(N) memory scan.

## Decision
1. **Consolidate to a single embedded SQLite database (`library.db` at `~/.aiassistant/index/`)**:
   - Write-ahead logging (`PRAGMA journal_mode = WAL;`) for concurrent reads.
   - Foreign keys (`PRAGMA foreign_keys = ON;`) and `busy_timeout = 5000`; `synchronous = NORMAL`.
2. **Schema & versioned migrations** (`schema_migrations` table, forward-only):
   - `documents` (path, title, content_hash, size, mtime, indexed_at)
   - `chunks` (document_id, chunk_index, text, token_count, …)
   - `notes` (memory notes index; raw Markdown stays on disk)
   - `tool_audit` (timestamp, tool, action, path, approved, status)
   - `chunks_fts` (FTS5 virtual table over chunks)
   - Chat history is *not* in SQLite: conversations are JSON thread files under `~/.aiassistant/conversations/` with settings in `config.json`.
3. **Hybrid search architecture — keyword shipped, vector not wired**:
   - Keyword search via SQLite FTS5 with BM25 scoring and porter/unicode61 tokenization ships and is the production search path.
   - The vector side (cosine similarity over stored embeddings) and Reciprocal Rank Fusion code exist in `search.rs`, but no embeddings are ever written and no production query passes a vector, so fusion never runs. Wiring real embeddings is the gate for turning it on.
4. **Retire Tantivy, Sled, and LMDB** — the dependencies and their on-disk formats are gone.

## Consequences
- **Positive**: One transactional ACID engine with WAL recovery; no lock poisoning; fast startup and low memory footprint.
- **Positive**: Search, notes, and the audit log share one database with consistent transactions.
- **Negative**: Migration from legacy sled/tantivy files is not provided; a legacy install re-indexes.
- **Honesty note**: earlier revisions of this ADR promised "P95 search under 50 ms at 100k chunks"; no benchmark run backs that number and it has been removed. Measured figures, where they exist, live in [docs/evidence/benchmarks.md](../evidence/benchmarks.md).
