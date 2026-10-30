# Phase 3 Evidence: Typed IPC & Unified App State

## Deliverables & Automated Verification

1. **Strictly Typed IPC Boundary (`src/lib/api/bindings.ts`)**
   - Zero `serde_json::Value` or `any` in IPC signatures.
   - Comprehensive domain contracts: `SystemProbeResult`, `ModelInfo`, `ModelPreset`, `ModelVerifyResult`, `ModelSizeResult`, `ChatStreamRequest`, `ChatStreamEvent`, `SummarizeRequest`, `SummarizeResponse`, `IndexFolderResponse`, `SearchResultDoc`, `IndexerStatsResponse`, `WatchFolderResponse`, `MemoryNote`, `ToolExecutionRequest`, `ToolExecutionResponse`, `AppConfigDto`, `DownloadProgressEvent`, `JobInfo`.

2. **Single App State Owner (`src-tauri/src/app/state.rs`)**
   - `AppState` manages `JobRegistry`, `ModelManager`, `LlamaState`, `Indexer`, `ConfigStore`, and `VoiceManager`.
   - Single managed state passed to Tauri builder.

3. **Standard Machine-Readable Error Taxonomy (`src-tauri/src/app/error.rs`)**
   - Codes: `MODEL_NOT_INSTALLED`, `MODEL_LOAD_FAILED`, `CHECKSUM_MISMATCH`, `SANDBOX_DENIED`, `DB_BUSY`, `CANCELLED`, `INVALID_INPUT`, `INTERNAL`.
   - Carries structured `code`, `message`, `detail`, and `retryable` flags.

4. **Tauri Channels for Streaming (`src/lib/api/client.ts` & `src-tauri/src/commands/chat.rs`)**
   - Replaced leaky global `listen("llm_stream")` events with dedicated per-invocation `tauri::ipc::Channel<ChatStreamEvent>`.
   - Download progress streams via `tauri::ipc::Channel<DownloadProgressEvent>`.

5. **Single Frontend API Client (`src/lib/api/client.ts`)**
   - All frontend screens (`ChatScreen`, `ModelManagerScreen`, `MemoryScreen`, `SearchScreen`, `SettingsScreen`, `SetupWizard`, `DevConfigScreen`, `App`) route 100% of Tauri operations through `api`.
   - Zero raw `invoke()` calls remaining in frontend screen components.

6. **Contract Test Suite (`src-tauri/tests/contract_test.rs`)**
   - 6 automated contract integration tests verify:
     - `test_app_error_serialization`
     - `test_system_probe_serialization`
     - `test_model_info_serialization`
     - `test_chat_stream_event_serialization`
     - `test_document_and_preset_serialization`
     - `test_job_registry_cancellation_and_progress`

## Acceptance Checklist
- [x] No `serde_json::Value` or `any` in IPC signatures.
- [x] `npm run check` passes with 0 errors and 0 warnings.
- [x] `cargo test` passes all unit and integration contract tests.
- [x] Cancelling a long-running download or index job sets the atomic cancellation flag.
- [x] Every error returned to the UI has a stable machine-readable code.
