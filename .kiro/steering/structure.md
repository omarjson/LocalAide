---
inclusion: always
---

# LocalAide: Repo Structure

## Top level

- `src/`: Svelte 5 frontend.
- `src-tauri/`: Rust core and Tauri config.
- `vendor/`: vendored `llama_cpp_rs` (patched in via `[patch.crates-io]`).
- `docs/`: scope, backlog, ADRs, evidence.
- `scripts/`: smoke test (`smoke-test.mjs`), packaging helpers.
- `.kiro/`: Kiro project files. Currently holds `skills/impeccable/` and this `steering/` folder. No specs or hooks yet.
- `README.md`, `ARCHITECTURE.md`, `docs/scope.md`: product and design docs. Some claims in them do not match the code. See `product.md` and `tech.md`.

## Frontend (`src/`)

- `main.ts`, `App.svelte`: entry point, global keyboard handling, screen routing.
- `main.css`: Tailwind v4 entry.
- `screens/`: one file per screen. `ChatScreen`, `SearchScreen`, `MemoryScreen`, `ModelManagerScreen`, `SettingsScreen`, `SetupWizard`, `DevConfigScreen`.
- `components/`: shared pieces. `ChatInput`, `MessageBubble`, `ToolApprovalModal`.
- `lib/api/bindings.ts`: typed DTOs that mirror the Rust domain types.
- `lib/api/client.ts`: the typed IPC client. Screens call the backend through it.
- `lib/types.ts`, `lib/format.ts`: shared types and formatting helpers.

Put new Svelte screens in `src/screens/`, reusable UI in `src/components/`, and every backend call through `src/lib/api/client.ts`.

## Backend (`src-tauri/src/`)

- `app/`: `state.rs` (`AppState`, the single state owner), `paths.rs` (all `~/.aiassistant/` paths), `error.rs` (`AppError`), `jobs.rs` (job registry with cancel flags), `events.rs`.
- `commands/`: Tauri command handlers, one file per area: `chat`, `library`, `memory`, `models`, `settings`, `system`, `tools`. Keep them thin. Each one takes a DTO and calls a service.
- `domain/`: DTO and domain types shared with the frontend: `chat`, `document`, `memory`, `model`, `settings`, `system`, `tool`.
- `services/`:
  - `inference/`: `llama.rs` (worker thread), `context.rs` (token budget), `prompt.rs`.
  - `library/`: `db.rs` (SQLite schema and queries), `ingest.rs` (chunking), `search.rs` (keyword, vector and RRF), `watch.rs`, `mod.rs` (`LibraryService`).
  - `memory/`: `notes.rs` (markdown notes on disk), `rag.rs` (recall and citations), `mod.rs`.
  - `tools/`: `sandbox.rs` (`WorkspaceJail`), `approval.rs`, `mod.rs` (`ToolService`, audit).
  - `models/`: `catalog.rs` (built-in catalog and manifest check), `download.rs` (resumable downloads), `verify.rs` (SHA-256), `mod.rs`.
- Top-level modules `indexer.rs`, `watcher.rs`, `embeddings.rs`, `voice.rs`, `model_manager.rs`, `components.rs`, `config.rs`, `tools.rs`: older code or wiring. Check how a module is used before you change it. `model_manager.rs` still lists vision and voice entries that v1.0 excludes.

## Tests (`src-tauri/tests/`)

- `contract_test.rs`, `fault_injection_test.rs`, `inference_test.rs`, `library_test.rs`, `memory_rag_test.rs`, `tools_sandbox_test.rs`.
- Run them with `cd src-tauri && cargo test --all-targets`.

## Placement rules

- Add a new backend feature as a service in `services/`, a command in `commands/`, and DTOs in `domain/`.
- Keep UI controls wired end to end. Do not add a button without its command and handler.
- New tool actions go through `WorkspaceJail` and the approval flow, and get an audit row.
- Keep decisions in `docs/adr/`. Keep evidence in `docs/evidence/`.
