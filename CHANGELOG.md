# Changelog

All notable changes to LocalAide will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- llama-server sidecar engine (ADR-0007): chat runs in a llama.cpp `llama-server` child process on 127.0.0.1, built from the pinned llama.cpp tag by CI (`.github/workflows/llama-server.yml`) and bundled via the `tauri.sidecar.conf.json` overlay.
- Opt-in Online Providers mode: `RouterEngine` routes between the local sidecar and the new `RemoteEngine` for OpenAI-compatible providers the user configures in Settings with their own API key. Base URLs must be HTTPS; plain HTTP is accepted only for loopback proxies (127.0.0.1 / localhost). Off by default; local inference remains the default, and a cloud badge shows when cloud mode is active.
- Kiroween 2026 branch work: spec-driven engine upgrade and design pass (`.kiro/specs/kiroween-2026/`), 36-model verified catalog, folder-indexing fix, and the stitched-ghost design tokens.

### Changed
- Documentation truth passes: steering docs, ADRs, README, PACKAGING, scope, and evidence files aligned with the shipped code (chunk sizes, search honesty, catalog digest, sidecar build steps).

### Removed
- Dead code: the in-process llama_cpp_rs engine and its vendored fork, the legacy tantivy/sled indexer, the old top-level watcher/embeddings/voice/components modules, and the obsolete sandbox path (jail uses `std::fs::canonicalize` + lexical checks).

## [0.2.0-alpha] - 2026-10-30
### Added
- Comprehensive 10/10 Build Plan and Contract (`docs/scope.md`, `docs/backlog.md`).
- Architectural Decision Records:
  - ADR-0001: Typed IPC Boundary
  - ADR-0002: Unified Storage Engine with SQLite WAL
  - ADR-0003: Model Inference Architecture
  - ADR-0004: Workspace Jail Sandbox
  - ADR-0005: Cryptographic Verification of App Updates & Catalog
  - ADR-0006: Feature Gating & Deferrals
- Rubric scorecard tracking (`docs/evidence/scorecard.md`).
- PR template with Definition of Done checklist.

### Changed
- Truthful README status matrix distinguishing Shipped, Experimental, Planned, and Removed features.
- Deferred voice, vision, cloud LLM, and modular hot-swapping stubs until post-v1.0 gates pass.

## [0.1.0] - Initial Prototype
- Initial application scaffold using Tauri v2, Svelte 5, and Tailwind CSS.
- Basic model manager download scaffold.
- Initial Tantivy and Sled search prototype.
