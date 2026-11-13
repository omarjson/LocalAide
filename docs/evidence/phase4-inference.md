# Phase 4 Evidence: Production-Grade Model Lifecycle & Inference

> Note (2026-11-13): the in-process engine described here (llama_cpp_rs token callbacks, dedicated worker thread) was replaced by the llama-server sidecar (ADR-0007). Kept as historical evidence; the `InferenceEngine` trait, context budgeting and prompt formatting below still describe shipped code.

## Deliverables & Automated Verification

1. **`InferenceEngine` Trait (`src-tauri/src/services/inference/mod.rs`)**
   - Clean abstraction defining `load`, `unload`, `generate`, and `status`.
   - Decoupled from Tauri IPC for testability and headless benchmarking.

2. **Dedicated OS Worker Thread (`src-tauri/src/services/inference/llama.rs`)**
   - Inference runs on a dedicated OS thread (`localaide-llama-worker`) instead of the Tauri async runtime.
   - Synchronous `mpsc` command/response channels prevent thread starvation and UI blocking.

3. **Sub-100ms Generation Cancellation**
   - `AtomicBool` cancellation flag checked at every token callback in `llama_cpp_rs`.
   - Returning `false` terminates native evaluation immediately, freeing worker resources.
   - Cancel state registered with `JobRegistry`.

4. **Context Budgeting & Deterministic Truncation (`src-tauri/src/services/inference/context.rs`)**
   - Automatically computes token limits and reserves generation space.
   - Preserves system prompt and latest user turn.
   - Truncates oldest non-system turns when budget is exceeded.
   - Verified by unit tests: `test_context_budget_truncates_oldest_turns` and `test_context_budget_within_limits`.

5. **Prompt Formatting & RAG Context (`src-tauri/src/services/inference/prompt.rs`)**
   - Standard and ChatML formatting with deterministic citation markers (`[1] (Source: ...)`).
   - Verified by unit test: `test_prompt_builder_rag_citations`.

6. **Active Model Persistence & Auto-Reload (`src-tauri/src/lib.rs` & `commands/chat.rs`)**
   - Active model ID persisted to `config.json`.
   - Auto-reloaded on launch with graceful fallback if model file is missing or moved.

## Acceptance Checklist
- [x] Generation can be cancelled mid-stream in <100ms and frees the worker.
- [x] Over-context prompt triggers deterministic truncation strategy, not a crash.
- [x] Restarting the app restores the active model without re-downloading.
- [x] All 11 automated Rust unit and integration tests pass cleanly.
