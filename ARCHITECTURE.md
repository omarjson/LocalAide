# LocalAide Architecture & System Design

LocalAide is an open-source, local-first desktop AI assistant built on Tauri v2, Rust, Svelte 5, and SQLite WAL.

## 1. High-Level Target Architecture

```
                  ┌─────────────────────────────────────────┐
                  │          Svelte 5 UI (Runes)            │
                  │   Tailwind 4 • High-Contrast WCAG 2.2   │
                  └────────────────────┬────────────────────┘
                                       │ Generated TypeScript Bindings
                                       │ (Typed DTOs; dynamic JSON only for tool args & events)
                                       ▼
                  ┌─────────────────────────────────────────┐
                  │              Tauri Core                 │
                  │   Strict CSP • Channel Event Streaming  │
                  └────────────────────┬────────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         ▼                             ▼                             ▼
┌──────────────────┐         ┌───────────────────┐         ┌──────────────────┐
│   ChatService    │         │   LibraryService  │         │   ToolService    │
│  SidecarEngine   │         │   SQLite (WAL)    │         │  Workspace Jail  │
│  Token Budgeting │         │   FTS5 BM25       │         │  User Approval   │
│  RAG Citations   │         │   Keyword Search  │         │  Audit Log       │
└────────┬─────────┘         └─────────┬─────────┘         └────────┬─────────┘
         │                             │                            │
         ▼                             ▼                            ▼
┌──────────────────┐         ┌───────────────────┐         ┌──────────────────┐
│  InferenceEngine │         │  Embedded Storage │         │ Host Filesystem  │
│  llama-server    │         │  ~/.aiassistant/  │         │ ~/.aiassistant/  │
│  Next-token Stop │         │  index/library.db │         │ workspace/       │
└──────────────────┘         └───────────────────┘         └──────────────────┘
```

---

## 2. Core Service Boundaries

### 1. Unified AppState (`src-tauri/src/app/state.rs`)
- Manages singleton services across Tauri invocations.
- Zero distributed state or ad-hoc file caches; single state owner.
- Holds `JobRegistry` for concurrent lifecycle tracking, cancel tokens, and progress monitoring.

### 2. Inference & Model Lifecycle (`src-tauri/src/services/inference/`)
- `RouterEngine`: one `InferenceEngine` for the rest of the app. It forwards to the local sidecar by default and to the remote engine only when the user switches on cloud mode in Settings. Loading a model always returns to local.
- `SidecarEngine`: runs `llama-server` (llama.cpp v0.9.0) as a child process on 127.0.0.1 with a random API key. Chat streams from `/v1/chat/completions`. See ADR-0007.
- `RemoteEngine` (opt-in): streams from the user-configured OpenAI-compatible provider over HTTPS — or from a loopback proxy over HTTP. Enabled only from Settings with a user-supplied key; the chat header shows a cloud badge while it is active.
- Cancellation flag checked per streamed token; generation stops at the next token after Stop or `Esc`.
- `ContextBudget`: strict token budgeting preserving system persona while truncating oldest turns.

### 3. Document Library & Retrieval Engine (`src-tauri/src/services/library/`)
- Single transactional ACID store: `library.db` running SQLite in WAL mode (`journal_mode = WAL`, `synchronous = NORMAL`, `busy_timeout = 5000`).
- Multi-format parser: natural paragraph/sentence chunker (1500 chars, 250 overlap), pure PDF stream extractor via `flate2`.
- Search: keyword (BM25) over FTS5 only. Vector search and RRF fusion are not wired in v1.0.

### 4. Memory & RAG Pipeline (`src-tauri/src/services/memory/`)
- Plaintext Markdown files in `~/.aiassistant/memory/` synchronized with SQLite `notes` index.
- Relevance gate: filters out low-similarity chunks (`MIN_RELEVANCE_SCORE`), eliminating hallucination context pollution.
- Discrete citation stream: emits `ChatStreamEvent::Citation` payloads with deterministic keys `[1]`, `[2]` and metadata before token streaming.
- Zero-trace "Forget All": unlinks all disk notes and purges database rows in a single atomic transaction.

### 5. Capability-Rooted Tool Jail & Security (`src-tauri/src/services/tools/`)
- `WorkspaceJail`: rooted at `~/.aiassistant/workspace`.
- Blocks lexical `..` traversal escapes and canonical target escapes.
- Blocks external symlinks and detects symlink loops.
- Explicit approval flow: mutating operations (`write_file`) require interactive approval with preview.
- SQLite audit log (`tool_audit` table) tracking every execution; user-visible and clearable from Settings.

---

## 3. Threat Model & Invariants

| Threat | Mitigation Mechanism |
|---|---|
| **Prompt Injection Exfiltration** | Filesystem tools constrained to `WorkspaceJail`; absolute paths and external symlinks blocked with `SANDBOX_DENIED`. |
| **Model Weight Tampering** | Catalog integrity digest (`CatalogManifest.signature`) over ids, sizes, and hashes; streaming SHA-256 integrity verification before atomic rename. |
| **Cross-Site Scripting (XSS)** | Hardened CSP (`default-src 'self' ...`); no remote CDN scripts; IPC channel isolation. |
| **Data Exfiltration / Snooping** | Zero telemetry stance; with cloud mode off, zero network requests in chat/search paths and fully local inference. Cloud mode is opt-in, sends chat only to the user-configured provider, and shows a badge while active. |
| **Memory Exhaustion (OOM)** | Streaming hashing via 64 KB buffers (< 100 KB RAM); strict turn budgeting in context engine. |
