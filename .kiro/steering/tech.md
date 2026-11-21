---
inclusion: always
---

# LocalAide: Tech Stack

## Stack (verified against package.json, Cargo.toml, tauri.conf.json)

- **Shell:** Tauri 2 (`tauri` 2.12.x, `tauri-build` 2.7.x), with `tauri-plugin-log` (always on) and `tauri-plugin-dialog`. `tauri-plugin-opener` is a dependency for its `open_url` function only; the plugin is not registered. Window config and CSP live in `src-tauri/tauri.conf.json`.
- **Rust core:** edition 2024, `rust-version` 1.90. The toolchain is pinned to 1.100.0 in `rust-toolchain.toml`, which rustup applies to every cargo command in the repo; `RUST_TOOLCHAIN` in `ci.yml` must match it (the version gate checks). Crate name `ai-desktop-assistant`, library crate `app_lib`. Release profile: thin LTO, `strip`, panics unwind.
- **Frontend:** Svelte 5 with runes (`$state`, `$derived`), TypeScript (`~5.9`), Tailwind CSS v4 (`@tailwindcss/vite`, `@import "tailwindcss"` in `src/main.css`), Vite 8.
- **Inference:** llama.cpp's `llama-server` run as a local sidecar child process on 127.0.0.1 (ADR-0007), given its API key through the `LLAMA_API_KEY` environment variable. The app talks to it over HTTP through an `InferenceEngine` trait (`services/inference/`): `SidecarEngine` for local models, plus an opt-in cloud router (`RouterEngine` + `RemoteEngine`) for user-configured OpenAI-compatible providers. `RemoteEngine` does not follow redirects.
- **Storage:** `rusqlite` with the bundled SQLite. WAL mode, `synchronous = NORMAL`, `busy_timeout = 5000`. FTS5 table `chunks_fts` for keyword search; `chunks.fts_rowid` links each chunk to its full-text row (schema version 2).
- **Other crates:** `sha2` (SHA-256), `reqwest` (downloads, web, cloud), `flate2` and `pdf-extract` (PDF text), `encoding_rs` (UTF-16 and Windows-1252 text), `notify` (folder watching), `sysinfo` (RAM and disk probe), `aes-gcm` and `argon2` (backups), `getrandom`, `log`.
- **Legacy crates removed:** the old `tantivy`/`sled` indexer is gone; SQLite is the only store (ADR-0002).
- **Optional, off by default:** `voice` Cargo feature (`whisper-rs`, `piper-rs`, `hound`). Do not enable it in shipped builds (ADR-0006).

## Data locations (`src-tauri/src/app/paths.rs`)

Everything sits under `~/.aiassistant/`:

| Path | Contents |
|---|---|
| `~/.aiassistant/index/library.db` | SQLite database: documents, chunks, FTS5, notes index, tool audit |
| `~/.aiassistant/memory/` | Markdown memory notes |
| `~/.aiassistant/workspace/` | Jail root for tools |
| `~/.aiassistant/models/` | Downloaded GGUF models |
| `~/.aiassistant/conversations/` | Chat threads |
| `~/.aiassistant/logs/` | `localaide.log` (app, rotated at 5 MB, 5 kept) and `llama-server.log` |
| `~/.aiassistant/config.json` | App config, written atomically |
| `~/.aiassistant/instance.lock` | Single-instance lock |

## Commands (verified in package.json and Makefile)

- `npm run dev`: Vite dev server. Use `npx tauri dev` for the desktop shell.
- `npm run build`: `tsc --noEmit && vite build`.
- `npm run check`: `svelte-check` with `tsconfig.json`.
- `npm run smoke`: `node scripts/smoke-test.mjs` (checks `dist/`).
- `npm run test:unit`: Node's test runner on `tests/unit/*.test.ts`. `npm test` runs the smoke test and the unit tests.
- `cd src-tauri && cargo test --all-targets`.
- `cd src-tauri && cargo clippy --all-targets -- -D warnings`, as CI runs it. No crate-level `allow` hides lints.
- `make all` runs check, fmt, lint, test and smoke, like CI. `make release` runs the release-overlay build (`npx tauri build --config src-tauri/tauri.release.conf.json`) after `npm run build`.
- On Windows use `npm.cmd`, since PowerShell may block `npm.ps1`.

## Conventions

- **Typed IPC only.** Every Tauri command takes a concrete DTO and returns `Result<T, AppError>`. No `serde_json::Value` at the boundary. Frontend bindings live in `src/lib/api/bindings.ts`. Tauri 2 expects camelCase argument names from the frontend.
- **No `any` in TypeScript.** Use `unknown` and `errorMessage()` for caught errors.
- **Single state owner.** All mutable state sits in `AppState` (`src-tauri/src/app/state.rs`), managed by Tauri.
- **Never block the UI thread.** Slow commands are `async` and run their work through `app/blocking.rs`. Inference cancellation uses an `Arc<AtomicBool>` flag checked every 200 ms.
- **No `unwrap()` or `expect()` on user-facing paths.** Outside tests, three `expect` calls remain, each on an invariant the code checks first (`commands/hf.rs`, `services/models/mod.rs`, the Argon2 call in `services/backup/mod.rs`). Poisoned locks use `unwrap_or_else(|e| e.into_inner())`. Keep new code free of them.
- **Sandbox every tool path.** Resolve through `WorkspaceJail` in `src-tauri/src/services/tools/sandbox.rs`. Reject `..`, escaping symlinks and symlink loops.
- **Links open outside the app.** Use `openInBrowser` (`src/lib/links.ts`); the backend opens http(s) links only.

## Verify before you claim

Before you say a feature works, check the code path end to end. Past doc claims that were wrong: hybrid vector search (once dead code, now wired), Ed25519 signing (never existed), a `security@localaide.dev` mailbox (the domain does not exist). Read the code, then trust it over the README.
