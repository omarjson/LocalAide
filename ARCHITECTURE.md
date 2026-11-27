# LocalAide Architecture

LocalAide is a local-first desktop AI assistant built on Electron, Svelte 5, and SQLite. The backend is plain Node ESM under `electron/` — no build step, no bundler, no native modules; SQLite comes from Node's built-in `node:sqlite`. This page describes how the shipped code works; [docs/scope.md](docs/scope.md) describes what is in and out, and the [ADRs](docs/adr/0001-typed-ipc.md) record the decisions.

## 1. High-Level Architecture

```
                  ┌─────────────────────────────────────────┐
                  │          Svelte 5 UI (Runes)            │
                  │   Tailwind 4 • High-Contrast WCAG 2.2   │
                  └────────────────────┬────────────────────┘
                                       │ Typed IPC Client
                                       │ (@tauri-apps/api specifiers aliased
                                       │  to src/lib/platform/tauri.ts)
                                       ▼
                  ┌─────────────────────────────────────────┐
                  │            Electron Main                │
                  │  preload bridge (__LOCALAIDE__) • CSP   │
                  │  localaide-file:// • Channel streaming  │
                  └────────────────────┬────────────────────┘
                                       │ one ipcMain channel ("localaide:invoke")
                                       │ name → handler table (electron/ipc.mjs)
         ┌─────────────────────────────┼─────────────────────────────┐
         ▼                             ▼                             ▼
┌──────────────────┐         ┌───────────────────┐         ┌──────────────────┐
│  Chat & Memory   │         │  Library + Indexer│         │   Tools + MCP    │
│  chat.mjs        │         │  SQLite (WAL)     │         │  Workspace jail  │
│  modes.mjs       │         │  FTS5 keyword +   │         │  Approval modes  │
│  usage.mjs       │         │  float32 vectors  │         │  Audit log       │
└────────┬─────────┘         └─────────┬─────────┘         └────────┬─────────┘
         │                             │                            │
         ▼                             ▼                            ▼
┌──────────────────┐         ┌───────────────────┐         ┌──────────────────┐
│  Engine sidecar  │         │  Embedded Storage │         │ Host Filesystem  │
│  llama-server ×1 │         │  ~/.aiassistant/  │         │ ~/.aiassistant/  │
│  (+ embedding ×1)│         │  index/library.db │         │ workspace/       │
│  or cloud/OpenAI │         │  conversations/   │         │ models/ memory/  │
└──────────────────┘         └───────────────────┘         └──────────────────┘
```

## 2. Core Service Boundaries

### One main process (`electron/`)
`main.mjs` owns the window, the preload bridge, the `localaide-file://` protocol and the CSP headers. `ipc.mjs` is the dispatcher: one `ipcMain` channel, a name → handler table wrapped in `guard()`, which turns every throw into the `{ code, message }` `AppError` shape the UI's `errorMessage()` already maps to plain words. An unported command throws `NOT_IMPLEMENTED` — the UI shows one honest error instead of a blank screen. Long-lived services (`engine`, `embedding`, `mcp`, `download`) shut down on quit.

### Inference (`electron/services/engine.mjs`, `chat.mjs`, `stream.mjs`)
- The local sidecar runs `llama-server` as a child process on `127.0.0.1` on a free port with a random API key. It streams from `/v1/chat/completions`, passes `--mmproj` for vision models, and is killed on unload, model switch, or exit. The binary lives at `~/.aiassistant/tools/<tool>-<build>/llama-server`, where the downloader unpacked it. See [ADR-0007](docs/adr/0007-sidecar-engine.md).
- Online providers (opt-in) stream from the user-configured OpenAI-compatible endpoint over HTTPS — or HTTP for a loopback proxy. The chat header shows a cloud badge while one is active.
- `chat.mjs` runs the tool loop: up to `max_rounds` (16/6/8 for autonomous/strict/other), the four chat modes inject their instruction text (`modes.mjs`, ported verbatim), strict caps temperature at 0.3, and autonomous turns the web on by itself.
- Approval modes: `plan` refuses every write outright; `ask` asks; `smart` auto-approves read-only shell work; `auto` never asks. Approval timers (plan auto-accept, terminal countdown) come from the config, clamped 0..600 s.
- The `Channel` streams 14 event variants (`token`, `citation`, `reasoning`, `tool_call/approval/result`, `question`, `todos`, `subagent_step`, `skill_change`, …) through the preload as `{ index, message }` frames.

