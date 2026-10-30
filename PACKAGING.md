# Packaging Guide

LocalAide packages native desktop bundles using Tauri v2 CLI.

## Prerequisites
- Rust stable 1.90+
- Node.js 20+
- Platform build dependencies:
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Visual Studio C++ Build Tools (MSVC)
  - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`

## Local Packaging Commands

### macOS
```bash
# Build native macOS DMG / App Bundle
npm run build
npx tauri build
```
Output bundles: `src-tauri/target/release/bundle/dmg/`

### Windows
```bash
# Build native Windows NSIS / MSI installers
npm run build
npx tauri build
```
Output bundles: `src-tauri/target/release/bundle/nsis/` and `msi/`

### Linux
```bash
# Build native Linux AppImage / Deb
npm run build
npx tauri build
```
Output bundles: `src-tauri/target/release/bundle/appimage/` and `deb/`
