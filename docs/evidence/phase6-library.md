# Phase 6 Verification: Document Library & Retrieval Engine

## Completed Deliverables

1. **Embedded SQLite WAL Engine (`src-tauri/src/services/library/db.rs`):**
   - Consolidated storage from legacy Tantivy/Sled into a single ACID SQLite database (`library.db`).
   - Configured with `PRAGMA journal_mode = WAL;`, `PRAGMA synchronous = NORMAL;`, `PRAGMA foreign_keys = ON;`, and `PRAGMA busy_timeout = 5000;`.
   - Forward-only versioned schema migrations creating `documents`, `chunks`, foreign-key cascade relationships, and the `chunks_fts` virtual table.

2. **Multi-Format Ingestion Pipeline (`src-tauri/src/services/library/ingest.rs`):**
   - Natural boundary chunking: splits by paragraphs (`\n\n`), sentences, and words with 1500-char target and 250-char overlap.
   - Text & code extractor: Markdown (`.md`), plain text (`.txt`, `.log`), source code (`.rs`, `.py`, `.js`, `.ts`, `.svelte`, `.c`, `.cpp`, `.go`), configuration files (`.json`, `.csv`, `.toml`, `.yaml`).
   - Pure PDF text extraction with `FlateDecode` stream decompression via `flate2` and text operator parsing (`Tj`, `TJ`).
   - Robustness guards: binary files (null-byte inspection in initial 1024 bytes) and empty files (0 bytes) are rejected gracefully without panicking.
   - SHA-256 content hashing to deduplicate and detect modified files.

3. **FTS5 BM25 Keyword Search:**
   - Full-text search over `chunks_fts` using `porter unicode61` tokenization.
   - Dynamic snippet extraction (`snippet(chunks_fts, 3, '<mark>', '</mark>', '...', 20)`) with BM25 relevance scoring.
   - Query sanitization preventing FTS syntax injection or runtime syntax errors.

4. **Vector Similarity & Reciprocal Rank Fusion (`src-tauri/src/services/library/search.rs`)** — *designed and unit-tested, not wired into production:*
   - The code paths exist: endian-explicit float BLOB serialization, cosine similarity, and RRF with constant $k = 60$.
   - Honesty note: chunk embeddings are never written and no production query passes a vector, so the fusion path never runs. Production search is BM25 keyword only (see `system.rs` and [ADR-0002](../adr/0002-storage.md)). The test below validates the math with synthetic vectors.

5. **Bounded File Watcher (`src-tauri/src/services/library/watch.rs`):**
   - Recursive folder watching using `notify`.
   - 500ms debounce batching window to prevent filesystem thrashing during rapid edits.
   - Ignores `.git`, `node_modules`, `target`, `dist`, `vendor`, and hidden files.
   - Automatically re-indexes modified files and cleans deleted files from the database.

6. **Frontend Experience (`src/screens/SearchScreen.svelte`):**
   - Real-time hybrid/keyword/semantic mode selection.
   - Search latency timer (displays ms duration).
   - Local directory indexing and live watching status.
   - Document count and size statistics header.
   - Keyboard shortcuts: Enter to search, Esc to clear.

## Automated Verification

All 5 integration tests pass in `src-tauri/tests/library_test.rs`:
- `test_sqlite_library_db_crud_and_fts`: verifies SQLite creation, chunk insertion, FTS5 BM25 search, stats calculation, and cascading delete.
- `test_text_chunking_with_overlap_and_boundaries`: verifies chunk boundaries and overlap preservation.
- `test_robustness_against_empty_and_binary_files`: confirms empty files and null-byte binary files return clean `Err` results without crashing.
- `test_hybrid_search_reciprocal_rank_fusion`: validates the BM25 + cosine vector fusion math and the latency threshold on synthetic vectors; the production search path never supplies a vector, so it exercises keyword only.
- `test_folder_recursive_crawler_and_ignore_patterns`: verifies recursive crawler indexes valid files and ignores `.git`/`node_modules`.
