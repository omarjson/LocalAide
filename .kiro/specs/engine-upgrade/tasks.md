# Engine upgrade to llama.cpp v0.9.0: tasks

Decision (2026-11-13): keep two engine paths.
- Chat runs in a `llama-server` sidecar built from llama.cpp tag v0.9.0.
- Speech runs through a rewritten in-process binding for v0.9.0.

Status legend: DONE = written and checked by reading. TODO = not started. Nothing below has been compiled or run. Compile and test on your machine.

## Why this change

- The vendored llama.cpp in `vendor/llama_cpp_rs` is a 2023 snapshot. It loads only LLaMA, Falcon, GPT-2, GPT-NeoX, MPT, StarCoder, Baichuan, and Refact. None of the 35 catalog models can load.
- The `llama-cpp-2` crate bundles an older llama.cpp than v0.9.0, so it doesn't meet the version requirement.
- v0.9.0 `llama-server` has the flags the sidecar needs: `--host`, `--port`, `--api-key`, `--mmproj`, `--jinja`, and `/health`.

## Phase 1: build the sidecar binary

- [DONE] `.github/workflows/llama-server.yml` builds `llama-server` at tag v0.9.0 for `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`. Each binary is named `llama-server-<triple>[.exe]`.
- [TODO] Run the workflow on GitHub Actions. Confirm all three jobs pass and each artifact runs `--version`. Trigger it from the Actions tab with `workflow_dispatch`, or push a branch first.
- [TODO] Add the binaries to the release pipeline in `ci.yml`, so release builds bundle them.

## Phase 2: Tauri sidecar wiring

- [DONE] `src-tauri/tauri.sidecar.conf.json` adds `bundle.externalBin: ["binaries/llama-server"]`. It is an overlay, so the base config and `cargo test` do not need the binary. Release builds pass it with `--config`.
- [DONE] `scripts/prepare-sidecar.sh <dir> [triple]` copies a workflow artifact to `src-tauri/binaries/llama-server-<triple>[.exe]`. The folder is in `.gitignore`.
- [DONE] No Tauri shell permission is needed. The app spawns the server from Rust. Tauri places the bundled binary next to the app executable, under the name `llama-server`, which `locate_binary` already checks.
- [TODO] Build a release with the overlay: `npx tauri build --config src-tauri/tauri.sidecar.conf.json`, after running `prepare-sidecar.sh` for each target. Not yet in `ci.yml`.
- [TODO] Sign the binary with the app on macOS and Windows. Notarization fails when an unsigned binary sits inside the bundle.
- For development, the app finds the server through `LOCALAIDE_LLAMA_SERVER`, or next to the executable.

## Phase 3: sidecar manager in Rust

- [DONE] `src-tauri/src/services/inference/sidecar.rs` implements `InferenceEngine` with `SidecarEngine`. It starts `llama-server` on 127.0.0.1 with a free port and a 256-bit random API key. It passes `--mmproj` when `{id}.mmproj.gguf` sits next to the model. It waits for `/health`, streams from `/v1/chat/completions`, and kills the process on unload, reload, or exit. Server output goes to `~/.aiassistant/logs/llama-server.log`.
- [DONE] `app/state.rs` builds `SidecarEngine`. The worker engine, `services/inference/llama.rs`, and the legacy `src/inference.rs` are deleted. Their deletions are staged in git.
- [DONE] `Cargo.toml` drops `llama_cpp_rs` and its `[patch]` entry. It enables reqwest's `blocking` feature.
- [TODO] `vendor/llama_cpp_rs` is still on disk, unused. Delete it after the build passes on your machine.
- [TODO] Cancel behaves as before, but it applies at the next token. Check that Stop ends generation on a real model.
- [DONE] Real message history. The frontend sends earlier turns as `history` (the new message is excluded). `ContextBudget::build_messages` keeps the newest turns that fit, and drops older ones. The engine sends a message list, so the server applies the model's chat template. Summarize sends a system and a user message. `ContextBudget::build_prompt` stays for the existing tests.

## Phase 4: image input

- [TODO] Attach an image in the chat input. Send it as an `image_url` content part.
- [TODO] Load the model's `mmproj` companion. Show an error if the model has no projector.
- [TODO] Show the image in the transcript, so the user can see what the model received.

## Phase 5: speech through a rewritten in-process binding

- [DONE] Research. v0.9.0's TTS tool is `tools/tts/tts.cpp`, target `llama-tts`. It loads a backbone GGUF and an `mmproj` GGUF with audio generation. Its generation loop is `mtmd_helper::gen_audio`, a C++ class in `mtmd-helper.h`. The C header `mtmd.h` only exposes audio metadata and tensor reads. So an in-process binding needs a C++ shim, not a plain C wrapper.
- [FINDING] The ggml-org Qwen3-TTS 1.7B Base model has no built-in voices. It needs a reference speaker file (`--tts-speaker-file`) for each voice. The product must decide how users supply one.
- [TODO] Not started in code. Decide between the C++ shim (in-process, large, untested) and running `llama-tts` as a process from the CI build (smaller, testable in CI).
- [TODO] Vendor llama.cpp v0.9.0 for the speech path only. Write a small C++ shim around `mtmd_helper::gen_audio`, exposed to Rust as a C API.
- [TODO] Speech-to-text: confirm v0.9.0 can run Qwen3-ASR. The tree has only a Whisper audio encoder (`tools/mtmd/models/whisper-enc.cpp`). If it can't, pick a second runtime, and record the choice.
- [TODO] Add speech catalog entries only after each one loads on v0.9.0 and its license is checked.

## Phase 6: cleanup and docs

- [TODO] Update README, ARCHITECTURE, and `docs/scope.md`. Remove the claim that inference is in-process.
- [TODO] Add North Mini Code to the catalog. Its `cohere2moe` architecture is in v0.9.0.
- [TODO] Record the engine choice in a new ADR under `docs/adr/`.

## Risks

- The in-process speech binding is the largest piece. It is hand-written C FFI against a new API, and nothing here has compiled it yet.
- The sidecar adds a process. Startup time and crash recovery need tests on all three platforms.
- The chat prompt is flattened, so multi-turn quality will be weaker until the known gap above is fixed.
