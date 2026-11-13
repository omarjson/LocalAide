# LocalAide v1.0 Scope and Product Contract

## Mission
LocalAide is a privacy-first, local-first desktop assistant designed for daily computer workflows. Every shipped control works, every backend claim is true, all inference and processing execute locally on-device, and failures are graceful and recoverable.

---

## 1. Scope Boundary

### Included in v1.0 (The Shipped Core)
1. **Local Chat**:
   - Multi-turn conversation with streaming generation via Tauri channels.
   - Stop generation, message regeneration, edit-and-resend, message copy, and conversation deletion.
   - Context window tracking with oldest-message trimming and context warnings.
   - Markdown rendering with code syntax highlighting, copy button, and sanitized HTML.
   - Persistence of conversations and messages in an embedded SQLite database.
2. **Model Management**:
   - Curated catalog of 36 verified GGUF models: Qwen 3/3.5/3.8, Gemma 4 (QAT), Granite 4.x, LFM, MiniCPM5, SmolLM3, plus coding, embedding, and reranking entries.
   - Atomic streaming downloads with `.part` files, pause/resume, cancel without losing progress, and preflight disk checks.
   - Mandatory SHA-256 integrity verification against the catalog before installation.
   - Inference runs in a `llama-server` child process, with clean load, unload, and model switching.
3. **Document Library & Indexing**:
   - User-specified folder indexing into embedded SQLite with WAL mode.
   - Content hashing, deduplication, incremental scanning skipping unchanged files by mtime/size.
   - Bounded file reading (preventing memory exhaustion on large files) and token-based chunking.
   - Keyword search over SQLite FTS5 (BM25). Vector search is deferred until embeddings are wired.
4. **Memory & RAG**:
   - Personal markdown memory notes under a fixed memory root at `~/.aiassistant/memory/`.
   - Full atomic CRUD operations (create, read, update, delete) with slug management.
   - Memory search scoped strictly to the memory root with recency biasing.
   - Chat retrieval augmented generation (RAG) with numbered, clickable citations linking to source notes.
5. **Safe Tools & Workspace Jail**:
   - Workspace directory isolation (`cap-std` / openat semantics) rejecting path traversal and symlink escapes.
   - Mandatory explicit user approval prompt displaying tool action, arguments, and file diffs prior to write execution.
   - Persistent audit log for all tool invocations; user-visible and clearable from Settings.
6. **Opt-In Online Providers**:
   - User-configured OpenAI-compatible providers (label, base URL, model, API key), stored in the local config file.
   - Cloud mode is off by default; switching it on sends chat only to the chosen provider over HTTPS (plain HTTP allowed only for loopback proxies).
   - A cloud badge in the chat header shows whenever cloud mode is active; local inference stays available and switches back on model load.
   - The API key never leaves the backend except as a bearer token to the provider's base URL.
7. **Accessible, Honest Frontend**:
   - Svelte 5 runes architecture with design system components.
   - Zero dead buttons, no unhandled async rejections, no synthetic mock data.
   - Keyboard complete navigation, ARIA semantics, and WCAG 2.2 AA conformance.

---

## 2. Explicit Non-Goals (Out of Scope for v1.0)
The following capabilities are explicitly deferred to Phase 12 or post-v1.0. All UI controls, commands, dependencies, and doc claims for these features are disabled or removed until fully implemented end-to-end:
- **Speech (STT / TTS)**: Needs a second runtime. Deferred until verified on the target platforms.
- **Vision / Multimodal SLM**: Image comprehension models are excluded from the v1.0 core until multimodal support is verified end-to-end through the sidecar engine.
- **Modular Component Hot-swapping**: In-app binary component replacement is deferred; all updates happen via verified whole-app updates.
- **Telemetry / Remote Analytics**: There is zero background telemetry, analytics, or user profiling. Ever.

---

## 3. Supported Platforms and Tiers

### Tier 1 (Fully Supported & Tested in CI)
- **macOS Apple Silicon (aarch64-apple-darwin)**: macOS 13+ (Ventura, Sonoma, Sequoia). Native Metal GPU acceleration.
- **macOS Intel (x86_64-apple-darwin)**: macOS 12+ (Monterey+). CPU / Accelerate backend.
- **Windows x64 (x86_64-pc-windows-msvc)**: Windows 10/11 64-bit. CPU fallback, Vulkan/DirectX where available.
- **Linux x64 (x86_64-unknown-linux-gnu)**: Ubuntu 22.04+, Fedora 38+. Requires WebKitGTK 4.1.

---

## 4. Definition of "Supported"
A platform, model, or feature is **supported** if and only if:
1. Automated CI tests build, test, and pass lint/clippy checks for it.
2. An end-to-end smoke test verifies cold start, model loading, and basic interaction.
3. Every error in that path produces a user-readable error code (`AppError`) without crashing the application.
4. Reproduction steps and documented evidence exist in `docs/evidence/`.

---

## 5. Privacy and Security Stance
- **Zero Telemetry**: No tracking pixels, analytics SDKs, crash beacons, or remote logging.
- **Network Boundaries**: Network access occurs exclusively for explicit user requests: model downloads from verified catalog URLs, app updater checks, and — when cloud mode is explicitly switched on — chat requests to the provider the user configured.
- **Filesystem Isolation**: File access by tools is jailed to approved user directories. Path traversal (`../`), symlink escapes, and system roots are hard-rejected.
- **Integrity**: Models are verified via cryptographic SHA-256 checksums before entering active status. Provider base URLs must be HTTPS unless they point at the user's own machine.
