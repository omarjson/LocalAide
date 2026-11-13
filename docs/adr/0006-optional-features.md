# ADR-0006: Gating and Deferral of Optional Features (Voice, Vision, Cloud, Modular Updates)

## Status
Accepted. Amended 2026-11-13: cloud LLM shipped as the opt-in Online Providers mode (plan §0 in `.kiro/specs/kiroween-2026/plan.md`), with keys in the local config file rather than the OS keychain, and image input shipped through the sidecar's multimodal path (ADR-0007) instead of the Candle vision plan below.

## Context
The legacy application claimed support for:
- Voice (Whisper STT, Piper TTS)
- Vision (Qwen2.5-VL via Candle)
- Cloud LLM integration
- Modular component hot-swapping

In reality, these were stubs or placeholders:
- `voice.rs` took `State<State<VoiceManager>>` (invalid Tauri state nesting) and Whisper/Piper methods returned placeholder errors.
- `vision_identify` was a mock returning dummy tag arrays.
- Cloud LLM in the UI had no backend implementation.
- Modular updates referenced non-existent endpoints and missing source files.
A stub behind a visible control is a defect, not a feature.

## Decision
1. **Remove or cleanly gate out-of-scope features**:
   - All dead UI (microphone button in chat, "Add model", the cloud toggle with no backend, modular-update surfaces, the Candle vision path) was removed from the shipped app.
   - Unused stub crates (`candle-core`, `candle-transformers`, `candle-nn`, `rodio`) stay out of default builds; only the `voice` Cargo feature remains, off by default.
2. **Gates for re-introduction**:
   - **Voice**: only after audio capture, resampling, VAD, a native Whisper runtime, and synthesis pass automated integration tests with zero memory leaks. Deferred — findings in `.kiro/specs/engine-upgrade/tasks.md` Phase 5.
   - **Vision**: shipped differently than planned. Image input runs through llama.cpp's multimodal support in the sidecar (ADR-0007), not through Candle; runtime verification against a real vision model is still outstanding.
   - **Cloud LLM**: shipped as the opt-in Online Providers mode — off by default, user-supplied key stored in the local config file on this machine only, sent only to the configured base URL. (The original gate said "keys in the OS Keychain"; the shipped decision keeps keys in the config file, masked in the UI.)
   - **Modular Updates**: only with signed manifests, atomic swaps, and automated rollback tests (see `docs/MODULAR_UPDATES.md`).
3. **Product truth rule**:
   - If a feature is not tested end-to-end, it must not be visible or claimed in documentation.

## Consequences
- **Positive**: No dead UI, fewer compiler warnings, faster builds.
- **Positive**: Product honesty: every visible control works.
- **Negative**: The feature set is strictly local chat, documents, memory, and safe tools until a gate is passed.
