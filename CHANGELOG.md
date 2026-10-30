# Changelog

All notable changes to LocalAide will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
