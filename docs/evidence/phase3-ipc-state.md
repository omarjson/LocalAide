# Phase 3 Evidence: Typed IPC & Unified App State

## Deliverables & Automated Verification

1. **Strictly Typed IPC Boundary (`src/lib/api/bindings.ts`)**
   - Zero `serde_json::Value` or `any` in IPC signatures.
   - Domain contracts covering system probe, models, presets, chat streaming, library indexing and search, memory notes, tools, cloud providers, config, download progress, and jobs.

2. **Single App State Owner (`src-tauri/src/app/state.rs`)**
   - `AppState` manages `JobRegistry`, `ModelManager`, `ModelService`, the `RouterEngine` (sidecar plus opt-in cloud), `LibraryService`, `MemoryService`, `ToolService`, and `ConfigStore`.
   - Single managed state passed to the Tauri builder.

3. **Standard Machine-Readable Error Taxonomy (`src-tauri/src/app/error.rs`)**
   - Codes: `MODEL_NOT_INSTALLED`, `MODEL_LOAD_FAILED`, `CHECKSUM_MISMATCH`, `SANDBOX_DENIED`, `DB_BUSY`, `CANCELLED`, `INVALID_INPUT`, `STORAGE_ERROR`, `INTERNAL`.
   - Carries structured `code`, `message`, `detail`, and `retryable` flags.

4. **Tauri Channels for Streaming (`src/lib/api/client.ts` & `src-tauri/src/commands/chat.rs`)**
   - Replaced leaky global `listen("llm_stream")` events with dedicated per-invocation `tauri::ipc::Channel<ChatStreamEvent>`.
   - Download progress streams via `tauri::ipc::Channel<DownloadProgressEvent>`.

5. **Single Frontend API Client (`src/lib/api/client.ts`)**
   - All screens route Tauri operations through the typed `api` client. No raw `invoke()` calls in screen components.

6. **Contract Test Suite (`src-tauri/tests/contract_test.rs`)**
   - 7 automated contract tests verify error, system-probe, model-info, chat-event, thread, document/preset, and job-registry serialization and cancellation.

## Acceptance Checklist
- [x] No `serde_json::Value` or `any` in IPC signatures.
- [x] `npm run check` passes with 0 errors and 0 warnings.
- [x] `cargo test` passes all contract tests.
- [x] Cancelling a long-running download or index job sets the atomic cancellation flag.
- [x] Every error returned to the UI has a stable machine-readable code.
