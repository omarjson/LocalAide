# Modular Updates

LocalAide is designed for independent updates of models and runtime components.

## Components

* `models/` - GGUF, safetensors, ONNX models. Managed by `model_manager` with SHA256 verification, resume, cancel.
* `components/` - Optional native binaries or crates that can be hot-swapped, e.g. `llama_cpp_rs` custom build, `whisper-rs`, `piper-rs`.
* `tools/` - Sandboxed tool binaries.

## Update Flow

1. App checks `https://updates.localaide.dev/manifest.json` for component versions.
2. User approves updates in Settings → Updates.
3. Downloads are streamed with progress, SHA256 verified, written to `~/.aiassistant/components/<id>/`.
4. On next restart, the component is loaded from the new path.

No full package reinstall required.

## Adding a New Module

Add entry to `src-tauri/src/components/catalog.rs` with:
* name, version, url, sha256, platform matrix
* install path

The updater handles the rest.
