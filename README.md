# LocalAide

<div align="center">

**A private, local-first desktop AI assistant**

*Local by default. Zero telemetry. Opt-in cloud with your own key.*

[![CI & Release Pipeline](https://github.com/omarjson/LocalAide/actions/workflows/ci.yml/badge.svg)](https://github.com/omarjson/LocalAide/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-v0.1.0-blue.svg)](https://github.com/omarjson/LocalAide/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust: 2024](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](https://www.rust-lang.org/)
[![Tauri: v2](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app/)
[![Svelte: 5](https://img.shields.io/badge/Svelte-5%20(Runes)-FF3E00.svg)](https://svelte.dev/)
[![SQLite: WAL](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57.svg)](https://www.sqlite.org/)
[![Accessibility: WCAG 2.2 AA target](https://img.shields.io/badge/Accessibility-WCAG%202.2%20AA%20target-purple.svg)](docs/evidence/a11y.md)

[Download Releases](https://github.com/omarjson/LocalAide/releases) • [Architecture](ARCHITECTURE.md) • [Security Policy](SECURITY.md) • [Packaging Guide](PACKAGING.md) • [Project Map](docs/evidence/project-map.html) • [Evidence Index](docs/evidence/scorecard.md)

</div>

---

**Status:** `main` is the branch of record. The CI quality gate (rustfmt, clippy, cargo test, svelte-check, frontend build, smoke test) is green on `main`. macOS/package bundling is the one CI stage never verified: the macOS sidecar builds were cancelled twice to save runner cost, and a cancelled job is not a failed one, so no package has been proven to install and run. Everything else in this README is matched to the code and re-checked against the standing honesty audit in [docs/evidence/audit-2026-11-13.md](docs/evidence/audit-2026-11-13.md).

## Table of Contents

- [What is LocalAide?](#what-is-localaide)
- [Key Features](#key-features)
- [Hardware Requirements & Models](#hardware-requirements--recommended-models)
- [Getting Started](#getting-started)
- [Keyboard Navigation](#keyboard-navigation)
- [Verification & Quality Gates](#verification--quality-gates)
- [Scope & Non-Goals](#scope--non-goals)
- [Architecture Decision Records](#architecture-decision-records-adrs)
- [Security Disclosure](#security-disclosure)
- [License](#license)

## What is LocalAide?

LocalAide is an open-source, local-first desktop AI assistant built on Tauri v2, Rust, Svelte 5, and SQLite. It runs quantized open-weights models (GGUF) through a `llama-server` sidecar process built from llama.cpp v0.9.0, talking to it over `127.0.0.1` — there is no browser runtime and no bundled Electron shell.

With cloud mode off — the default — it runs fully offline and collects no telemetry. Documents, conversations, and memory notes stay under `~/.aiassistant/` on your machine. If you add an online provider in Settings and switch cloud mode on, chat requests go only to that provider over HTTPS with your own API key, and the chat header shows a cloud badge the whole time.

Search is BM25 keyword search over SQLite FTS5. Vector search and rank fusion are not wired in this release. See [ARCHITECTURE.md](ARCHITECTURE.md) for how each subsystem works.

## Key Features

- **Local chat**: token streaming with a working Stop control and `Esc` (cancellation applies at the next streamed token), conversation history in JSON thread files, and message actions (retry, edit-and-resend, copy, delete thread).
- **Image input with vision models**: attach up to 4 images (PNG, JPEG, WEBP, GIF, 20 MB total); the sidecar loads the model's `mmproj` projector when one is present and rejects image input for text-only models.
- **Opt-in online providers**: add an OpenAI-compatible provider (OpenAI, OpenRouter, a localhost proxy such as Ollama or LM Studio) with your own API key. Off by default; the key is stored in the local config file, masked in the UI, and sent only to the provider's base URL.
- **Sliding-window context budget**: older turns are pruned when the conversation exceeds the model's context, preserving the system prompt, the retrieved context block, and the latest message.
- **Document library**: index folders of Markdown, text, source, and PDF files; chunking follows natural boundaries (1500 characters, 250 overlap); search runs FTS5 BM25 keyword matching.
- **Memory notes with citations**: Markdown notes in `~/.aiassistant/memory/` are indexed into SQLite; chat recalls notes above a relevance score of 0.010 and renders clickable citation pills with the snippet, path, and score.
- **Capability-rooted workspace jail**: file tools are confined to `~/.aiassistant/workspace/`. Paths are checked lexically and canonically; traversal escapes, outside symlinks, and symlink loops are rejected with `SANDBOX_DENIED`.
- **Explicit tool consent**: reads, listings, and searches run without prompts; file writes require an approval modal ("Allow Once", "Allow for Session", "Deny"). Every tool call lands in a SQLite audit log you can view and clear in Settings.
- **Verified model downloads**: a 36-model catalog with an integrity digest over ids, sizes, and hashes; resumable HTTP downloads to `.part` files; SHA-256 verified in streaming 64 KB buffers before the file is renamed into place.
- **Catalog refresh, fail-closed**: the Models screen can fetch the newest catalog from this repository over HTTPS ([ADR-0008](docs/adr/0008-catalog-refresh.md)). Refreshes are explicit, validated structurally, checked against the integrity digest, and blocked from rolling back to an older version; Reset restores the list compiled into the app.
- **Keyboard-complete navigation**: see the matrix below.
- **Accessibility**: WCAG 2.2 AA is the target. Text contrast is audited against the design tokens ([docs/evidence/a11y.md](docs/evidence/a11y.md)); one open finding remains (border contrast), and focus rings, `aria-live` streaming, and reduced-motion support are in place.

## Hardware Requirements & Recommended Models

LocalAide runs GGUF models with llama.cpp v0.9.0. The built-in catalog lists 36 models, including vision, embedding, reranking, and coding models. Each entry was checked against Hugging Face; file sizes and SHA-256 values come from the hub's metadata, and each license is shown in the Model Manager. The catalog can also be refreshed from this repository without updating the app — see the Models screen and [ADR-0008](docs/adr/0008-catalog-refresh.md).

| Preset | RAM needed | Models |
|---|---|---|
| Tiny | about 1 GB | Qwen3.5 0.8B |
| Small | about 4 GB | Qwen3.5 2B, Gemma 4 E2B (QAT) |
| Balanced | about 9.5 GB | Qwen3.5 9B, Gemma 4 E4B (QAT) |
| Advanced | about 20 GB | Gemma 4 26B-A4B (MoE), Qwen3.8 27B |

Minimum hardware: x86_64 CPU with AVX2 or Apple Silicon; 8 GB RAM (16 GB for 3B–7B models); 2 GB of SSD space for the app plus 0.15–22 GB per model, depending on the entry.

Full list, sources, and rejected models: [docs/evidence/models-2026-11.md](docs/evidence/models-2026-11.md).

## Getting Started

### Option A: download precompiled binaries

Grab a package from the [Releases page](https://github.com/omarjson/LocalAide/releases):

- **macOS (Apple Silicon)**: `LocalAide_<version>_aarch64.dmg`
- **macOS (Intel)**: `LocalAide_<version>_x64.dmg`
- **Windows (x64)**: `LocalAide_<version>_x64_en-US.msi` or the NSIS installer (`.exe`)
- **Linux (x64)**: `LocalAide_<version>_amd64.deb` or `LocalAide_<version>_amd64.AppImage`

Note the status line above: the macOS packages come from a CI stage that has not yet completed, so treat them as unverified until a run proves them.

### Option B: build from source

1. **Prerequisites**: Node.js 20+ (`nvm use 20`), a Rust stable toolchain with `rustfmt` and `clippy` (`rustup update stable`; the crate declares MSRV 1.90), and a C/C++ compiler:
   - macOS: Xcode Command Line Tools (`xcode-select --install`)
   - Linux: `sudo apt install build-essential libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev`
   - Windows: Visual Studio 2022 C++ Build Tools (MSVC)
2. **Clone and install**:
   ```bash
   git clone https://github.com/omarjson/LocalAide.git
   cd LocalAide
   npm ci
   ```
3. **Run the local checks**:
   ```bash
   make all   # check (svelte-check + cargo check), test (cargo test), smoke (build + smoke test)
   make lint  # clippy, as the CI quality gate runs it
   ```
4. **Run in development**:
   ```bash
   npx tauri dev   # or `npm run dev` for the frontend alone
   ```
5. **Build a package**: the app chats through a local `llama-server` sidecar (ADR-0007), so the package must bundle that binary next to the app executable. Build it from the pinned llama.cpp tag (v0.9.0) and stage it with `scripts/prepare-sidecar.sh`, or let CI do both — see [.github/workflows/llama-server.yml](.github/workflows/llama-server.yml) and [PACKAGING.md](PACKAGING.md). Then:
   ```bash
   npx tauri build --config src-tauri/tauri.sidecar.conf.json
   ```
   The overlay adds `bundle.externalBin`. A bare `npx tauri build` — including `make release`, which passes the overlay for you — produces an app that cannot chat without it. Artifacts land in `src-tauri/target/release/bundle/`.

## Keyboard Navigation

| Key | Action |
|---|---|
| `Cmd/Ctrl+K` | Command palette |
| `Cmd/Ctrl+N` | New conversation |
| `/` | Focus library search (outside inputs) |
| `Cmd/Ctrl+,` | Settings |
| `Cmd/Ctrl+1` … `5` | Chat, Search, Memory, Models, Settings |
| `Esc` | Abort generation / close modal |
| `?` | Keyboard shortcuts help (outside inputs) |
| `Enter` / `Shift+Enter` | Send / newline in chat input |

## Verification & Quality Gates

CI runs on every push to `main`: rustfmt, `clippy --all-targets -- -D warnings`, the full `cargo test --all-targets` suite (unit, contract, fault-injection, inference, library, memory/RAG, and tools/sandbox tests), `svelte-check`, the frontend production build, and the smoke test. Release builds compile for macOS (Apple Silicon and Intel), Windows x64, and Linux x64, each bundling the matching `llama-server` sidecar.

Design and honesty are tracked against the rubric in [docs/evidence/scorecard.md](docs/evidence/scorecard.md). Known open items, kept visible on purpose:

- Border contrast fails the 3:1 non-text threshold ([docs/evidence/a11y.md](docs/evidence/a11y.md)).
- The design pass has not been checked on screen ([DESIGN.md](DESIGN.md)).
- macOS/package bundling is unverified in CI (see the status line).
- Catalog signing with Ed25519 is future work; today's catalog check is an integrity digest, and refreshed catalogs are validated structurally with anti-rollback ([ADR-0005](docs/adr/0005-updates.md), [ADR-0008](docs/adr/0008-catalog-refresh.md)).

## Scope & Non-Goals

LocalAide keeps a strict scope. What is in and out, with reasons, lives in [docs/scope.md](docs/scope.md). In brief:

- **Shipped in 0.1.0**: local GGUF chat via the sidecar, image input with vision models, the model catalog and manager, the SQLite WAL document library with keyword search, Markdown memory notes with RAG citations, safe file tools in the workspace jail, and opt-in online providers.
- **Deferred**: speech (STT/TTS) — it needs a second runtime (`.kiro/specs/engine-upgrade/tasks.md`); vision runtime verification — image input is wired but has not been exercised against a real vision model end to end.
- **Non-goals**: telemetry of any kind, silent cloud failover, and in-app component hot-swapping (updates ship as whole packages; updater signing is future work per ADR-0005).

## Architecture Decision Records (ADRs)

- [ADR-0001: Typed IPC Boundaries](docs/adr/0001-typed-ipc.md)
- [ADR-0002: Storage Engine Migration to SQLite WAL](docs/adr/0002-storage.md)
- [ADR-0003: Isolated llama.cpp Worker Engine (superseded by ADR-0007)](docs/adr/0003-inference.md)
- [ADR-0004: Capability Workspace Jail](docs/adr/0004-sandbox.md)
- [ADR-0005: Atomic Updates & Verification](docs/adr/0005-updates.md)
- [ADR-0006: Feature Gating & Deferred Scopes](docs/adr/0006-optional-features.md)
- [ADR-0007: llama-server Sidecar for Chat](docs/adr/0007-sidecar-engine.md)

## Security Disclosure

Found a security vulnerability? Follow the [Security Policy](SECURITY.md) and report it privately — do not open a public issue.

## License

LocalAide is open-source software licensed under the [MIT License](LICENSE).
Copyright © 2026 LocalAide Contributors.
