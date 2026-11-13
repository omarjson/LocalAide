# ADR-0003: Model Inference Architecture and llama.cpp Runtime

## Status
Superseded by ADR-0007 (sidecar engine)

## Context
The legacy application included a vendored fork of `llama_cpp_rs` (version 0.3.0 in `vendor/llama_cpp_rs`) patched via `[patch.crates-io]`. However:
1. `inference_load` was never invoked from the frontend; model activation in the UI was purely cosmetic.
2. The model lifecycle lacked dedicated worker isolation, causing risk of blocking Tauri's async runtime or crashing the process on FFI faults.
3. Chat streaming relied on global event matching via `stream_id` strings.
4. SAMPLER settings, chat templates, and context window truncation were hardcoded or missing.

## Decision
1. **Maintain Dedicated `InferenceEngine` Trait Abstraction**:
   - Define a clean trait `InferenceEngine` decoupling Tauri commands from backend implementation specifics:
     ```rust
     pub trait InferenceEngine: Send + Sync {
         fn load(&self, model_path: &Path, config: &ModelConfig) -> Result<(), AppError>;
         fn generate(&self, prompt: &str, params: &SamplingParams, cancel: Arc<AtomicBool>, on_token: Box<dyn FnMut(String) -> bool + Send>) -> Result<GenerationMetrics, AppError>;
         fn unload(&self) -> Result<(), AppError>;
         fn active_model(&self) -> Option<LoadedModelInfo>;
     }
     ```
2. **Worker Thread Isolation**:
   - Run all model initialization, FFI calls, and token generation on a dedicated OS worker thread (not inside Tokio async tasks).
   - Wrap FFI boundary invocations with `std::panic::catch_unwind` to prevent native panics from terminating the Tauri host process.
3. **Vendored vs Upstream Crate**:
   - Note (2026-11-13): the vendored `vendor/llama_cpp_rs` fork was deleted after the sidecar swap shipped (ADR-0007); CI now builds `llama-server` from the pinned upstream llama.cpp tag (`.github/workflows/llama-server.yml`).
4. **Context Accounting & Truncation**:
   - Track token usage strictly per message.
   - When the conversation history approaches context limits, perform oldest-message truncation while preserving the system prompt.
5. **Prompt Formatting**:
   - Use standard chat templates (ChatML / Qwen format) conforming to the active GGUF model metadata.

## Consequences
- **Positive**: Complete UI responsiveness during token generation; zero Tokio thread pool starvation.
- **Positive**: Clean model swapping without memory accumulation.
- **Positive**: Immediate cancellation responsiveness via atomic token callback interrupts.
