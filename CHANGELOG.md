# Changelog

All notable changes to LocalAide are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Model catalog refresh (ADR-0008): an explicit "Refresh catalog" action fetches `catalog/models.json` from this repository over a pinned HTTPS URL, validates it (integrity digest, structural checks, version anti-rollback) before swapping it in, caches it next to `config.json`, and offers a Reset that restores the built-in list. A parity test keeps the JSON and the compile-time list identical.
- Catalog v7 (2026-11-14): six verified additions across previously missing families — gpt-oss 20B (MXFP4 MoE), DeepSeek-R1-0528 Qwen3 8B, Mistral Small 3.2 24B, Llama 3.2 3B, Phi-4-mini 3.8B, and the BGE-M3 embedding model. A permanent `catalog-pins` CI job verifies every sha256 and size against the Hugging Face API.
- Catalog v8 (2026-11-14, operator review): the six v7 additions were 2024-2025 generations and were replaced with the newest verified models per role — Spark-X2.6 4B and 1.7B (October 2026 on-device agent models), GLM-5.3 Flash (320B-total/18B-active MoE with vision), Mistral Small 4 (119B MoE, native MXFP4, vision), and Nomic Embed v2 MoE. Split-GGUF support: a catalog entry can now name its primary file and carry shard companions, so multi-shard llama.cpp models download and load end to end; the pin verifier checks shard totals.
- Companion downloads fixed: vision projector (`mmproj`) files now download with their model, and the Models screen shows a Vision badge plus kind filters (Chat/Code/Embedding/Reranker). Deleting a model also deletes its companions.
- Semantic and hybrid retrieval: a local embedding engine (a second `llama-server --embedding` instance) vectorizes library chunks at ingest and powers Semantic/Hybrid search modes with reciprocal-rank fusion; memory recall and chat RAG embed queries too. Includes a one-click backfill for previously indexed chunks and a truthful `embedding_status`.
- Skills: reusable prompt templates as local markdown files (`~/.aiassistant/skills/`) with a full editor screen; typing `/` in chat opens a skill picker. Custom prompts reused three or more times surface a local "save as skill?" suggestion — heuristics only, nothing leaves the machine.
- Persisted chat preferences: system prompt, sampling temperature, response cap, and persona survive restarts, with a context-window slider in Settings.
- Engine context is now a setting (default 8192 tokens) and is capped per model by the catalog's native context hint, replacing the hard-wired 2048.
- Diagnostics: a Settings → Diagnostics tab with local subsystem health, a llama-server log viewer, and a display-only update check against GitHub Releases (the app never downloads anything by itself).
- Encrypted local backups (ADR-0009): one AES-256-GCM archive (`.labak`) of config, memory notes, and conversations under an argon2id-derived key from a passphrase that is never stored. Local-folder targets ship working (including desktop-synced OneDrive/Drive folders, with zero OAuth); provider API keys are excluded by default; restore verifies the passphrase before touching any file and never clobbers a newer config. Cloud provider targets are deferred until operator-registered OAuth clients exist.

### Changed
- Toolchain currency pass: Node 20 → 24 (removed from GitHub runners 2026-09-23), GitHub Actions checkout/setup-node v4 → v5, Vite 6 → 8, vite-plugin-svelte 5 → 7, vite-plugin-checker 0.8 → 1.0, svelte-check 4.1 → 4.7, TypeScript → 5.9, notify 6 → 8, and the llama.cpp sidecar pin v0.9.0 → v1.0.0 (the semver milestone). Unused `autoprefixer`/`postcss` devDependencies were removed and an `engines` field pins Node ≥ 24.
- Platform builds (sidecar, macOS/Windows/Linux bundles) now run only for release tags, manual dispatch, or commits marked `[build-all]` — routine pushes verify with the quality gate alone, honoring the macOS 10× runner cost.

## [0.1.0] - 2026-11-13

First release on `main`, the branch of record. The earlier `v0.0.1` GitHub release and its tag were deleted on 2026-11-13 as superseded by this version. macOS/package bundling is the one CI stage not yet verified: the macOS sidecar builds were cancelled twice to save runner cost (a cancelled job, not a failed one).

### Added
- llama-server sidecar engine (ADR-0007): chat runs in a llama.cpp `llama-server` child process on `127.0.0.1`, built from the pinned llama.cpp tag v0.9.0 by CI (`.github/workflows/llama-server.yml`) and bundled via the `tauri.sidecar.conf.json` overlay.
- Image input for vision models: attach up to 4 images; the sidecar loads the model's `mmproj` projector and streams image content parts to the server.
- Opt-in Online Providers mode: `RouterEngine` routes between the local sidecar and a new `RemoteEngine` for OpenAI-compatible providers configured in Settings with the user's own API key. Base URLs must be HTTPS; plain HTTP is accepted only for loopback proxies. Off by default, and a cloud badge in the chat header shows while it is active.
- Kiroween 2026 work merged to `main` (`bb0d0aa`): spec-driven engine upgrade and design pass (`.kiro/specs/kiroween-2026/`), a 36-model catalog verified against Hugging Face, the folder-indexing fix, and the stitched-ghost design tokens.

### Changed
- Documentation truth passes: steering docs, ADRs, README, PACKAGING, scope, and evidence files aligned with the shipped code (chunk sizes, search honesty, catalog digest, sidecar build steps).

### Removed
- Dead code: the in-process llama_cpp_rs engine and its vendored fork, the legacy tantivy/sled indexer, the old top-level watcher/embeddings/voice/components modules, and the obsolete sandbox path (the jail uses `std::fs::canonicalize` plus lexical checks).

## Earlier milestones

Recorded here because these milestones were never published as releases under the version headings they originally carried.

- **2026-10-30 planning pass** (originally headed `0.2.0-alpha`): scope contract and backlog (`docs/scope.md`, `docs/backlog.md`), ADR-0001 through ADR-0006, the rubric scorecard, and the PR template.
- **Initial prototype** (originally headed `0.1.0`): Tauri v2 + Svelte 5 + Tailwind scaffold, a model-download scaffold, and a Tantivy/Sled search prototype, since replaced by SQLite FTS5 (ADR-0002).