### Downloads (`electron/services/download.mjs`)
- Model and engine downloads write to `<name>.part`, resume with an HTTP `Range` request, verify SHA-256 before the atomic rename, and keep the part file on Pause/Cancel so "Resume" appears. A disk-space check refuses early when the model would not fit.
- The llama.cpp engine is pinned per platform/graphics build (CPU, Vulkan, ROCm) with URL + SHA-256 + size, unpacked with the system `tar`, and emitted on the `engine-download` event.
- The model catalog (`catalog/models.json`) is verified by an integrity digest over ids, sizes and hashes, plus structural validation (safe ids, https URLs, `.gguf` names) and an anti-rollback version check; refreshed catalogs are fetched from the project's repository, capped at 512 KB.

### Document library (`electron/services/indexer.mjs`, `library.mjs`, `embedding.mjs`)
- One transactional store: `~/.aiassistant/index/library.db`, SQLite in WAL mode.
- Ingest walks folders to 20,000 files / 32 levels, skipping dot-folders, `node_modules`, `target`, `dist`, `vendor`; reads text and simple PDFs (inflated streams, text operators), refuses binaries and empties with a reason; chunks at natural boundaries (1500 characters, 250 overlap) with a content hash so rescans skip unchanged files.
- `EmbeddingEngine` runs a second `llama-server --embedding` instance for an installed embedding model and serves `/v1/embeddings` locally; chunk vectors are float32 blobs, filled by the backfill command.
- Search has three modes — keyword BM25 over FTS5, semantic cosine over chunk vectors, and hybrid (both, best score per file). Semantic and hybrid quietly fall back to keyword while the embedding engine is off.

### Memory (`electron/services/memory.mjs`)
- Markdown notes in `~/.aiassistant/memory/` are mirrored into the SQLite `notes` table; pinned notes ride in every chat with recall on.
- Suggestions the model offers wait in `~/.aiassistant/note_suggestions/` until the user accepts or dismisses them.
- Recall (`memoryContext`) injects pinned notes and the best keyword matches above the answer, and citations are emitted as `citation` events the answer renders as source chips.

### Tools, MCP and the jail (`electron/services/tools.mjs`, `mcp.mjs`)
- File tools are jailed to `~/.aiassistant/workspace/` — or the chat's own `folder` when one is set. Paths resolve inside the jail; escapes are rejected.
- Reads, listings and searches run without prompts (per the approval mode); writes and terminal commands ask first, with payload previews; every call lands in the SQLite `tool_audit` table.
- MCP servers (stdio programs or Streamable HTTP addresses) connect lazily; their tools are offered to the model as `mcp_<server>_<tool>` specs, executed through the server, with each server's `auto_approve` deciding whether a call asks first.

### Web search (`electron/services/websearch.mjs`)
- The provider table fixes what exists: DuckDuckGo (lite page), Wikipedia REST search, Jina Reader, Exa (API or keyless MCP), Firecrawl. Unknown ids are refused before any request.
- Keyless providers need nothing; keys live in `config.json`, travel only to their own provider, and are excluded from backups unless included.
- Crawl targets are validated against a private-host list, so addresses on the user's machine or network are never sent to an online reader. Responses are capped at 2 MB.

## 3. Threat Model & Invariants

| Threat | Mitigation |
|---|---|
| Prompt-injection exfiltration | Tools confined to the workspace jail; escapes rejected. MCP tools ask unless the server is trusted. |
| Model weight tampering | Catalog integrity digest checked before any download; streaming SHA-256 verification before atomic rename; https-only URLs. |
| XSS | Strict CSP on every `file://` response (`script-src 'self'`, loopback-only `connect-src`), no remote scripts, context isolation on, no node in the page. |
| Data exfiltration | Zero telemetry; with cloud mode off, the chat path makes no network requests. Cloud mode is opt-in, sends chat only to the configured provider, and shows a badge while active. Web search sends queries to the provider the user picked, only on an explicit click, with a 2 MB response cap. |
| Memory exhaustion | Streaming SHA-256 in 64 KB buffers; the 2 MB web-response cap; bounded tool reads and writes; strict context budgeting; the index walk stops at 20,000 files. |
