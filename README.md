# LocalAide

Local-first AI desktop assistant built with Rust + Tauri v2, Svelte 5 runes, and Tailwind CSS 4.

**LocalAide** runs summarization, tool calling, image understanding, semantic search/indexing, voice, automation, and persistent memory entirely on your machine. No telemetry by default, SHA-256 verified model downloads, and modular updates.

## Highlights

- Local inference with llama-cpp-rs GGUF and Candle EmbeddingGemma 2 multimodal embeddings
- Optional voice stack: Whisper Tiny/Base STT and Piper TTS behind the `voice` Cargo feature
- Vision scaffold for Qwen2.5-VL via Candle-transformers
- Semantic search with Tantivy BM25, batched commits, manual reader reload
- Persistent memory via sled/lmdb under `~/.aiassistant/`
- Model Manager with resume/cancel downloads and SHA-256 verification
- Multi-arch releases: macOS Intel/Apple Silicon dmg, Windows x64/ARM64 NSIS/MSI/EXE
- Dark industrial UI: zinc/fuchsia/cyan, glass sidebar, rounded-2xl cards

## Tech Stack

- **Rust 1.90+** / Tauri v2
- **Frontend**: Svelte 5 runes, TypeScript, Tailwind CSS 4
- **Backend crates**: `tauri`, `sysinfo`, `reqwest` stream, `futures`, `serde`, `sled`, `tantivy`, `llama-cpp-rs` 0.3.0 vendored via `[patch.crates-io]`, `candle-core/candle-transformers/candle-nn`
- **Models**: EmbeddingGemma 2 300M multimodal, Whisper Tiny/Base, Piper TTS, Qwen2.5-VL vision
- **RAM budget**: `(total RAM − 2 GB) × 0.7` with presets Tiny/Small/Balanced/Advanced

## Requirements

- Rust stable 1.90+
- Node 20+ and npm
- Targets for cross builds:
  - `x86_64-apple-darwin`, `aarch64-apple-darwin`
  - `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`
- ~2-10 GB free for models depending on preset

## Install

Prebuilt releases are published from CI artifacts. Download the dmg or installer for your platform and run.

## Development

```bash
git clone https://github.com/omarjson/LocalAide.git
cd LocalAide
npm install
# add Rust targets if building for other platforms
rustup target add x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc aarch64-pc-windows-msvc
cargo tauri dev
```

First run opens the Setup Wizard. It probes system RAM via `sysinfo`, recommends a preset, and downloads models to `models/` with resume and SHA-256 verification.

### Project layout

```
src/               # Svelte frontend
  screens/         # SetupWizard, Chat, Search, Memory, ModelManager, Settings, DevConfig
  components/      # MessageBubble, ChatInput
  lib/             # format.ts, types.ts, tauri.d.ts
src-tauri/         # Rust backend
  src/
    inference.rs   # stream_id based chat_stream and summarize
    model_manager.rs
    indexer.rs     # BM25, batched commits, MAX_INDEX_BYTES 2MB
    watcher.rs     # debounced file watcher with error logging
    embeddings.rs  # EmbeddingEngine shared, deterministic fallback
    voice.rs       # cfg(feature = "voice") Whisper/Piper stubs
```

## Build

### macOS Intel
```bash
cd src-tauri
cargo tauri build --target x86_64-apple-darwin
# output: target/x86_64-apple-darwin/release/bundle/dmg/
```

### macOS Apple Silicon
```bash
rustup target add aarch64-apple-darwin
cargo tauri build --target aarch64-apple-darwin
```

### Windows x64
```bash
rustup target add x86_64-pc-windows-msvc
cargo tauri build --target x86_64-pc-windows-msvc
# output: .../bundle/nsis/ and .../msi/
```

### Windows ARM64
```bash
rustup target add aarch64-pc-windows-msvc
cargo tauri build --target aarch64-pc-windows-msvc
```

The frontend is built by `npm run build` before `cargo tauri build`. CI caches Cargo and npm for speed.

## Configuration

Data is stored locally:

- `~/.aiassistant/config.json`
- `models/` – GGUF and safetensors
- `index/` – Tantivy index
- `memory/` – sled/lmdb stores

Edit `src-tauri/tauri.conf.json` for product name and bundle settings. The app is rebranded to **LocalAide**, identifier `com.localaide.app`.

## Models and Presets

Presets compute a RAM budget as `(total RAM − 2 GB) × 0.7`.

- **Tiny** ~1.9 GB – EmbeddingGemma 2, Qwen3.5-0.8B Q4, Whisper Tiny, Piper small
- **Small** ~2.7 GB – EmbeddingGemma 2, Qwen3.5-2B Q4, Whisper Base
- **Balanced** ~4.3 GB – EmbeddingGemma 2, Qwen3.5-4B Q4, Whisper Base, Piper
- **Advanced** ~9.5 GB – Qwen3.5-9B Q4, EmbeddingGemma 2, Vision SLM

Downloads are verified by SHA-256 and can be resumed or cancelled.

## Voice

Voice features are optional:

```bash
cargo tauri dev --features voice
```

Requires Whisper and Piper models in `models/`. Transcription uses `hound` WAV decoding. TTS outputs WAV files.

## Security & Privacy

- Local-first, no telemetry by default
- Tools sandbox enforces size caps, path traversal rejection, forbidden roots
- Model downloads are SHA-256 verified
- See `SECURITY.md`

## Troubleshooting

- **zstd-sys / Windows cross-compile errors**: use GitHub Windows runners for release builds; local macOS cross-compile needs mingw-w64 sysroot
- **Voice build fails**: `voice` feature needs cmake/ffmpeg and ONNX prebuilts for the target
- **Indexer not updating**: the watcher debounces changes and reloads the Tantivy reader manually; check logs for `skip dir` warnings
- **Model download stuck**: cancel via UI; partial files are cleaned up and resume works on retry

## Modular Updates

Components such as llama-cpp, candle, whisper, piper are versioned independently. The Model Manager and Component Updater can replace a module without a full reinstall.

See `docs/MODULAR_UPDATES.md` and `OPTIMIZATIONS.md`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

MIT
