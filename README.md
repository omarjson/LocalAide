# LocalAide

<div align="center">

**A private, local-first desktop AI assistant**

*Local by default. Zero telemetry. Opt-in cloud, including free providers that need no key.*

[![CI & Release Pipeline](https://github.com/omarjson/LocalAide/actions/workflows/ci.yml/badge.svg)](https://github.com/omarjson/LocalAide/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-v0.1.1-blue.svg)](https://github.com/omarjson/LocalAide/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust: 2024](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](https://www.rust-lang.org/)
[![Tauri: v2](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app/)
[![Svelte: 5](https://img.shields.io/badge/Svelte-5%20(Runes)-FF3E00.svg)](https://svelte.dev/)
[![SQLite: WAL](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57.svg)](https://www.sqlite.org/)
[![Accessibility: WCAG 2.2 AA target](https://img.shields.io/badge/Accessibility-WCAG%202.2%20AA%20target-purple.svg)](docs/evidence/a11y.md)

[Download Releases](https://github.com/omarjson/LocalAide/releases) • [Architecture](ARCHITECTURE.md) • [Security Policy](SECURITY.md) • [Release Guide](docs/release.md) • [Project Map](docs/evidence/project-map.html) • [Evidence Index](docs/evidence/scorecard.md)

</div>

---

**Status:** `main` is the branch of record. CI checks every branch: rustfmt, clippy with `-D warnings` and `cargo test` on Ubuntu and on Windows, svelte-check, the frontend build, the smoke test, the catalog pins, the version numbers and the third-party licenses. Platform builds (macOS Apple Silicon and Intel, Windows, Linux) run for tags and `[build-all]` commits. The production-readiness work and what is still open are tracked in [docs/evidence/audit-2026-11-14.md](docs/evidence/audit-2026-11-14.md). The installers have not been run on real machines yet.

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

LocalAide is an open-source, local-first desktop AI assistant built on Tauri v2, Rust, Svelte 5, and SQLite. It runs quantized open-weights models (GGUF) through a `llama-server` sidecar process built from llama.cpp v1.0.0, talking to it over `127.0.0.1` — there is no browser runtime and no bundled Electron shell.

With cloud mode off — the default — it runs fully offline and collects no telemetry. Documents, conversations, and memory notes stay under `~/.aiassistant/` on your machine. If you add an online provider in Settings and switch cloud mode on, chat requests go only to that provider over HTTPS with your own API key, and the chat header shows a cloud badge the whole time.

Search is BM25 keyword search over SQLite FTS5 by default; with the local embedding engine running, semantic and hybrid retrieval with reciprocal-rank fusion rank by vector similarity, and with the local reranker engine running too, the top fused candidates are re-ordered by a true reranking model. See [ARCHITECTURE.md](ARCHITECTURE.md) for how each subsystem works.

## Key Features

- **Local chat**: token streaming with a working Stop control and `Esc` (cancellation applies at the next streamed token), conversation history in JSON thread files, and message actions (retry, edit-and-resend, copy, delete thread).
- **Image input with vision models**: attach up to 4 images (PNG, JPEG, WEBP, GIF, 20 MB total); the sidecar loads the model's `mmproj` projector when one is present and rejects image input for text-only models.
- **Opt-in online providers**: add an OpenAI-compatible provider with your own API key, or start from a preset list aggregated from the open 9Router registry — LLM7, Synthetic, Groq, Cerebras, NVIDIA NIM, OpenRouter, Chutes, Mistral, api.airforce, plus your own local Ollama (no key needed for proxies on this machine). Off by default; the key is stored in the local config file, masked in the UI, and sent only to the provider's base URL.
- **Tools the model chooses, visible thinking, one model menu, MCP** ([ADR-0012](docs/adr/0012-chat-tools-and-mcp.md), 0.1.4): in chat the model decides when to search the web, read a page, search your notes, use a skill or read workspace files; each call shows as a card, and writes ask first. Thinking models show a collapsible "Thought for N s" block with an effort setting. The model menu lists local models and every online provider you switch on, with thinking, image and tool icons and the context size. Attach or drop files, and add MCP servers (local programs or web addresses) in Settings → Tools and MCP. Not yet driven against a real tool-calling model or MCP server; see the ADR.
- **Web search & page reading** ([ADR-0011](docs/adr/0011-web-search.md)): the Search screen's Web mode searches the open internet. DuckDuckGo, Wikipedia, and the Jina Reader work keylessly out of the box; Exa and Firecrawl run after you paste an API key in Settings → Web search. Every request is an explicit click — nothing crawls in the background — and keys are stored locally, sent only to their own provider, and excluded from backups unless you opt in.
- **Sliding-window context budget**: older turns are pruned when the conversation exceeds the model's context, preserving the system prompt, the retrieved context block, and the latest message.
- **Document library**: index folders of Markdown, text, source, and PDF files; chunking follows natural boundaries (1500 characters, 250 overlap); search runs FTS5 BM25 keyword matching — or semantic and hybrid retrieval with reciprocal-rank fusion when the local embedding engine is running, re-ranked by a local reranker model when that engine is running too, with a one-click backfill for existing indexes.
- **Skills**: reusable prompts saved as local markdown files with a full editor; type `/` in chat to pull one in. Custom prompts you keep re-typing surface a local save-as-skill suggestion (usage counts stay on this machine).
- **Memory notes with citations**: Markdown notes in `~/.aiassistant/memory/` are indexed into SQLite; chat recalls notes above a relevance score of 0.010 and renders clickable citation pills with the snippet, path, and score.
- **Capability-rooted workspace jail**: file tools are confined to `~/.aiassistant/workspace/`. Paths are checked lexically and canonically; traversal escapes, outside symlinks, and symlink loops are rejected with `SANDBOX_DENIED`.
- **Explicit tool consent**: reads, listings, and searches run without prompts; file writes require an approval modal ("Allow Once", "Allow for Session", "Deny"). Every tool call lands in a SQLite audit log you can view and clear in Settings.
- **Verified model downloads**: a 41-model curated catalog with an integrity digest over ids, sizes, and hashes, plus a community browser that can download any public GGUF from the Hugging Face hub ([ADR-0010](docs/adr/0010-community-models.md)); resumable HTTP downloads to `.part` files (vision projector files and split-GGUF shards download with their model); SHA-256 verified in streaming 64 KB buffers before the file is renamed into place.
- **Catalog refresh, fail-closed**: the Models screen can fetch the newest catalog from this repository over HTTPS ([ADR-0008](docs/adr/0008-catalog-refresh.md)). Refreshes are explicit, validated structurally, checked against the integrity digest, and blocked from rolling back to an older version; Reset restores the list compiled into the app. A CI job verifies every catalog pin against the Hugging Face API.
- **Encrypted backups**: one `.labak` archive of your config, memory notes, and conversations under AES-256-GCM with an argon2id passphrase that is never stored ([ADR-0009](docs/adr/0009-encrypted-backups.md)). The archive is a plain portable file — upload it wherever you like (a OneDrive or Drive sync folder gives free off-machine redundancy with no cloud login in the app), and the built-in restore verifies your passphrase before touching any file and never clobbers a newer config. Provider API keys are excluded by default.
- **Keyboard-complete navigation**: see the matrix below.
- **Accessibility**: WCAG 2.2 AA is the target. Text and border contrast are audited against the design tokens ([docs/evidence/a11y.md](docs/evidence/a11y.md)) — meaningful borders now clear 3:1 through a dedicated `edge` ramp in both themes, and the on-screen re-measure is the open item. Focus rings, `aria-live` streaming, and reduced-motion support are in place.
- **Dark and light themes**: the stitched-ghost dark palette is the designed default; a light theme (bone paper, darkened accents, same semantic tokens) can be switched in Settings and persists. The light theme has not yet been eyeballed on a real screen — dark remains the reference.

## Hardware Requirements & Recommended Models

LocalAide runs GGUF models with llama.cpp v1.0.0. The built-in catalog lists 41 models across late-2026 families — Spark-X2.6 (October 2026 on-device agents), GLM-5.3 Flash, Mistral Small 4, Qwen 3.5/3.6/3.8, Gemma 4, Granite 4.x, LFM 2.5/3, plus embedding (EmbeddingGemma, Nomic Embed v2 MoE, Qwen3-Embedding) and reranking models. Each entry was checked against Hugging Face; file sizes and SHA-256 values come from the hub's metadata, and each license is shown in the Model Manager. The catalog can also be refreshed from this repository without updating the app — see the Models screen and [ADR-0008](docs/adr/0008-catalog-refresh.md).

| Preset | RAM needed | Models |
|---|---|---|
| Tiny | about 1 GB | Qwen3.5 0.8B |
| Small | about 4 GB | Qwen3.5 2B, Gemma 4 E2B (QAT) |
| Balanced | about 9.5 GB | Qwen3.5 9B, Gemma 4 E4B (QAT) |
| Advanced | about 20 GB | Gemma 4 26B-A4B (MoE), Qwen3.8 27B |

Minimum hardware: a 64-bit x64 or ARM64 processor; 8 GB RAM (16 GB for 3B–7B models); 2 GB of SSD space for the app plus 0.15–22 GB per model, depending on the entry.

Full list, sources, and rejected models: [docs/evidence/models-2026-11.md](docs/evidence/models-2026-11.md).

## Getting Started

### Option A: download precompiled binaries

Grab a package from the [Releases page](https://github.com/omarjson/LocalAide/releases):

- **macOS (Apple Silicon)**: `LocalAide_<version>_aarch64.dmg`
- **macOS (Intel)**: `LocalAide_<version>_x64.dmg`
- **Windows (x64)**: the installer ending in `_x64-setup.exe`
- **Windows on ARM**: the installer ending in `_arm64-setup.exe`
- **Linux (x64)**: the `.deb` package or the `.AppImage`

The first time a model loads, LocalAide downloads the llama.cpp engine built for your processor (about 13 to 25 MB, from github.com). Any 64-bit x64 or ARM64 processor works.

**Check your download.** Each release has `SHA256SUMS.txt`. On Linux or macOS run `sha256sum -c SHA256SUMS.txt --ignore-missing` in the download folder. On Windows run `Get-FileHash <file>` in PowerShell and compare the result with the line for that file.

**Warnings on first start.** Release builds are not code-signed yet, so the system warns before the first start:

- **Windows**: SmartScreen says "Windows protected your PC". Choose **More info**, then **Run anyway**.
- **macOS**: Gatekeeper says the app cannot be checked. Open **System Settings → Privacy & Security**, then choose **Open Anyway** for LocalAide.

**Updates.** Open **Settings → About → Check for updates**. When a new version exists, LocalAide shows what changed and offers **Install and restart**: it downloads the update, checks its signature, installs it and keeps your data. Nothing is checked unless you press the button. Builds made without the update key (see `docs/release.md`) link to the download page instead.

**Legal.** The [Privacy Policy](PRIVACY.md) and [Terms of Use](TERMS.md) ship inside the app (Settings → About → Legal).

How releases are built, and what signing needs, is in [docs/release.md](docs/release.md).

### Option B: build from source

1. **Prerequisites**: Node.js 24+ (`nvm use 24`), a Rust stable toolchain with `rustfmt` and `clippy` (`rustup update stable`; the crate declares MSRV 1.90), and a C/C++ compiler:
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
5. **Build a package**: the app chats through a local `llama-server` (ADR-0007), which it downloads for the user's processor on first use (ADR-0012). Write the license file, then build:
   ```bash
   (cd src-tauri && cargo about generate about.hbs > ../THIRD_PARTY_LICENSES.html)
   make release
   ```
   `make release` passes the `src-tauri/tauri.release.conf.json` overlay, which bundles the license file. Artifacts land in `src-tauri/target/release/bundle/`. The full steps are in [docs/release.md](docs/release.md).

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
| `Cmd/Ctrl+Shift+M` | Open the model menu in Chat |

## Verification & Quality Gates

CI runs on every push: rustfmt, `clippy --all-targets -- -D warnings`, the full `cargo test --all-targets` suite (unit, contract, fault-injection, inference, library, memory/RAG, and tools/sandbox tests) on Ubuntu and Windows, `svelte-check`, the frontend production build, the smoke test, the version gate and the license check. A weekly job audits `Cargo.lock` and the npm runtime packages for known vulnerabilities, and Dependabot proposes updates. Release builds compile for macOS (Apple Silicon and Intel), Windows x64, and Linux x64, each bundling the matching `llama-server` sidecar, and go to a draft release with checksums and a software bill of materials.

Design and honesty are tracked against the rubric in [docs/evidence/scorecard.md](docs/evidence/scorecard.md). Known open items, kept visible on purpose:

- Border contrast is fixed at the token level (a dedicated `edge` ramp clears 3:1 in both themes), but nothing has been re-measured on a real screen yet ([docs/evidence/a11y.md](docs/evidence/a11y.md)).
- The installers (`.dmg`/`-setup.exe`/`.deb`/`.AppImage`) have not been run on real machines yet (demo-run open item). They are not code-signed yet ([docs/release.md](docs/release.md)).
- The design pass has not been checked on screen ([DESIGN.md](DESIGN.md)) — including the new light theme.
- Catalog signing with Ed25519 is future work; today's catalog check is an integrity digest, and refreshed catalogs are validated structurally with anti-rollback ([ADR-0005](docs/adr/0005-updates.md), [ADR-0008](docs/adr/0008-catalog-refresh.md)).

## Scope & Non-Goals

LocalAide keeps a strict scope. What is in and out, with reasons, lives in [docs/scope.md](docs/scope.md). In brief:

- **Shipped in 0.1.0**: local GGUF chat via the sidecar, image input with vision models, the model catalog and manager (plus the HF community browser, also in the first-run wizard), the SQLite WAL document library with keyword/semantic/hybrid search and the reranker second stage, Markdown memory notes with RAG citations, local skills, encrypted backups, safe file tools in the workspace jail, opt-in online providers with 9Router-derived presets, and keyless-first web search with page reading ([ADR-0011](docs/adr/0011-web-search.md)).
- **Deferred**: speech (STT/TTS) — it needs a second runtime (`.kiro/specs/engine-upgrade/tasks.md`); vision runtime verification — image input is wired but has not been exercised against a real vision model end to end.
- **Non-goals**: telemetry of any kind, silent cloud failover, and in-app component hot-swapping (updates ship as whole signed packages, ADR-0005).

## Architecture Decision Records (ADRs)

- [ADR-0001: Typed IPC Boundaries](docs/adr/0001-typed-ipc.md)
- [ADR-0002: Storage Engine Migration to SQLite WAL](docs/adr/0002-storage.md)
- [ADR-0003: Isolated llama.cpp Worker Engine (superseded by ADR-0007)](docs/adr/0003-inference.md)
- [ADR-0004: Capability Workspace Jail](docs/adr/0004-sandbox.md)
- [ADR-0005: Cryptographic Verification of App Updates and Model Catalog](docs/adr/0005-updates.md)
- [ADR-0006: Feature Gating & Deferred Scopes](docs/adr/0006-optional-features.md)
- [ADR-0007: llama-server Sidecar for Chat](docs/adr/0007-sidecar-engine.md)
- [ADR-0012: Model engine downloaded for the processor](docs/adr/0012-engine-download.md)
- [ADR-0008: Model Catalog Refresh](docs/adr/0008-catalog-refresh.md)
- [ADR-0009: Encrypted Local Backups](docs/adr/0009-encrypted-backups.md)
- [ADR-0010: Community Models from the Hugging Face Hub](docs/adr/0010-community-models.md)
- [ADR-0011: Web Search and Page Reading](docs/adr/0011-web-search.md)
- [ADR-0012: Chat Tools, Thinking, the Model Picker and MCP](docs/adr/0012-chat-tools-and-mcp.md)

## Security Disclosure

Found a security vulnerability? Follow the [Security Policy](SECURITY.md) and report it privately — do not open a public issue.

## License

LocalAide is open-source software licensed under the [MIT License](LICENSE).
Copyright (c) 2026 Omar. Bundled third-party software is listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
