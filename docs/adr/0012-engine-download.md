# ADR-0012: Model engine downloaded for the processor

Status: accepted (0.1.7)

## Context

Up to 0.1.6 every installer carried a `llama-server` that CI built from llama.cpp with `GGML_NATIVE=OFF`. On x86_64 that build needs AVX2. A user on a processor without one of its instructions got `exit code: 0xc000001d` (illegal instruction) the moment a model loaded, and no model could ever run. The bundled build also had no Windows on ARM version, and it had to be built and tested for every platform in CI.

## Decision

LocalAide no longer ships `llama-server`. The first time a model loads (chat, embedding, reranking, the self-test or the startup auto-load), `services/engine.rs` downloads the official llama.cpp release build for this computer and keeps it in `~/.aiassistant/tools/llama.cpp-<build>/`.

- The build is pinned: llama.cpp `b12359` (the v1.0.0 release), one URL, size and SHA-256 per platform in `services/externals.rs`. A file that does not match is deleted and never run.
- The build is picked for the processor, not for the app. `native_arch()` uses `IsWow64Process2` on Windows and `sysctl.proc_translated` on macOS, so an x64 app on Windows on ARM, or an Intel app under Rosetta, still gets the ARM64 engine. The engine is a separate process, so it does not have to match the app.
- The x64 Windows and Linux builds have one CPU library per instruction set (`ggml-cpu-sse42` up to `ggml-cpu-zen4`/AVX-512) and load the best one the processor supports, which fixes the 0xc000001d crash on older processors. Windows ARM64 and macOS builds have one CPU library; macOS adds Metal.
- One download at a time; the window shows its progress through the `engine-download` event. The download is listed in the cloud audit log as a GitHub download with no personal content.
- `LOCALAIDE_LLAMA_SERVER`, or a `llama-server` next to the app, still wins, for development.
- If a server still stops with 0xc000001d or SIGILL, the error says the processor lacks an instruction, instead of only the exit code.

## Consequences

- Installers are smaller, and CI no longer builds llama.cpp (`.github/workflows/llama-server.yml` and `scripts/prepare-sidecar.sh` are gone).
- The first model load needs the internet once (13 to 25 MB). The model download already needed it.
- Updating llama.cpp is a change of the pins, not a CI build.
- The engine is the official build, not one we compiled, so its flags are llama.cpp's (`GGML_BACKEND_DL`, `GGML_CPU_ALL_VARIANTS`).
