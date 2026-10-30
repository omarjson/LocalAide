# ADR-0002: Unified Storage Engine with SQLite WAL, FTS5, and Vector Index

## Status
Accepted

## Context
The legacy application utilized a split storage model:
- `tantivy` for inverted index search in `~/.aiassistant/index`
- `sled` embedded KV store in `~/.aiassistant/index/store.sled`
- raw filesystem files in `~/.aiassistant/memory`
- `config.json` for settings
- `lmdb` listed as an unused dependency

This architecture suffered from multiple failure modes:
1. Tantivy initialization panicked on second launch if the directory already contained an index (`Index::create_in_dir`).
2. Sled is unmaintained, prone to file corruption on ungraceful shutdown, and lacks transactional guarantees across multiple tables.
3. No foreign key constraints or atomic transactions existed between document records, chunks, and metadata.
4. Semantic search required deserializing every single document from sled to extract its embedding, resulting in $O(N)$ memory scans.

## Decision
1. **Consolidate to a Single Embedded SQLite Database (`library.db`)**:
   - Enable Write-Ahead Logging (`PRAGMA journal_mode = WAL;`) for concurrent read access.
   - Enforce foreign keys (`PRAGMA foreign_keys = ON;`) and busy timeouts (`PRAGMA busy_timeout = 5000;`).
2. **Schema & Versioned Migrations**:
   - Store all structured entities in SQLite:
     - `documents` (path, title, content_hash, size, mtime, indexed_at)
     - `chunks` (document_id, chunk_index, text, token_count, start_offset, end_offset)
     - `embeddings` (chunk_id, vector_blob / sqlite-vec table)
     - `conversations` & `messages` (chat history, citations, token metrics)
     - `notes` (metadata, tags, backlinks; raw markdown preserved on disk or synchronized)
     - `tool_audit` (timestamp, tool, arguments, approved, result)
   - Maintain a forward-only migration table (`schema_migrations`).
3. **Hybrid Search Architecture**:
   - Keyword search via SQLite **FTS5** using BM25 scoring with porter/unicode61 tokenizers.
   - Vector similarity using embedded vector indexing (`sqlite-vec` or `usearch`).
   - Combine scores using Reciprocal Rank Fusion (RRF).
4. **Retire Tantivy, Sled, and LMDB**:
   - Drop `sled`, `lmdb`, and `tantivy` dependencies to eliminate redundant disk writes and memory footprints.

## Consequences
- **Positive**: Single transactional ACID engine with WAL recovery and no lock poisoning.
- **Positive**: Rapid startup, low memory footprint, and reliable multi-query performance.
- **Positive**: P95 search latency under 50ms at 100k chunks.
- **Negative**: Migration required from existing sled and tantivy files on legacy installations.
