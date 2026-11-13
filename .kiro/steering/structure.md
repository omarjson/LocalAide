---
inclusion: always
---

# LocalAide: Repo Structure

## Top level

- `src/`: Svelte 5 frontend.
- `src-tauri/`: Rust core and Tauri config.
- `vendor/`: deleted. The old vendored `llama_cpp_rs` fork is gone; the `llama-server` sidecar is built from the pinned llama.cpp tag by CI (`.github/workflows/llama-server.yml`).
- `docs/`: scope, backlog, ADRs, evidence.
- `scripts/`: smoke test (`smoke-test.mjs`), packaging helpers, `prepare-sidecar.sh`.
- `.kiro/`: Kiro project files: `skills/impeccable/`, this `steering/` folder, and `specs/` (`engine-upgrade`, `kiroween-2026`).
- `README.md`, `ARCHITECTURE.md`, `docs/scope.md`: product and design docs. They aim to match the code; if you find a mismatch, fix the doc and note it (see `tech.md` — verify before you claim).

## Frontend (`src/`)

- `main.ts`, `App.svelte`: entry point, global keyboard handling, screen routing.
- `main.css`: Tailwind v4 entry.
- `screens/`: one file per screen. `ChatScreen`, `SearchScreen`, `MemoryScreen`, `ModelManagerScreen`, `SettingsScreen`, `SetupWizard`, `DevConfigScreen`.
- `components/`: shared pieces. `ChatInput`, `MessageBubble`, `ToolApprovalModal`, `Icon` (the single hand-rolled inline-SVG icon set).
- `lib/api/bindings.ts`: typed DTOs that mirror the Rust domain types.
- `lib/api/client.ts`: the typed IPC client. Screens call the backend through it.
- `lib/format.ts`: shared formatting helpers.

Put new Svelte screens in `src/screens/`, reusable UI in `src/components/`, and every backend call through `src/lib/api/client.ts`.

## Backend (`src-tauri/src/`)

- `app/`: `state.rs` (`AppState`, the single state owner), `paths.rs` (all `~/.aiassistant/` paths), `error.rs` (`AppError`), `jobs.rs` (job registry with cancel flags), `events.rs`.
- `commands/`: Tauri command handlers, one file per area: `chat`, `cloud`, `library`, `memory`, `models`, `settings`, `system`, `tools`. Keep them thin. Each one takes a DTO and calls a service.
- `domain/`: DTO and domain types shared with the frontend: `chat`, `document`, `memory`, `model`, `settings`, `system`, `tool`.
- `services/`:
  - `inference/`: `mod.rs` (the `InferenceEngine` trait), `sidecar.rs` (local llama-server child process on 127.0.0.1), `remote.rs` (opt-in OpenAI-compatible cloud engine), `router.rs` (routes local vs cloud; cloud is off unless the user enables it), `context.rs` (token budget), `prompt.rs`. The old in-process `llama.rs` worker is gone (ADR-0007). Voice is deferred (ADR-0006); only feature-gated Cargo deps remain, no voice module.
  - `library/`: `db.rs` (SQLite schema and queries), `ingest.rs` (chunking), `search.rs` (BM25 keyword search; the vector and RRF code paths exist but production queries never pass a query vector), `watch.rs` (folder watcher), `mod.rs` (`LibraryService`).
  - `memory/`: `notes.rs` (markdown notes on disk), `rag.rs` (recall and citations), `mod.rs`.
  - `tools/`: `sandbox.rs` (`WorkspaceJail`), `approval.rs`, `mod.rs` (`ToolService`, audit).
  - `models/`: `catalog.rs` (built-in catalog and manifest check), `download.rs` (resumable downloads), `verify.rs` (SHA-256), `mod.rs`.
- Top-level modules `model_manager.rs` (installed-model registry and download wiring, used by `lib.rs` at startup) and `config.rs` (config load/save). The old `indexer.rs`, `watcher.rs`, `embeddings.rs`, `voice.rs`, `components.rs` and `tools.rs` modules are deleted.

## Tests (`src-tauri/tests/`)

- `contract_test.rs`, `fault_injection_test.rs`, `inference_test.rs`, `library_test.rs`, `memory_rag_test.rs`, `tools_sandbox_test.rs`.
- Run them with `cd src-tauri && cargo test --all-targets`.

## Placement rules

- Add a new backend feature as a service in `services/`, a command in `commands/`, and DTOs in `domain/`.
- Keep UI controls wired end to end. Do not add a button without its command and handler.
- New tool actions go through `WorkspaceJail` and the approval flow, and get an audit row.
- Keep decisions in `docs/adr/`. Keep evidence in `docs/evidence/`.
