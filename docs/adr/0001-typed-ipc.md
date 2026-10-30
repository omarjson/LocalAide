# ADR-0001: Strongly Typed IPC Boundary via Code-Generated Bindings

## Status
Accepted

## Context
The legacy implementation used untyped `serde_json::Value` across commands in `src-tauri/src/lib.rs` and loose `any` typing in the frontend. This resulted in silent runtime failures, mismatch between Rust return values and TypeScript expectations (such as `memory_search` expecting `r.path` when Rust returned `doc`), and the inability to detect API drift at compile time.

## Decision
1. **No `serde_json::Value` or `any` at the IPC Boundary**: Every Tauri command must take a strongly typed Rust request struct and return a strongly typed Rust response struct.
2. **TypeScript Binding Generation**: Use `tauri-specta` or an equivalent typed export generator to produce TypeScript definitions directly from Rust command signatures into `src/lib/api/bindings.ts`.
3. **Structured `AppError`**: All errors returned across the boundary conform to a unified error shape:
   ```rust
   pub struct AppError {
       pub code: String,        // e.g. "MODEL_NOT_INSTALLED", "CHECKSUM_MISMATCH"
       pub message: String,     // User-friendly message
       pub detail: Option<String>, // Context for diagnostics/logs
       pub retryable: bool,
   }
   ```
4. **Channels over Global Events**: Replace global Tauri event emitters (`stream_id` matching) with `tauri::ipc::Channel<T>` for all asynchronous streaming data (e.g., token streams, download progress, indexing progress).
5. **Unified Frontend Client**: Frontend screens must never call `@tauri-apps/api/core` directly; all calls go through a single typed client (`src/lib/api/client.ts`) that handles error mapping and logging.

## Consequences
- **Positive**: Renaming or mutating a field in Rust will immediately fail the TypeScript build (`svelte-check` / `tsc`).
- **Positive**: Eliminates listener leaks and race conditions caused by global event multiplexing.
- **Negative**: Requires maintaining explicit request/response DTOs for every IPC interaction.
