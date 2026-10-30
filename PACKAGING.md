# Packaging

## Windows (NSIS) Intel/x64
```bash
cd src-tauri
cargo tauri build --target x86_64-pc-windows-msvc
```
Output: `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/...` and `nsis/...`

## macOS Intel
```bash
cd src-tauri
cargo tauri build --target x86_64-apple-darwin
```
Output: `src-tauri/target/x86_64-apple-darwin/release/bundle/dmg/...`

## Prerequisites
- Rust 1.90+
- Node 18+
- Xcode Command Line Tools (macOS)
- Rust targets installed: `rustup target add x86_64-pc-windows-msvc x86_64-apple-darwin`

## Notes
- Bundle config in `src-tauri/tauri.conf.json` sets `bundle.active = true`, `targets = "all"`.
- Icons are placeholders in `src-tauri/icons/`.
- Frontend is built via `npm run build` before Tauri build (`beforeBuildCommand`).
