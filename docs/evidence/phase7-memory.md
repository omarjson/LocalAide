# Phase 7 Verification: Memory & RAG with Citations

## Completed Deliverables

1. **Bidirectional Disk & SQLite Memory Store (`src-tauri/src/services/memory/notes.rs`):**
   - User notes are stored as plain Markdown files in `~/.aiassistant/memory/` and indexed in SQLite (`notes` table in `library.db`).
   - Atomic nanosecond note ID generation with atomic counter to prevent collision during rapid note creation.
   - Tag extraction from markdown content (parsing `#tag` tokens) and YAML frontmatter metadata support.
   - Automatic background indexing into the SQLite FTS5 index used by the production keyword search.
   - Complete CRUD operations: create, read, update, delete with disk-file synchronization.

2. **Relevance Gate & Strict Token Budgeting (`src-tauri/src/services/memory/rag.rs`):**
   - Relevance Gate: Only context chunks exceeding `MIN_RELEVANCE_SCORE` (calibrated on standardized RRF scale $1 / (60 + rank + 1)$) are admitted into the prompt context. Unrelated queries receive zero injected context, preventing hallucination pollution.
   - Bounded Token Budget: Limits injected context to a configurable token budget (default 512 tokens / ~2048 characters), sorting candidates by score and greedily packing until the budget is exhausted.
   - Stable Citation Identifiers: Assigns deterministic `[1]`, `[2]`, ... citation keys to injected sources with metadata (`id`, `title`, `source`, `snippet`, `score`).

3. **Discrete Citation IPC Stream (`src-tauri/src/commands/chat.rs`):**
   - RAG recall occurs automatically inside `chat_stream` before token generation commences.
   - Discrete `ChatStreamEvent::Citation` payloads are dispatched over the Tauri channel directly to the frontend prior to streaming tokens.
   - Assistant prompt is injected with context blocks and system instruction to reference source numbers `[1]`, `[2]` when making factual claims.

4. **Zero-Trace "Forget All" Privacy Implementation:**
   - Unlinks all note files on disk in the memory directory.
   - Purges all rows from the `notes` SQLite table.
   - Cascades deletions to the `documents`, `chunks`, and `chunks_fts` tables in `library.db`.
   - Verified zero disk remnants and zero database traces.

5. **Frontend User Experience (`src/screens/MemoryScreen.svelte`, `src/components/MessageBubble.svelte`):**
   - Interactive memory manager with real-time tag filter pills (`#architecture`, `#preferences`, etc.).
   - Note editor with markdown preview and delete confirmations.
   - "Forget All" safety modal with double confirmation.
   - Clickable citation pills `[1] {title}` on assistant message bubbles opening an inspection drawer displaying snippet, source file path, and relevance score.

## Automated Verification

All 4 integration tests pass in `src-tauri/tests/memory_rag_test.rs`:
- `test_memory_crud_and_disk_sync`: validates note creation, markdown file write on disk, SQLite indexing, and tag parsing.
- `test_rag_relevance_gate_and_citation_generation`: verifies relevant queries recall context and citations with score thresholds, while unrelated queries recall zero items.
- `test_rag_strictly_respects_token_budget`: proves that context injection never exceeds the configured token budget even with extensive matching documents.
- `test_memory_forget_all_cleans_disk_and_sqlite`: proves atomic total erasure across disk and database.
