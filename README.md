# LocalAide

<div align="center">

**The Private, Local-First Desktop AI Assistant**

*Zero cloud subscriptions. Zero telemetry. 100% on-device inference.*

[![CI & Release Pipeline](https://github.com/omarjson/LocalAide/actions/workflows/ci.yml/badge.svg)](https://github.com/omarjson/LocalAide/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-v0.1.0-blue.svg)](https://github.com/omarjson/LocalAide/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust: 2024](https://img.shields.io/badge/Rust-2024%20Edition-orange.svg)](https://www.rust-lang.org/)
[![Tauri: v2](https://img.shields.io/badge/Tauri-v2-24C8DB.svg)](https://tauri.app/)
[![Svelte: 5](https://img.shields.io/badge/Svelte-5%20(Runes)-FF3E00.svg)](https://svelte.dev/)
[![SQLite: WAL](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57.svg)](https://www.sqlite.org/)
[![Accessibility: WCAG 2.2 AA](https://img.shields.io/badge/Accessibility-WCAG%202.2%20AA-purple.svg)](docs/evidence/a11y.md)

[Download Releases](https://github.com/omarjson/LocalAide/releases) • [Architecture](ARCHITECTURE.md) • [Security Policy](SECURITY.md) • [Packaging Guide](PACKAGING.md) • [Audit & Evidence](docs/evidence/audit-2026-11-13.md)

</div>

---

## Table of Contents

- [What is LocalAide?](#what-is-localaide)
- [Why LocalAide? (Cloud vs Local)](#why-localaide-the-case-for-local-first-desktop-ai)
- [Key Features](#key-features)
- [Technology Stack](#technology-stack)
- [System Architecture](#system-architecture)
- [How Each Subsystem Works](#how-each-subsystem-works)
  - [1. Inference Engine & Token Budgeting](#1-inference-engine--token-budgeting)
  - [2. Document Library & Keyword Search (FTS5)](#2-document-library--keyword-search-fts5)
  - [3. Memory Notes & Verifiable Citations](#3-memory-notes--verifiable-citations)
  - [4. Capability Sandbox & Tool Approval](#4-capability-sandbox--workspacejail)
  - [5. Model Manager & Cryptographic Verification](#5-model-manager--streaming-verification)
- [Hardware Requirements & Models](#hardware-requirements--recommended-models)
- [Getting Started](#getting-started)
  - [Option A: Download Precompiled Binaries](#option-a-download-precompiled-binaries)
  - [Option B: Building From Source](#option-b-building-from-source)
- [Keyboard Navigation Matrix](#keyboard-navigation-matrix)
- [Verification & Quality Gates](#verification--quality-gates)
- [Roadmap & Scope Policy](#roadmap--scope-policy)
- [Security Disclosure](#security-disclosure)
- [License](#license)

---

## What is LocalAide?

**LocalAide** is an open-source, local-first desktop AI assistant engineered for privacy, reliability, and speed. Built natively on **Rust**, **Tauri v2**, **Svelte 5**, and **SQLite**, it runs quantized open-weights Large Language Models (GGUF format) through a local `llama-server` engine — built from llama.cpp v0.9.0 — directly on your computer's hardware.

Unlike browser wrappers or Electron-based frontends, LocalAide is designed as a standalone, low-overhead native desktop application. It operates **100% offline**, never connects to third-party AI APIs, captures zero user telemetry, and stores all documents, conversation threads, and memory notes strictly on your local filesystem under `~/.aiassistant/`.

---

## Why LocalAide? (The Case for Local-First Desktop AI)

The majority of contemporary AI tools rely on centralized cloud infrastructure. While convenient, cloud-based assistants introduce structural liabilities for developers, legal teams, researchers, and enterprises:

| Risk / Vector | Cloud AI Assistants (OpenAI, Anthropic, Copilot) | LocalAide (Local-First Architecture) |
|---|---|---|
| **Data Privacy & Exfiltration** | Transmits proprietary code, documents, and chat transcripts to remote cloud datacenters. | **Zero Data Exfiltration.** All text processing and inference execute on your machine. |
| **Telemetry & Tracking** | Logs IP addresses, prompt habits, active hours, and hardware fingerprints. | **Zero Telemetry.** No tracking, no analytics, no phone-home pings, no external network requests. |
| **Recurring Costs** | Requires monthly subscription fees (\$20–\$30/user/month). | **Free & Open-Source Forever.** Licensed under MIT; run any open-weights model on your own hardware. |
| **Offline Resilience** | Bricked during internet outages, flight travel, or API outages. | **100% Offline.** Full chat, keyword search, memory recall, and file tools operate air-gapped. |
| **Hallucination Control** | Black-box citations that often invent references or fabricate URLs. | **Verifiable Citations.** Clickable citation pills link directly to local document chunks with exact score inspection. |
| **Filesystem Safety** | Plugins either lack local access or possess unrestricted filesystem access. | **Capability-Rooted Jail.** Strict workspace jail (`openat` checks, symlink defense, interactive user consent). |
| **Memory Footprint** | Bloated Chromium runtimes consuming 1.5–3 GB of idle RAM. | **Ultralight Desktop App.** Native Tauri v2 shell without a bundled browser runtime. |

---

## Key Features

- **Private Local Chat**: Real-time token streaming with a working Stop control (cancellation applies at the next streamed token, or press `Esc`).
- **Sliding-Window Context Budget**: Intelligently packs conversational history, dynamically truncates older turns while preserving the system prompt, and guarantees memory headroom for model responses.
- **Natural Boundary Document Chunker**: Ingests Markdown, plain text, and PDF files. Chunks text along natural paragraph and sentence boundaries with configurable sliding overlap.
- **Keyword Search (FTS5 BM25)**: Embedded SQLite WAL engine running full-text BM25 search over FTS5. Vector search and RRF fusion are not wired in v1.0.
- **Associative Memory with Verifiable Citations**: Bidirectional disk synchronization (`~/.aiassistant/memory/`) with SQLite. Automatically recalls relevant knowledge above a strict relevance gate (`0.010`) and displays clickable citation pills in the chat UI.
- **Capability-Rooted Workspace Jail**: Sandboxed tool execution environment confined to `~/.aiassistant/workspace/`. Traps directory traversal escapes (`..`), external symlinks, and recursive symlink loops (`ELOOP`).
- **Interactive Tool Consent**: Non-destructive operations (file reads, directory listings) are safe by default; state-modifying operations (file writes, executions) trigger an explicit approval modal with audit logging in SQLite.
- **Atomic Model Management**: Catalog manifest with an integrity digest over model ids, sizes, and hashes. Resumable HTTP downloads (`Range: bytes=X-`) and 64 KB constant-RAM streaming SHA-256 cryptographic verification (< 100 KB memory overhead).
- **Universal Keyboard Navigation**: Power-user keyboard matrix (`⌘K` Command Palette, `⌘N` New Chat, `/` Search, `⌘1`–`⌘5` Screen switching, `Esc` Abort, `?` Help).
- **WCAG 2.2 AA Accessibility**: Full contrast compliance (> 4.5:1 text, > 3.0:1 UI borders), universal high-visibility focus indicators (`#d946ef`), and screen-reader `aria-live` streaming announcements.

---

## Technology Stack

LocalAide is built from the ground up with a modern, high-performance systems and frontend stack:

```
┌─────────────────────────────────────────────────────────────┐
│                 FRONTEND USER INTERFACE                     │
│  Svelte 5 (Runes: $state, $derived) • TypeScript 5.7        │
│  Tailwind CSS v4 • Lucide Icons • Vite 6 Bundle Engine      │
└──────────────────────────────┬──────────────────────────────┘
                               │ Typed IPC Client (bindings.ts)
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                   TAURI V2 NATIVE HOST                      │
│  Custom Window Chrome • Strict CSP • System Trays           │
└──────────────────────────────┬──────────────────────────────┘
                               │ Rust Tauri Commands
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                   RUST APPLICATION CORE                     │
│  Rust 2024 Edition • Tokio Async • Single Managed AppState  │
│  JobRegistry (Atomic Cancellation Flags & Progress Tracker) │
└───────┬──────────────────────┬──────────────────────┬───────┘
        │                      │                      │
        ▼                      ▼                      ▼
┌─────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ INFERENCE ENGINE│  │  STORAGE SERVICE │  │ SECURITY SANDBOX │
│ llama-server    │  │ SQLite 3 (WAL)   │  │ WorkspaceJail    │
│ (llama.cpp      │  │ FTS5 BM25 Engine │  │ Canonical Tree   │
│  v0.9.0 sidecar)│  │ Keyword Search   │  │ Audit Logger     │
│ Context Budget  │  │                  │  │                  │
└─────────────────┘  └──────────────────┘  └──────────────────┘
```

- **Core Languages**:
  - **Rust** (2024 Edition / 1.90+): High-concurrency backend services, capability sandbox, SQLite integration, and OS thread orchestration.
  - **TypeScript** (v5.7+): Type-safe IPC client bindings, application models, and frontend contracts.
  - **Svelte** (v5.0+): High-efficiency UI reactive components utilizing modern Svelte 5 Runes without Virtual DOM overhead.
  - **C / C++**: Quantized tensor operations run inside the `llama-server` engine (built from llama.cpp v0.9.0 in CI), communicating with the app over localhost HTTP. Apple Metal and AVX2 vector optimizations come from the upstream engine build.
  - **SQL**: SQLite 3.45+ relational schemas, full-text indexes (FTS5), and transactional migrations.
- **Styling**: Tailwind CSS v4 with custom design tokens, dark mode palette, and WCAG-compliant color contrasts.
- **Data Layer**: `rusqlite` bundled with SQLite WAL mode, eliminating database locks and enabling concurrent background search and chat persistence.

---

## System Architecture

LocalAide adheres to strict architectural boundaries:
1. **Single State Owner**: All mutable domain state is encapsulated in `AppState` managed by Tauri's dependency container.
2. **Typed IPC Edges**: Every command takes a concrete DTO struct and returns a strongly-typed `Result<T, AppError>`. Dynamic JSON is confined to tool arguments and progress event payloads.
3. **Separate Inference Process**: Chat runs in a `llama-server` child process bound to 127.0.0.1 with a random API key. The GUI never waits on the model.
4. **Isolated Capability Jail**: File operations are confined to a designated directory with strict validation of lexical depth and canonical ancestry.

---

## How Each Subsystem Works

### 1. Inference Engine & Token Budgeting
- **Model Loading**: Loading a GGUF file starts `llama-server` with the model and its vision projector, if one is present. Switching or stopping models ends the process.
- **Stream Cancellation**: A cancellation flag is checked per streamed token. Pressing `Esc` or the Stop button halts generation at the next token without corrupting the model state.
- **Dynamic Context Budgeting**: The `ContextBudget` subsystem calculates the model's token limits. If the conversation history exceeds the budget, older turns are pruned while preserving the system instructions, the RAG context block, and the user's latest query.

### 2. Document Library & Keyword Search (FTS5)
- **Natural Boundary Chunker**: Text is partitioned into semantically meaningful chunks (target 1500 characters with 250-character overlap) respecting line breaks, paragraphs, and sentence punctuation.
- **PDF & Markdown Extraction**: Clean, pure-Rust extraction from PDF, Markdown, and text files without external dependencies.
- **Keyword Search (BM25)**: Full-text search runs over SQLite FTS5. Vector search and RRF fusion are not wired in v1.0; the catalog already lists embedding models for when they are.

### 3. Memory Notes & Verifiable Citations
- **Associative Memory Sync**: Personal notes stored in `~/.aiassistant/memory/*.md` are continuously synchronized with SQLite tables.
- **Relevance Gate**: During conversation turns, the `MemoryService` performs a search across stored notes. Only chunks exceeding the strict relevance threshold (`MIN_RELEVANCE_SCORE = 0.010`) are included in the prompt context.
- **Citation Pills**: Retrieved sources are tagged with sequential markers (`[1]`, `[2]`). In the UI, these render as clickable citation pills that open an inspection drawer showing the exact text snippet, file path, and relevance score.
- **Zero-Trace Privacy Purge**: The "Forget All" action permanently deletes memory files from disk and clears the database tables in a single atomic transaction.

### 4. Capability Sandbox (`WorkspaceJail`)
- **Restricted Filesystem Confinement**: Tools are locked to `~/.aiassistant/workspace/`.
- **Traversal Defense**: All file paths are checked for lexical `..` components and canonicalized to ensure their resolved path resides strictly under the jail's root directory.
- **Symlink Protection**: Symlinks pointing outside the jail or pointing to circular symlink loops (`ELOOP`) are rejected with `SANDBOX_DENIED`.
- **Interactive Approval Modal**: Non-destructive operations (reading a file, listing directories) are permitted automatically. Write or command actions trigger an interactive approval dialog in Svelte.
- **Audit Log**: Every tool execution request, parameter set, user approval decision, and execution status is recorded in a SQLite audit table. The log is user-visible and can be cleared from Settings.

### 5. Model Manager & Streaming Verification
- **Catalog Integrity Digest**: The bundled manifest carries a digest over every entry's id, size, and hash. A mismatch aborts the catalog before any download starts, so a tampered list cannot send you to a spoofed URL.
- **Byte-Range Resume**: Interrupted downloads automatically resume using HTTP `Range: bytes=<offset>-` headers, writing to temporary `.part` files.
- **Constant-RAM Streaming Hash**: Checksums are computed in streaming 64 KB buffers, consuming less than 100 KB of RAM even when validating large 8 GB model files.

---

## Hardware Requirements & Recommended Models

LocalAide runs GGUF models with llama.cpp v0.9.0. The built-in catalog lists 36 models, including vision, embedding, reranking and coding models. Each entry was checked against Hugging Face. Each file's size and SHA-256 match the hub, and each license is shown in the Model Manager.

| Preset | RAM needed | Models |
|---|---|---|
| Tiny | about 1 GB | Qwen3.5 0.8B |
| Small | about 4 GB | Qwen3.5 2B, Gemma 4 E2B (QAT) |
| Balanced | about 9.5 GB | Qwen3.5 9B, Gemma 4 E4B (QAT) |
| Advanced | about 20 GB | Gemma 4 26B-A4B (MoE), Qwen3.8 27B |

Full list, sources and rejected models: [docs/evidence/models-2026-11.md](docs/evidence/models-2026-11.md).

### Minimum Hardware
- **Processor**: Intel/AMD x86_64 CPU (with AVX2 support) or Apple Silicon (M1/M2/M3/M4).
- **RAM**: 8 GB minimum (16 GB recommended for 3B/7B models).
- **Storage**: 2 GB free SSD space for the application; 1–6 GB per downloaded model.

---

## Getting Started

### Option A: Download Precompiled Binaries

Download ready-to-run native desktop packages directly from the [**Releases Page**](https://github.com/omarjson/LocalAide/releases):

- **macOS (Apple Silicon M1/M2/M3/M4)**: `LocalAide_<version>_aarch64.dmg`
- **macOS (Intel Core x86_64)**: `LocalAide_<version>_x64.dmg`
- **Windows (x64)**: `LocalAide_<version>_x64_en-US.msi` or NSIS installer (`.exe`)
- **Linux (x64)**: `LocalAide_<version>_amd64.deb` or `LocalAide_<version>_amd64.AppImage`

---

### Option B: Building From Source

#### 1. Prerequisites
- **Node.js**: v20 or newer (`nvm use 20`)
- **Rust Toolchain**: Stable 1.90+ with `rustfmt` and `clippy` (`rustup update stable`)
- **C/C++ Compiler**:
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `sudo apt install build-essential libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev`
  - Windows: Visual Studio 2022 C++ Build Tools (MSVC)

#### 2. Clone and Setup
```bash
git clone https://github.com/omarjson/LocalAide.git
cd LocalAide

# Install frontend dependencies
npm ci
```

#### 3. Run Quality Verification Suite
```bash
# Execute full quality gate (formatting, clippy, unit tests, frontend build, smoke check)
make all
```

#### 4. Launch Desktop Application in Development Mode
```bash
npm run dev
# or launch with native Tauri shell
npx tauri dev
```

#### 5. Compile Native Release Installer
```bash
make release
# or
npx tauri build
```
Binaries will be placed in `src-tauri/target/release/bundle/`.

---

## Keyboard Navigation Matrix

LocalAide is fully navigable without a mouse:

| Key Binding | Action | Scope |
|---|---|---|
| `⌘K` / `Ctrl+K` | Open Command Palette | Global |
| `⌘N` / `Ctrl+N` | Start New Conversation Thread | Global |
| `/` | Focus Search Input | Global |
| `⌘,` / `Ctrl+,` | Open Settings & Security | Global |
| `⌘1` | Navigate to Chat Screen | Global |
| `⌘2` | Navigate to Search (Document Library) | Global |
| `⌘3` | Navigate to Memory Notes | Global |
| `⌘4` | Navigate to Model Manager | Global |
| `⌘5` | Navigate to Settings | Global |
| `Esc` | Abort Active Generation / Close Modals | Global |
| `?` | Open Keyboard Shortcuts Reference | Global |
| `Enter` | Submit Prompt | Chat Input |
| `Shift+Enter` | Insert Line Break | Chat Input |
| `Tab` / `Shift+Tab` | Traverse Focusable Controls | Global |

---

## Verification & Quality Gates

LocalAide is verified by the CI pipeline on every push. The quality gate runs: Rust `rustfmt` and `clippy --all-targets -- -D warnings`, the full `cargo test --all-targets` suite (unit, contract, fault-injection, inference, library, memory/RAG, and tools/sandbox tests), the frontend `svelte-check` typecheck, and the frontend production build. Release builds compile for macOS (Apple Silicon and Intel), Windows x64, and Linux x64, each bundling the matching `llama-server` sidecar binary.

Design and honesty are tracked against the rubric in [docs/evidence/scorecard.md](docs/evidence/scorecard.md), and the project keeps a standing honesty audit in [docs/evidence/audit-2026-11-13.md](docs/evidence/audit-2026-11-13.md) that lists every known gap between claims and code. Claims in this README are matched to the code and re-checked against that audit.

### Architectural Decision Records (ADRs)
- [ADR-0001: Typed IPC Boundaries](docs/adr/0001-typed-ipc.md)
- [ADR-0002: Storage Engine Migration to SQLite WAL](docs/adr/0002-storage.md)
- [ADR-0003: Isolated llama.cpp Worker Engine (superseded by ADR-0007)](docs/adr/0003-inference.md)
- [ADR-0007: llama-server Sidecar for Chat](docs/adr/0007-sidecar-engine.md)
- [ADR-0004: Capability Workspace Jail](docs/adr/0004-sandbox.md)
- [ADR-0005: Atomic Updates & Verification](docs/adr/0005-updates.md)
- [ADR-0006: Feature Gating & Deferred Scopes](docs/adr/0006-optional-features.md)

---

## Roadmap & Scope Policy

To guarantee stability and quality, LocalAide follows a strict scope discipline:

- **Shipped in v1.0**: Local GGUF chat via llama-server, model catalog & manager, SQLite WAL document library with keyword search (FTS5 BM25), markdown memory notes with RAG citations, and capability-rooted safe file tools.
- **In Development (Opt-In, Next Release)**:
  - *Online Providers*: OpenAI-compatible cloud APIs as an explicit opt-in, disabled by default, with a user-supplied key. The local-first default and its zero-data-exfiltration guarantee do not change.
- **Explicit Non-Goals for v1.0 (Deferred per ADR-0006)**:
  - *Speech (STT / TTS)*: Deferred. The speech path needs a second runtime. See `.kiro/specs/engine-upgrade/tasks.md`.
  - *Vision (Candle SLM / Qwen-VL)*: Deferred until multimodal support is verified end-to-end through the sidecar engine.
  - *Modular In-App Hot-Swapping*: Replaced with standard signed app updates.

---

## Security Disclosure

If you discover a security vulnerability within LocalAide, please review our [Security Policy](SECURITY.md) for reporting instructions. Do not open public issues for sensitive security vulnerabilities.

---

## License

LocalAide is open-source software licensed under the [MIT License](LICENSE).
Copyright © 2026 LocalAide Contributors.
