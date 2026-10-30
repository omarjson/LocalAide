# LocalAide

Local-first AI desktop assistant built with Rust + Tauri v2, Svelte 5, and Tailwind CSS 4.

**LocalAide** brings summarization, tool calling, image understanding, semantic search/indexing, voice, automation, and persistent memory to your desktop — all running locally with no telemetry by default.

## Features

* Local inference: llama-cpp-rs GGUF, Candle EmbeddingGemma 2 multimodal
* Voice: Whisper STT, Piper TTS via optional `voice` feature
* Vision: Qwen2.5-VL scaffold via Candle-transformers
* Semantic search: Tantivy BM25 with batched commits
* Persistent memory: sled/lmdb, `~/.aiassistant/`
* Model manager: SHA256-verified downloads with resume/cancel
* Packaging: macOS dmg, Windows NSIS/MSIX, Intel & Apple Silicon

## Quick Start

```bash
git clone https://github.com/<user>/localaide.git
cd localaide
npm install
cargo tauri dev
```

First run opens Setup Wizard to download models.

## Builds

### macOS Intel
```bash
cd src-tauri
cargo tauri build --target x86_64-apple-darwin
```
Output: `target/x86_64-apple-darwin/release/bundle/dmg/`

### macOS Apple Silicon
```bash
rustup target add aarch64-apple-darwin
cargo tauri build --target aarch64-apple-darwin
```

### Windows x64
```bash
rustup target add x86_64-pc-windows-msvc
cargo tauri build --target x86_64-pc-windows-msvc
```
Output: `target/x86_64-pc-windows-msvc/release/bundle/nsis/` and `msi/`

### Windows ARM64
```bash
rustup target add aarch64-pc-windows-msvc
cargo tauri build --target aarch64-pc-windows-msvc
```

## Modular Updates

Components like llama-cpp, candle, whisper, piper are versioned in `models/` and `components/`. The app can update individual modules without a full package reinstall via the Model Manager and Component Updater.

See `docs/MODULAR_UPDATES.md`.

## License

MIT

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
