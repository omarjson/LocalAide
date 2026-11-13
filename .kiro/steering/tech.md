---
inclusion: always
---

# LocalAide: Tech Stack

## Stack (verified against package.json, Cargo.toml, tauri.conf.json)

- **Shell:** Tauri 2 (`tauri` 2.12.x, `tauri-build` 2.7.x). Window config and CSP live in `src-tauri/tauri.conf.json`.
- **Rust core:** edition 2024, `rust-version` 1.90. Toolchain channel is `stable` (not pinned to 1.90). Crate name `ai-desktop-assistant`, library crate `app_lib`.
- **Frontend:** Svelte 5 with runes (`$state`, `$derived`), TypeScript (`~5.8.2`), Tailwind CSS v4 (`@tailwindcss/vite`, `@import "tailwindcss"` in `src/main.css`), Vite 6.
- **Inference:** `llama_cpp_rs`, patched to the vendored copy in `vendor/llama_cpp_rs`. Runs GGUF models on a dedicated OS thread.
- **Storage:** `rusqlite` with the bundled SQLite. WAL mode, `synchronous = NORMAL`, `busy_timeout = 5000`. FTS5 table `chunks_fts` for keyword search.
- **Other crates:** `sha2` (SHA-256), `reqwest` (downloads), `flate2` (PDF stream), `notify` (folder watching), `sysinfo` (RAM probe), `tracing`/`log`.
- **Legacy, present but not the primary path:** `tantivy`, `sled`. ADR-0002 moved storage to SQLite.
- **Optional, off by default:** `voice` Cargo feature (`whisper-rs`, `piper-rs`, `hound`). Do not enable it in v1.0 (ADR-0006).

## Data locations (`src-tauri/src/app/paths.rs`)

Everything sits under `~/.aiassistant/`:

| Path | Contents |
|---|---|
| `~/.aiassistant/index/library.db` | SQLite database: documents, chunks, FTS5, notes index, tool audit |
| `~/.aiassistant/memory/` | Markdown memory notes |
| `~/.aiassistant/workspace/` | Jail root for tools |
| `~/.aiassistant/models/` | Downloaded GGUF models |
| `~/.aiassistant/conversations/` | Chat threads |
| `~/.aiassistant/config.json` | App config |

The database is `index/library.db`, not `~/.aiassistant/library.db` as the older docs say.

## Commands (verified in package.json and Makefile)

- `npm run dev`: Vite dev server. Use `npx tauri dev` for the desktop shell.
- `npm run build`: `tsc --noEmit && vite build`.
- `npm run check`: `svelte-check` with `tsconfig.json`.
- `npm run smoke` / `npm test`: `node scripts/smoke-test.mjs`.
- `cd src-tauri && cargo test --all-targets`.
- `cd src-tauri && cargo clippy --all-targets`. The Makefile does not pass `-D warnings`. Note that `src-tauri/src/lib.rs` sets `#![allow(clippy::all, unused)]`, so clippy is quieter than the default.
- `make all` runs check, test and smoke. `make release` runs `npx tauri build`.

## Conventions

- **Typed IPC only.** Every Tauri command takes a concrete DTO and returns `Result<T, AppError>`. No `serde_json::Value` at the boundary. Frontend bindings live in `src/lib/api/bindings.ts`.
- **No `any` in TypeScript.** Current code has `catch (e: any)` in several screens. Do not add new ones, and use `unknown` in new code.
- **Single state owner.** All mutable state sits in `AppState` (`src-tauri/src/app/state.rs`), managed by Tauri.
- **Inference off the UI thread.** Cancellation uses an `Arc<AtomicBool>` flag checked on every token.
- **No `unwrap()` or `expect()` on user-facing paths.** The current code has 16 occurrences in `src-tauri/src`. Keep new code free of them.
- **Sandbox every tool path.** Resolve through `WorkspaceJail` in `src-tauri/src/services/tools/sandbox.rs`. Reject `..`, escaping symlinks and symlink loops.

## Verify before you claim

Before you say a feature works, check the code path end to end. Two past doc claims were wrong: hybrid vector search and Ed25519 signing. Read the code, then trust it over the README.
