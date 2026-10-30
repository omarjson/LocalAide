# ADR-0006: Gating and Deferral of Optional Features (Voice, Vision, Cloud, Modular Updates)

## Status
Accepted

## Context
The legacy application claimed support for:
- Voice (Whisper STT, Piper TTS)
- Vision (Qwen2.5-VL via Candle)
- Cloud LLM integration
- Modular component hot-swapping

In reality, these were stubs or placeholders:
- `voice.rs` took `State<State<VoiceManager>>` (invalid Tauri state nesting) and Whisper/Piper methods returned placeholder errors.
- `vision_identify` was a mock returning dummy tag arrays.
- `Cloud LLM` in the UI had no backend implementation.
- Modular updates referenced non-existent endpoints and missing source files (`src-tauri/src/components/catalog.rs`).
A stub behind a visible control is a defect, not a feature.

## Decision
1. **Remove or Cleanly Gate Out-of-Scope Features for v1.0**:
   - All dead UI buttons (Microphone button in chat, "Add model", "Cloud LLM", "Modular updates", Vision button) must be removed from the default view.
   - Remove unused stub crates (`candle-core`, `candle-transformers`, `candle-nn`, `rodio`) from default release builds.
2. **Definite Gates for Optional Feature Re-introduction (Phase 12)**:
   - **Voice**: Allowed only after implementing audio capture (`cpal`), resampling (`rubato`), VAD, native Whisper runtime, and Piper synthesis with automated integration tests and zero memory leaks.
   - **Vision**: Allowed only when an official multimodal GGUF or ONNX runtime is integrated with fixture-based regression tests.
   - **Cloud LLM**: Allowed only with explicit opt-in, API keys in the OS Keychain, and strict pre-transmission redaction previews.
   - **Modular Updates**: Allowed only with signed manifests, atomic swaps, and automated rollback tests.
3. **Product Truth Rule**:
   - If a feature is not tested end-to-end, it must not be visible or claimed in documentation.

## Consequences
- **Positive**: Eliminates confusing dead UI, removes dozens of compiler warnings, and dramatically speeds up build times.
- **Positive**: Restores product honesty and user trust.
- **Negative**: Narrows feature set strictly to local chat, document search, memory, and safe tools for v1.0.
