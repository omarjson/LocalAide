# Packaging Guide

LocalAide packages native desktop bundles using Tauri v2 CLI.

## Prerequisites
- Rust stable 1.90+
- Node.js 20+
- Platform build dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Visual Studio C++ Build Tools (MSVC)
  - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
- **The `llama-server` sidecar binary.** The app chats through a local `llama-server` child process (ADR-0007), so the package must bundle it. Build it from the pinned llama.cpp tag (v0.9.0) and stage it with `scripts/prepare-sidecar.sh` into `src-tauri/binaries/`, or let CI do both — see [.github/workflows/llama-server.yml](.github/workflows/llama-server.yml).

## Local Packaging Commands

Every platform builds with the `src-tauri/tauri.sidecar.conf.json` overlay, which adds `bundle.externalBin` so the sidecar is bundled. A bare `npx tauri build` without the overlay produces a package that cannot chat.

### macOS
```bash
# Stage the sidecar artifact, then build the DMG / App Bundle
scripts/prepare-sidecar.sh <artifact-dir> aarch64-apple-darwin
npm run build
npx tauri build --config src-tauri/tauri.sidecar.conf.json
```
Output bundles: `src-tauri/target/release/bundle/dmg/`

### Windows
```bash
# Stage the sidecar artifact, then build the NSIS / MSI installers
scripts/prepare-sidecar.sh <artifact-dir> x86_64-pc-windows-msvc
npm run build
npx tauri build --config src-tauri/tauri.sidecar.conf.json
```
Output bundles: `src-tauri/target/release/bundle/nsis/` and `msi/`

### Linux
```bash
# Stage the sidecar artifact, then build the AppImage / Deb
scripts/prepare-sidecar.sh <artifact-dir> x86_64-unknown-linux-gnu
npm run build
npx tauri build --config src-tauri/tauri.sidecar.conf.json
```
Output bundles: `src-tauri/target/release/bundle/appimage/` and `deb/`
