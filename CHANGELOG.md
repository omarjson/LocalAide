# Changelog

All notable changes to LocalAide are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-11-18

Installs, updates and polish, from testing 0.1.1 on an Intel Mac and on Windows 11. This is the first version that can update itself, so people on 0.1.0 or 0.1.1 install it by hand once. On Windows the new installer replaces the old version; on macOS drag the new app over the old one. Your chats, notes, models and settings in `~/.aiassistant/` are kept.

### Added
- **In-app updates.** Settings → About → "Check for updates" shows the new version and what changed. "Install and restart" downloads it, checks its signature, installs it and keeps your data. Nothing is checked when the app starts.
- **Privacy Policy and Terms of Use.** Readable in Settings → About → Legal, the command palette and the setup screen. The Windows installer shows the terms as its license page.
- **Discover skills.** The Skills screen can browse and add skills from Anthropic Skills, GitHub Awesome Copilot, prompts.chat (about 2,600 prompts), or any public GitHub repository with a `skills` folder. Each added skill keeps its source and license.
- **Free and local online providers.** Pollinations works with no key. 9Router, OmniRoute, LM Studio and Ollama run on your own computer. Google Gemini, OpenRouter, Groq, Cerebras, Hugging Face, NVIDIA, Mistral, LLM7 and Chutes have free keys; OpenAI, DeepSeek and Together are paid. "Load models" shows what a provider offers today and marks the free models.
- **Hugging Face browsing.** Community models open on what is trending, and can be sorted by downloads, likes or newest. "Unfiltered only" shows uncensored and abliterated models, with a warning.
- **Command palette actions.** Ctrl/⌘+K can start a new chat, switch the theme, open online providers, check for updates and open the legal pages. Arrow keys and Enter work in the list.

