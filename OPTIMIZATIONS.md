# Optimizations Summary

> **Status: historical only.** This file describes unwrap-removal work on `src-tauri/src/inference.rs` (and parts of `model_manager.rs` / `indexer.rs`) — code that has since been replaced: the in-process engine was swapped for the llama-server sidecar (ADR-0007) and the legacy tantivy/sled `indexer.rs` was deleted. The file is retained as a record of that refactor; do not treat the file paths below as current.

## Scope
Files reviewed: `src-tauri/src/inference.rs`, `src-tauri/src/model_manager.rs`, `src-tauri/src/indexer.rs`

## Changes

### inference.rs
- Added explicit safety comment to `unsafe impl Send for LlamaHandle` and `unsafe impl Sync for LlamaHandle`.
  - Documents that `LlamaHandle` contains a non-Sync llama.cpp context and is only safe because all access is guarded by `LlamaState.model: Mutex<Option<LlamaHandle>>`. Ensures Mutex guards protect every operation.
- `LlamaState::active_model_path`
  - Replaced `self.model_path.lock().unwrap().clone()` with `lock().ok()?` to avoid panic on poisoned mutex; returns `None` on error.
- `inference_load`
  - Replaced `state.model.lock().unwrap()` and `state.model_path.lock().unwrap()` with `lock().map_err(|e| format!("... mutex poisoned: {e}"))?` returning `Result<String,String>`.
- `chat_stream` thread body
  - Replaced `state_arc.model.lock().unwrap()` with `match lock()` handling. On poison, logs `log::error!` and emits `llm_error` event instead of panicking.
- `summarize` thread body
  - Same poison handling as `chat_stream` for `state_arc.model.lock()`, logs error and emits `summarize_error`.

### model_manager.rs
- `save_state`
  - Removed double lock/unwrap pattern. Single `state.lock().map_err(...)` guard, serialized to JSON with proper error mapping.
- `list_models`
  - Replaced `self.state.lock().unwrap()` with match returning empty vec and `log::error!` on poison.
- `activate`
  - `self.state.lock().map_err(|e| format!("state mutex poisoned: {e}"))?`
- `remove`
  - Same poison mapping for state mutex.
- `is_installed`
  - `lock()` with `ok()` fallback, logs error and returns `false` on poison.
- `cancel_download`
  - `cancels.lock()` with match, logs error and returns early on poison.
- `is_downloading`
  - Safe lock with logging fallback to `false`.
- `download`
  - `cancels.lock().map_err(...)` for insert and removal of cancellation flag.
  - State mutation after successful download uses `state.lock().map_err(...)`.
All functions now return `Result<String,String>` where appropriate or log and recover instead of unwrapping.

### indexer.rs
- `IndexInner::add_docs`
  - Replaced `self.writer.lock().unwrap()` with `lock().map_err(|e| format!("writer mutex poisoned: {e}"))?`.
- `IndexInner::remove_doc`
  - Same poison handling for writer mutex.
Writer commits now propagate mutex poisoning as string errors instead of panicking.

## Rationale
- Eliminates `unwrap()` on `MutexGuard` which would panic on poisoned mutex after a thread panic.
- Errors are now mapped to `Result<String,String>` for Tauri commands or logged for fire-and-forget threads, preserving process stability.
- Safety comment makes the `unsafe impl Send/Sync` auditable and ties it to the Mutex guarding invariant.

## Verification
- All `lock().unwrap()` occurrences in the three files have been removed or replaced with error-aware handling.
- Unsafe impl retained with documented safety invariant; Mutex guards verified to protect all `LlamaHandle` access paths.
