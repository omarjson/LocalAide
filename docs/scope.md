# LocalAide Scope and Product Contract

## Mission
LocalAide is a privacy-first, local-first desktop assistant for daily computer workflows. Every shipped control works, every backend claim is true, all inference and processing run locally by default, and failures are graceful and recoverable.

---

## 1. Scope Boundary

### Included (the shipped core)

1. **Local chat**:
   - Multi-turn conversation with streaming generation via Tauri channels.
   - Stop generation, message retry, edit-and-resend, message copy, and conversation deletion.
   - Context window tracking with oldest-turn trimming and context warnings.
   - Markdown text with fenced code blocks shown in styled blocks with copy buttons; no raw HTML rendering.
   - Conversations and messages persist as JSON thread files under `~/.aiassistant/conversations/`; settings in `config.json`.
2. **Image input with vision models**:
   - Attach up to 4 images (PNG, JPEG, WEBP, GIF, 20 MB total) with models that ship an `mmproj` projector.
   - Text-only models reject image input with a readable error.
   - Runtime verification against a real vision model is still outstanding; see the status line in [README.md](../README.md).
3. **Model management**:
   - Curated catalog of 36 verified GGUF models: Qwen 3/3.5/3.8, Gemma 4 (QAT), Granite 4.x, LFM, MiniCPM5, SmolLM3, plus coding, embedding, and reranking entries.
   - Atomic streaming downloads to `.part` files. Downloads can be cancelled; a cancelled or interrupted download keeps its `.part` file and resumes via HTTP Range on the next attempt (there is no separate pause control).
   - Mandatory SHA-256 integrity verification against the catalog before installation.
   - Inference runs in a `llama-server` child process, with clean load, unload, and model switching.
4. **Document library & indexing**:
   - User-specified folder indexing into embedded SQLite with WAL mode.
   - Content hashing, deduplication, incremental scanning skipping unchanged files by mtime/size.
   - Bounded file reading and natural-boundary chunking (1500 characters, 250 overlap).
   - Keyword search over SQLite FTS5 (BM25). Vector search is deferred until embeddings are wired.
5. **Memory & RAG**:
   - Personal Markdown memory notes under a fixed memory root at `~/.aiassistant/memory/`.
   - Full atomic CRUD operations with slug management.
   - Memory search delegates to the whole-library keyword search (`MemoryService.search` calls `LibraryService.search`): notes are indexed into the unified SQLite index alongside documents, so results are not scoped to the memory root and there is no recency biasing.
   - Chat retrieval-augmented generation with numbered, clickable citations linking to source notes.
6. **Safe tools & workspace jail**:
   - Workspace directory isolation at `~/.aiassistant/workspace/` via `std::fs::canonicalize` plus lexical path checks, rejecting traversal and symlink escapes.
   - Explicit user approval before any file write, showing the tool name, action, path, and payload preview.
   - Persistent audit log for all tool invocations; user-visible and clearable from Settings.
7. **Opt-in online providers**:
   - User-configured OpenAI-compatible providers (label, base URL, model, API key), stored in the local config file.
   - Cloud mode is off by default; switching it on sends chat only to the chosen provider over HTTPS (plain HTTP allowed only for loopback proxies).
   - A cloud badge in the chat header shows whenever cloud mode is active; local inference stays available and switches back on model load.
   - The API key never leaves the backend except as a bearer token to the provider's base URL.
8. **Accessible, honest frontend**:
   - Svelte 5 runes architecture with design tokens ([DESIGN.md](../DESIGN.md)).
   - Zero dead buttons, no unhandled async rejections, no synthetic mock data.
   - Keyboard-complete navigation and ARIA semantics; WCAG 2.2 AA is the target with one open contrast finding ([docs/evidence/a11y.md](evidence/a11y.md)).

---

## 2. Explicit Non-Goals

The following are out of scope for this release. UI controls, dependencies, and doc claims for them are disabled or removed until implemented end-to-end:

- **Speech (STT / TTS)**: needs a second runtime. Deferred per ADR-0006; findings recorded in `.kiro/specs/engine-upgrade/tasks.md` Phase 5.
- **Vector / semantic search**: no embeddings are written and no query vector is passed; the search path is BM25 keyword only.
- **Modular component hot-swapping**: deferred; updates ship as whole packages (updater signing is future work, ADR-0005).
- **Telemetry / remote analytics**: zero background telemetry, analytics, or user profiling.

---

## 3. Supported Platforms and Tiers

### Tier 1 (built and tested in CI)
- **macOS Apple Silicon (aarch64-apple-darwin)**: macOS 13+. Metal acceleration (upstream llama.cpp default). *Sidecar builds currently cancelled in CI — bundling unverified.*
- **macOS Intel (x86_64-apple-darwin)**: macOS 12+. CPU / Accelerate backend. *Same caveat.*
- **Windows x64 (x86_64-pc-windows-msvc)**: Windows 10/11 64-bit. CPU inference (AVX2).
- **Linux x64 (x86_64-unknown-linux-gnu)**: Ubuntu 22.04+. CPU inference. Requires WebKitGTK 4.1.

---

## 4. Definition of "Supported"
A platform, model, or feature is **supported** if and only if:
1. Automated CI tests build, test, and pass lint/clippy checks for it.
2. An end-to-end smoke test verifies cold start, model loading, and basic interaction.
3. Every error in that path produces a user-readable error code (`AppError`) without crashing the application.
4. Reproduction steps and documented evidence exist in `docs/evidence/`.

By this definition, macOS packages are not yet supported: their CI stage has never completed.

---

## 5. Privacy and Security Stance
- **Zero telemetry**: no tracking pixels, analytics SDKs, crash beacons, or remote logging.
- **Network boundaries**: network access happens only for explicit user actions — model downloads from verified catalog URLs and, when cloud mode is switched on, chat requests to the provider the user configured. No update check is wired yet (ADR-0005).
- **Filesystem isolation**: tool file access is jailed to the workspace directory. Path traversal, symlink escapes, and system roots are hard-rejected.
- **Integrity**: models are verified via SHA-256 checksums before entering active status. Provider base URLs must be HTTPS unless they point at the user's own machine.
