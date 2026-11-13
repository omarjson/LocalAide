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
   - Curated catalog of GGUF chat models (Qwen 2.5/3.5 quantized series).
   - Atomic streaming downloads with `.part` files, pause/resume, cancel without losing progress, and preflight disk checks.
   - Mandatory SHA-256 integrity verification against the signed catalog before installation.
   - Inference runs in a `llama-server` child process, with clean load, unload, and model switching.
3. **Document Library & Indexing**:
   - User-specified folder indexing into embedded SQLite with WAL mode.
   - Content hashing, deduplication, incremental scanning skipping unchanged files by mtime/size.
   - Bounded file reading (preventing memory exhaustion on large files) and token-based chunking.
   - Keyword search over SQLite FTS5 (BM25). Vector search is deferred until embeddings are wired.
4. **Memory & RAG**:
   - Personal markdown memory notes under user-approved memory root.
   - Full atomic CRUD operations (create, read, update, delete) with slug management.
   - Memory search scoped strictly to the memory root with recency biasing.
   - Chat retrieval augmented generation (RAG) with numbered, clickable citations linking to source notes.
5. **Safe Tools & Workspace Jail**:
   - Workspace directory isolation (`cap-std` / openat semantics) rejecting path traversal and symlink escapes.
   - Mandatory explicit user approval prompt displaying tool action, arguments, and file diffs prior to write execution.
   - Persistent append-only audit log for all tool invocations.
6. **Accessible, Honest Frontend**:
   - Svelte 5 runes architecture with design system components.
   - Zero dead buttons, no unhandled async rejections, no synthetic mock data.
   - Keyboard complete navigation, ARIA semantics, and WCAG 2.2 AA conformance.

---

## 2. Explicit Non-Goals (Out of Scope for v1.0)
The following capabilities are explicitly deferred to Phase 12 or post-v1.0. All UI controls, commands, dependencies, and doc claims for these features are disabled or removed until fully implemented end-to-end:
- **Speech (STT / TTS)**: Needs a second runtime. Deferred until verified on the target platforms.
- **Vision / Multimodal SLM**: Image comprehension models (e.g. Qwen2.5-VL / mtmd) are excluded from the v1.0 core.
- **Cloud LLM APIs**: No OpenAI / Anthropic proxying; LocalAide v1.0 is strictly offline and local.
- **Modular Component Hot-swapping**: In-app binary component replacement is deferred; all updates happen via verified whole-app updates.
- **Telemetry / Remote Analytics**: There is zero background telemetry, analytics, or user profiling. Ever.

---

## 3. Supported Platforms and Tiers

### Tier 1 (Fully Supported & Tested in CI)
- **macOS Apple Silicon (aarch64-apple-darwin)**: macOS 13+ (Ventura, Sonoma, Sequoia). Native Metal GPU acceleration.
- **macOS Intel (x86_64-apple-darwin)**: macOS 12+ (Monterey+). CPU / Accelerate backend.
- **Windows x64 (x86_64-pc-windows-msvc)**: Windows 10/11 64-bit. CPU fallback, Vulkan/DirectX where available.

### Tier 2 (Supported for Manual Builds)
- **Linux (x86_64-unknown-linux-gnu)**: Ubuntu 22.04+, Fedora 38+. Requires WebKitGTK 4.1.

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
- **Network Boundaries**: Network access occurs exclusively for explicit user requests (model downloads from verified catalog URLs, and app updater checks).
- **Filesystem Isolation**: File access by tools is jailed to approved user directories. Path traversal (`../`), symlink escapes, and system roots are hard-rejected.
- **Integrity**: Models are verified via cryptographic SHA-256 checksums before entering active status.