### Changed
- **Icons and installers.** The app and installer icons are the LocalAide ghost; they were the default Tauri icons. The Windows installer has a branded header and side panel and installs for the current user, without an administrator prompt. The macOS disk image has a branded background.
- **Motion.** Screens, settings sections, setup steps and tabs animate. Lists appear one item after another, cards lift on hover and loading shows placeholders. All of it turns off when the system asks for reduced motion.
- **Fuller screens.** Chat, Skills and Setup have proper starting screens. Setup shows which step you are on.
- An API key is no longer required to add an online provider. A provider that needs one says so in plain words.
- Dependency updates: reqwest 0.13 (HTTPS now uses rustls with the operating system's certificate store instead of native-tls), sha2 0.11, rusqlite 0.42, getrandom 0.4, tauri-plugin-updater 2.14 (new), four smaller Rust updates, and six GitHub Actions updates. Stored file hashes and the catalog digest are unchanged.

### Fixed
- Search boxes no longer show a second box inside them when focused, or two clear buttons. Password fields no longer show Windows' extra reveal and clear buttons, and dropdowns show one arrow.
- The `llama-server` engine is built without llama.cpp's web chat page. That page is downloaded at build time and its files disappeared, which broke every build on 2026-11-18. LocalAide never used it.

### Release
- Each GitHub Release now uses its section of this changelog as its notes, and the release job fails if the section is missing.

## [0.1.1] - 2026-11-15

Merges the `design/windows-theme` work (UI redesign and production-hardening Phases 0 to 5, audit in `docs/evidence/audit-2026-11-14.md`). A `[build-all]` CI run on `6bd9194` passed every job, including all four `llama-server` builds and the macOS, Windows and Linux installers.

### Added
- UI redesign: shared components (buttons, tabs, page headers, settings rows, confirm dialogs, focus trap, toasts), plain copy, Ctrl+1 to Ctrl+6 screen shortcuts, native folder and file pickers, and chat export through a save window. Mockups live in `docs/design/mockups/`.
- Release pipeline: tag builds create a draft GitHub Release with installers, `SHA256SUMS.txt` and an SPDX SBOM; a version gate checks `package.json`, `Cargo.toml`, `tauri.conf.json` and the tag agree; portable `llama-server` builds (no native CPU flags, no OpenMP, static MSVC runtime) must start in CI; optional code signing through repository secrets. See `docs/release.md`.
- Third-party notices and a `cargo-about` license file ship inside the app; Dependabot and a weekly dependency audit.
- A Windows Rust job (clippy and tests) and readable CI failure annotations.
- Web search and page reading (ADR-0011): the Search screen's Web mode queries the open internet — DuckDuckGo (lite results page), Wikipedia REST search, and the Jina Reader work keylessly; Exa and Firecrawl run after the user pastes an API key in Settings → Web search. "Read page" pulls a result's content as markdown (Firecrawl when keyed, the keyless Jina Reader otherwise). Every request is an explicit click; keys live in the local config, travel only to their own provider, and are excluded from backups unless included. HTTP-level tests exercise every provider against loopback servers.
- Online-provider presets from the open 9Router registry (ADR-0011): the Settings provider form can prefill from LLM7, Synthetic, api.airforce, Groq, Cerebras, NVIDIA NIM, OpenRouter, Chutes, Mistral, or the user's own local Ollama, each with a note and a key link. Free tiers are labeled as changing without notice; loopback proxies may be saved without a key.
- Light theme: the ink ramp inverts to bone paper with dark ink text, accent text roles darken to stay readable, and the choice persists in config with a Settings toggle. Dark remains the designed default; the light theme has not yet been eyeballed on a real screen.
- First-run wizard community step: besides the RAM presets, the wizard can search public Hugging Face GGUF repositories, expand a repo's files, and install one through the same hash-verified download pipeline.
- Backup stance confirmed: the `.labak` archive is a portable file the user can upload anywhere; built-in passphrase-verified restore was already shipped and stays the only restore path.

### Changed
- Responsiveness: heavy commands run on worker threads, the model loads after the window opens, one load at a time, and Stop works before the first token.
- Library indexing skips unchanged files, streams vectors, reads UTF-16/Windows-1252 text and PDFs, caps file sizes, depth and counts, and refuses symlinks and junctions. Deleted folders leave the library.
- Downloads have timeouts, a free-space check, off-thread checksums and HTTP 416 recovery.
- Backup format v2 (Argon2id 64 MiB / 3 passes in an authenticated header); v1 archives still open.
- Rust toolchain pinned to 1.100.0 in both `rust-toolchain.toml` and CI.
- Meaningful borders moved from the `ink-800`/`ink-700` tokens (1.2:1 / 1.9:1 — both below the 3:1 non-text floor) to a dedicated `edge` ramp sized to clear 3:1 in both themes; text on ember fills now uses the semantic `on-ember` token that flips with the theme. Arithmetic recorded in `docs/evidence/a11y.md`; on-screen re-measure still open.

### Fixed
- Engines stop on exit, and child processes join a kill-on-close job object on Windows, so no `llama-server` is left running. Single-instance lock and a minimum window size.
- Download Cancel now matches the job it targets.
- Path checks on model ids, note paths, thread ids, backup entries and companion file names.
- Using a model loads it; downloads no longer mark a model active; nothing contacts GitHub at launch.

### Security
- Cloud chat follows no redirects, needs the key re-entered when a provider's host changes, and only `cloud_mode_set` turns online chat on. Local-network pages are never sent to page readers.
- Stricter CSP, links open in the browser for http/https only, random provider ids, engine key passed through `LLAMA_API_KEY`, folder approvals expire after 30 minutes, and refused tool actions are logged.

### Removed
- The dev-only Developer config screen, unused commands (`config_save`, `config_compute_ram`, `model_existing_size`, `chat_thread_export`), the unused prompt builder, `OPTIMIZATIONS.md`, `PACKAGING.md` and the lock-update workflow. Old config and state files still load.

## [0.1.0] - 2026-11-14

First published release of LocalAide, tagged `v0.1.0`. Every platform stage — llama-server sidecar builds and app bundles (`.dmg` for Apple Silicon and Intel, `.msi`/NSIS for Windows, `.deb`/`.AppImage` for Linux) — completed green in CI on the tagged commit, and the installers are attached to the GitHub Release.

### Added
- llama-server sidecar engine (ADR-0007): chat runs in a llama.cpp `llama-server` child process on `127.0.0.1`, built from the pinned llama.cpp tag v1.0.0 by CI (`.github/workflows/llama-server.yml`) and bundled via the `tauri.sidecar.conf.json` overlay.
- Image input for vision models: attach up to 4 images; the sidecar loads the model's `mmproj` projector and streams image content parts to the server.
- Opt-in Online Providers mode: `RouterEngine` routes between the local sidecar and a `RemoteEngine` for OpenAI-compatible providers configured in Settings with the user's own API key. Base URLs must be HTTPS; plain HTTP is accepted only for loopback proxies. Off by default, and a cloud badge in the chat header shows while it is active.
- Kiroween 2026 work merged to `main` (`bb0d0aa`): spec-driven engine upgrade and design pass (`.kiro/specs/kiroween-2026/`), a 36-model catalog verified against Hugging Face, the folder-indexing fix, and the stitched-ghost design tokens.
- Full platform verification (2026-11-14): a `[build-all]` run completed every platform stage for the first time — macOS Apple Silicon and Intel sidecars built on `macos-latest` (the retired `macos-14` label cancelled jobs at queue time) and bundle artifacts produced for macOS (`.dmg`, both architectures), Windows (`.msi`/NSIS), and Linux (`.deb`/`.AppImage`).
- Community models from the Hugging Face hub (ADR-0010): a search box on the Models screen lists public GGUF repositories (downloads, likes, license, gated badge), shows each repo's pinned GGUF files, and registers any file as a downloadable model — split GGUFs carry their shards, vision projectors ride along, and every download is verified against the hash the hub serves. Community models are badged as such; the weights and licenses are not vetted by LocalAide. A model picker in the chat header switches between installed models (loading now resolves through the filesystem, which also makes freshly downloaded and split-shard models loadable).
- Model catalog refresh (ADR-0008): an explicit "Refresh catalog" action fetches `catalog/models.json` from this repository over a pinned HTTPS URL, validates it (integrity digest, structural checks, version anti-rollback) before swapping it in, caches it next to `config.json`, and offers a Reset that restores the built-in list. A parity test keeps the JSON and the compile-time list identical.
- Catalog v8 (2026-11-14, operator review): the newest verified models per role — Spark-X2.6 4B and 1.7B (October 2026 on-device agent models), GLM-5.3 Flash (320B-total/18B-active MoE with vision), Mistral Small 4 (119B MoE, native MXFP4, vision), and Nomic Embed v2 MoE. Split-GGUF support: a catalog entry can now name its primary file and carry shard companions, so multi-shard llama.cpp models download and load end to end; the pin verifier checks shard totals.
- Companion downloads fixed: vision projector (`mmproj`) files now download with their model, and the Models screen shows a Vision badge plus kind filters (Chat/Code/Embedding/Reranker). Deleting a model also deletes its companions.
- Semantic and hybrid retrieval: a local embedding engine (a second `llama-server --embedding` instance) vectorizes library chunks at ingest and powers Semantic/Hybrid search modes with reciprocal-rank fusion; memory recall and chat RAG embed queries too. Includes a one-click backfill for previously indexed chunks and a truthful `embedding_status`.
- Reranker second stage: a third `llama-server --rerank` instance re-orders the top fused search candidates by true query-document relevance through `/v1/rerank` whenever its engine is running — on the Search screen, memory search, and chat RAG alike. A rerank failure degrades to the fused ranking instead of failing the search; the reranker model (the catalog's Qwen3 Reranker 0.6B, or a community one) is started from a chip on the Search screen and shown in diagnostics.
- Skills: reusable prompt templates as local markdown files (`~/.aiassistant/skills/`) with a full editor screen; typing `/` in chat opens a skill picker. Custom prompts reused three or more times surface a local "save as skill?" suggestion — heuristics only, nothing leaves the machine.
- Persisted chat preferences: system prompt, sampling temperature, response cap, and persona survive restarts, with a context-window slider in Settings.
- Engine context is now a setting (default 8192 tokens) and is capped per model by the catalog's native context hint, replacing the hard-wired 2048.
- Diagnostics: a Settings → Diagnostics tab with local subsystem health, a llama-server log viewer, and a display-only update check against GitHub Releases (the app never downloads anything by itself).
- Encrypted local backups (ADR-0009): one AES-256-GCM archive (`.labak`) of config, memory notes, and conversations under an argon2id-derived key from a passphrase that is never stored. Local-folder targets ship working (including desktop-synced OneDrive/Drive folders, with zero OAuth — the portable archive can be uploaded anywhere the user chooses); provider API keys are excluded by default; restore verifies the passphrase before touching any file and never clobbers a newer config.

### Changed
- Toolchain currency pass: Node 20 → 24 (removed from GitHub runners 2026-09-23), GitHub Actions checkout/setup-node v4 → v5, Vite 6 → 8, vite-plugin-svelte 5 → 7, vite-plugin-checker 0.8 → 1.0, svelte-check 4.1 → 4.7, TypeScript → 5.9, notify 6 → 8, and the llama.cpp sidecar pin v0.9.0 → v1.0.0 (the semver milestone). Unused `autoprefixer`/`postcss` devDependencies were removed and an `engines` field pins Node ≥ 24.
- Platform builds (sidecar, macOS/Windows/Linux bundles) run only for release tags, manual dispatch, or commits marked `[build-all]` — routine pushes verify with the quality gate alone, keeping pushes fast.
- Documentation truth passes: steering docs, ADRs, README, PACKAGING, scope, and evidence files aligned with the shipped code (chunk sizes, search honesty, catalog digest, sidecar build steps).

### Removed
- Dead code: the in-process llama_cpp_rs engine and its vendored fork, the legacy tantivy/sled indexer, the old top-level watcher/embeddings/voice/components modules, and the obsolete sandbox path (the jail uses `std::fs::canonicalize` plus lexical checks).

## Earlier milestones

Recorded here because these milestones were never published as releases under the version headings they originally carried.

- **2026-10-30 planning pass** (originally headed `0.2.0-alpha`): scope contract and backlog (`docs/scope.md`, `docs/backlog.md`), ADR-0001 through ADR-0006, the rubric scorecard, and the PR template.
- **Initial prototype** (originally headed `0.1.0`): Tauri v2 + Svelte 5 + Tailwind scaffold, a model-download scaffold, and a Tantivy/Sled search prototype, since replaced by SQLite FTS5 (ADR-0002).
