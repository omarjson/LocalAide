# LocalAide Architecture

LocalAide is a local-first desktop AI assistant built on Tauri v2, Rust (edition 2024), Svelte 5, and SQLite. This page describes how the shipped code works; [docs/scope.md](docs/scope.md) describes what is in and out, and the [ADRs](docs/adr/0001-typed-ipc.md) record the decisions.

## 1. High-Level Architecture

```
                  ┌─────────────────────────────────────────┐
                  │          Svelte 5 UI (Runes)            │
                  │   Tailwind 4 • High-Contrast WCAG 2.2   │
                  └────────────────────┬────────────────────┘
                                       │ Typed IPC Client
                                       │ (bindings.ts; dynamic JSON only for tool args & events)
                                       ▼
                  ┌─────────────────────────────────────────┐
                  │              Tauri Core                 │
                  │   Strict CSP • Channel Event Streaming  │
                  └────────────────────┬────────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         ▼                             ▼                             ▼
┌──────────────────┐         ┌───────────────────┐         ┌──────────────────┐
│  Chat & Memory   │         │  LibraryService   │         │   ToolService    │
│  RouterEngine    │         │   SQLite (WAL)    │         │  Workspace Jail  │
│  Token Budgeting │         │   FTS5 BM25       │         │  User Approval   │
│  RAG Citations   │         │   Keyword Search  │         │  Audit Log       │
└────────┬─────────┘         └─────────┬─────────┘         └────────┬─────────┘
         │                             │                            │
         ▼                             ▼                            ▼
┌──────────────────┐         ┌───────────────────┐         ┌──────────────────┐
│  InferenceEngine │         │  Embedded Storage │         │ Host Filesystem  │
│  llama-server    │         │  ~/.aiassistant/  │         │ ~/.aiassistant/  │
│  sidecar / cloud │         │  index/library.db │         │ workspace/       │
└──────────────────┘         └───────────────────┘         └──────────────────┘
```

## 2. Core Service Boundaries

### Single AppState (`src-tauri/src/app/state.rs`)
One managed state owner holds `JobRegistry` (cancel flags, progress), `ModelManager`, `ModelService`, `RouterEngine`, `LibraryService`, `MemoryService`, `ToolService`, and `ConfigStore`. There is no distributed state and no ad-hoc file cache.

### Inference (`src-tauri/src/services/inference/`)
- `RouterEngine` is the one `InferenceEngine` the rest of the app talks to. It forwards to the local sidecar by default and to the remote engine only while cloud mode is on; loading a model always returns to local.
- `SidecarEngine` runs `llama-server` (llama.cpp v0.9.0) as a child process on `127.0.0.1` on a free port with a random 256-bit API key. It streams from `/v1/chat/completions`, passes `--mmproj` for vision models, and is killed on unload, model switch, or exit. See [ADR-0007](docs/adr/0007-sidecar-engine.md).
- `RemoteEngine` (opt-in) streams from the user-configured OpenAI-compatible provider over HTTPS — or over HTTP for a loopback proxy. It is enabled only from Settings with a user-supplied key; the chat header shows a cloud badge while it is active.
- Cancellation is an atomic flag checked per streamed token; Stop or `Esc` halts generation at the next token.
- `ContextBudget` trims the oldest turns when history exceeds the model's context, preserving the system prompt, the RAG block, and the latest user message.

### Document library (`src-tauri/src/services/library/`)
- One transactional store: `~/.aiassistant/index/library.db`, SQLite in WAL mode (`journal_mode = WAL`, `synchronous = NORMAL`, `busy_timeout = 5000`).
- Ingest: natural paragraph/sentence chunking (1500 characters, 250 overlap), pure-Rust PDF text extraction via `flate2`, content hashing for deduplication.
- Search: keyword BM25 over FTS5 only. The vector and RRF code paths exist (`search.rs`) but production queries never pass a query vector, because chunk embeddings are never written. They are kept for when embeddings are wired.

### Memory & RAG (`src-tauri/src/services/memory/`)
- Markdown notes in `~/.aiassistant/memory/` are synchronized with the SQLite `notes` index.
- Relevance gate `MIN_RELEVANCE_SCORE = 0.010` keeps low-scoring chunks out of the prompt; unrelated questions get zero injected context.
- Citations are emitted as `ChatStreamEvent::Citation` payloads with deterministic keys `[1]`, `[2]` before tokens stream, and render as clickable pills with snippet, path, and score.
- "Forget All" unlinks every note file and purges the database rows in one transaction.

### Tools & jail (`src-tauri/src/services/tools/`)
- `WorkspaceJail` is rooted at `~/.aiassistant/workspace`. Paths are checked lexically (`..` depth) and canonically (`std::fs::canonicalize`); outside symlinks and symlink loops are rejected with `SANDBOX_DENIED`.
- Actions are `list_dir`, `read_file`, `write_file`, `search_files`. Reads are auto-approved and bounded (1 MB read, 1000-entry listings); `write_file` (4 MB cap) requires interactive approval with a payload preview, honored once or for the session.
- Every call — approved, denied, or failed — is written to the SQLite `tool_audit` table. The log is user-visible and clearable from Settings.

## 3. Threat Model & Invariants

| Threat | Mitigation |
|---|---|
| Prompt-injection exfiltration | Tools confined to `WorkspaceJail`; absolute paths and outside symlinks rejected with `SANDBOX_DENIED`. |
| Model weight tampering | Catalog integrity digest over ids, sizes, and hashes, checked before any download; streaming SHA-256 verification before atomic rename. |
| XSS | Strict CSP (`default-src 'self' ...`), no remote scripts, typed IPC edges. |
| Data exfiltration | Zero telemetry; with cloud mode off, the chat and search paths make no network requests. Cloud mode is opt-in, sends chat only to the configured provider, and shows a badge while active. |
| Memory exhaustion | Streaming SHA-256 in 64 KB buffers; bounded tool reads and writes; strict context budgeting. |
