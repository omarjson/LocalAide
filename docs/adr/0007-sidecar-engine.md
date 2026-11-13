# ADR-0007: llama-server sidecar for chat

Status: accepted, 2026-11-13. Supersedes ADR-0003.

## Context

- The vendored llama.cpp in `vendor/llama_cpp_rs` is a 2023 snapshot. It loads only LLaMA, Falcon, GPT-2, GPT-NeoX, MPT, StarCoder, Baichuan, and Refact.
- None of the catalog models could load on it. The catalog includes Qwen3.5, Qwen3.6, Gemma 4, Granite 4, LFM2.5, and MiniCPM5.
- llama.cpp v0.9.0 (released 2026-11-04) supports those architectures, the multimodal library (mtmd) for images, and cohere2moe for North Mini Code.
- The `llama-cpp-2` crate bundles an older llama.cpp than v0.9.0, so it does not meet the version requirement.
- Writing an in-process FFI binding to the new C API cannot be compiled or tested without a build on the target machine.

## Decision

Chat runs in a `llama-server` child process built from llama.cpp tag v0.9.0.

- `SidecarEngine` (`services/inference/sidecar.rs`) implements the existing `InferenceEngine` trait.
- It starts the server on 127.0.0.1, on a free port, with a random 256-bit API key.
- It passes `--mmproj` when `{id}.mmproj.gguf` sits next to the model.
- It waits for `/health`, then streams from `/v1/chat/completions`.
- It sends the message list, so the server applies the model's chat template.
- Cancel closes the HTTP connection. The server stops generating.
- Speech is handled separately. See `.kiro/specs/engine-upgrade/tasks.md`, Phase 5.

The sidecar binary is built by `.github/workflows/llama-server.yml` for macOS (Apple Silicon and Intel), Windows x64, and Linux x64. The release job bundles it through `src-tauri/tauri.sidecar.conf.json`.

## Consequences

- Positive: no FFI layer to maintain. The engine version is pinned in CI. The three platform builds share one recipe.
- Positive: image input uses the server's own multimodal support (`mmproj`).
- Negative: one extra process per loaded model. Startup takes time, and crashes need recovery. The server log is in `~/.aiassistant/logs/llama-server.log`.
- Negative: the prompt is a message list rather than a raw string. Earlier turns are trimmed by the context budget.
- Risk: the sidecar compiles in CI for Windows and Linux; the macOS sidecar builds were cancelled twice to save runner cost, so macOS bundling is unverified, and no platform has had a recorded real-model chat yet. Run one real chat per platform before release.
- Follow-up: done — `vendor/llama_cpp_rs` is deleted.
