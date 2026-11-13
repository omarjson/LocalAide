# LocalAide Engineering Backlog & Phase Tracking

This backlog tracks the path to a 10/10 v1.0 release. Each task is defined with strict "Done When" acceptance criteria and required verification evidence.

---

## Phase 0: Contract and Scope Freeze
- [x] **0.1 Scope Contract & Non-Goals**
  - **Done When**: `docs/scope.md` is approved, enumerating shipped core, explicit non-goals, platform tiers, and privacy stance.
  - **Evidence**: `docs/scope.md` committed.
- [x] **0.2 Architecture Decision Records (ADRs 0001–0006)**
  - **Done When**: ADRs for Typed IPC, Unified Storage, Inference Engine, Sandbox Jail, Verified Updates, and Feature Gating are documented.
  - **Evidence**: `docs/adr/0001-0006` committed.
- [x] **0.3 Truthful Documentation & Removal of False Claims**
  - **Done When**: `README.md`, `PACKAGING.md`, `docs/MODULAR_UPDATES.md`, and `CHANGELOG.md` are audited, with status tables linking only to verified implementations.
  - **Evidence**: Clean diffs in documentation files.
- [x] **0.4 PR Template & Quality Gates Definition**
  - **Done When**: `.github/pull_request_template.md` and `docs/evidence/scorecard.md` are established.
  - **Evidence**: Templates in repository.

---

## Phase 1: Boot Verification & Dead Surface Removal
- [x] **1.1 Svelte 5 Bootloader Fix**
  - **Done When**: `src/main.ts` uses `mount(App, { target })` instead of legacy Svelte 4 `new App()`.
  - **Evidence**: Frontend boots cleanly without Svelte instantiation error.
- [x] **1.2 CSS & Tailwind Pipeline Import**
  - **Done When**: `main.css` is imported in `main.ts`, and compiled CSS in `dist/assets/` contains full Tailwind utility definitions (>10 KB, not 162 bytes).
  - **Evidence**: `dist/assets/*.css` inspection.
- [x] **1.3 Window Title, Identity & Metadata Fix**
  - **Done When**: `tauri.conf.json` window title is set to "LocalAide", product name is set to "LocalAide", and missing `/vite.svg` is removed from `index.html`.
  - **Evidence**: Window and HTML title verified.
- [x] **1.4 Startup Crash Prevention (Tantivy & Sled)**
  - **Done When**: `Index::open_or_create` is used instead of panicking `create_in_dir`, and missing files return `Err` instead of `v.remove(0)` panic.
  - **Evidence**: Two consecutive app launches succeed without crashing.
- [x] **1.5 Dead Control Removal**
  - **Done When**: "Add model", "Cloud LLM", microphone button, "Verify", and reload hacks are removed or disabled from UI.
  - **Evidence**: Playwright UI smoke check passes with zero dead elements.
- [x] **1.6 Frontend Error Boundary**
  - **Done When**: Unhandled exceptions render a recoverable crash screen with a copyable diagnostics report instead of a white screen.
  - **Evidence**: Triggering an intentional error shows the error boundary.

---

## Phase 2: Quality Harness & Continuous Integration
- [x] **2.1 Standardized Developer Tooling**
  - **Done When**: `rust-toolchain.toml`, `.editorconfig`, `.nvmrc`, and a unified `justfile` / `Makefile` are added with `check`, `test`, `lint`, and `build` recipes.
  - **Evidence**: Tooling configuration files active.
- [x] **2.2 Fix All 29 `svelte-check` Errors**
  - **Done When**: Delete faulty `src/lib/tauri.d.ts`, fix prop typings and SVG attribute casings, and pass `npm run check` with 0 errors.
  - **Evidence**: `npm run check` exits with code 0.
- [x] **2.3 Meaningful Multi-OS CI Workflow**
  - **Done When**: GitHub Actions runs `cargo clippy -- -D warnings`, `cargo test`, `npm run check`, `npm run build`, and WebKitGTK deps on Linux.
  - **Evidence**: Green CI run on GitHub.
- [x] **2.4 Startup Smoke Test Automation**
  - **Done When**: Headless browser smoke test launches the built UI and asserts zero console errors and visible navigation.
  - **Evidence**: Test report in `docs/evidence/smoke-test.log`.

---

## Phase 3: Typed IPC & State Management
- [x] **3.1 Generated TypeScript Bindings**
  - **Done When**: Rust command requests and responses are generated into `src/lib/api/bindings.ts` with no hand-written DTOs or `serde_json::Value` / `any`.
  - **Evidence**: `src/lib/api/bindings.ts` generated.
- [x] **3.2 Structured AppError Taxonomy**
  - **Done When**: `AppError` enum with stable machine-readable codes is used across all Tauri commands.
  - **Evidence**: Unit tests verifying error code mappings.
- [x] **3.3 Single AppState Registration**
  - **Done When**: `AppState` holds managed services (Chat, Models, Library, Memory, Tools, Config, Jobs) without split global state.
  - **Evidence**: `lib.rs` registers a single managed `AppState`.
- [x] **3.4 Tauri Channels for Streaming**
  - **Done When**: `tauri::ipc::Channel<T>` replaces all `stream_id` global event listeners.
  - **Evidence**: Chat and download progress emit exclusively on channels.

---

## Phase 4: Production Model Lifecycle & Inference
- [x] **4.1 Inference Off the Async Runtime (sidecar engine)**
  - **Done When**: Model loading and token streaming run outside the Tauri async runtime; zero blocking of the UI.
  - **Evidence**: Chat generation runs in the `llama-server` sidecar child process (ADR-0007); `commands/chat.rs` drives it from a plain thread.
- [x] **4.2 Real Model Activation & Swap**
  - **Done When**: Activating a model starts the sidecar on the downloaded GGUF weights (SHA-256 verified before activation), and unloading or switching ends the previous server process, reclaiming its memory.
  - **Evidence**: `SidecarEngine::load`/`unload` in `src-tauri/src/services/inference/sidecar.rs` spawn and kill the child process per model.
- [x] **4.3 Interruptible Generation & Cancellation**
  - **Done When**: Stopping chat generation immediately interrupts token streaming at the next chunk.
  - **Evidence**: Stop generation latency < 100ms.
- [x] **4.4 Context Window Management**
  - **Done When**: Conversations exceeding token limits gracefully trim oldest turns with warning.
  - **Evidence**: Automated test exceeding context limits without crash.

---

## Phase 5: Complete Chat Product
- [x] **5.1 Persistent Conversations**
  - **Done When**: Conversations, messages, and settings survive app restarts. Shipped as JSON thread files under `~/.aiassistant/conversations/` plus `config.json`; the document library, notes index and audit log persist in SQLite WAL.
  - **Evidence**: `chat_thread_save` / `chat_thread_load` in `src-tauri/src/commands/chat.rs`; threads reload on restart.
- [ ] **5.2 Chat Action Controls**
  - **Done When**: Stop, regenerate, edit-and-resend, copy message, and conversation deletion are implemented.
  - **Evidence**: End-to-end chat test verifying all actions.
- [ ] **5.3 Markdown & Code Rendering**
  - **Done When**: Markdown renders sanitized HTML, highlighted code blocks with copy buttons, and styled tables.
  - **Evidence**: Visual verification and XSS sanitization tests.

---

## Phase 6: Document Library & Retrieval
- [x] **6.1 SQLite Database Schema & Migrations**
  - **Done When**: Embedded SQLite WAL database manages documents, chunks, and FTS5 tables with automated schema migrations.
  - **Evidence**: SQLite integration tests.
- [ ] **6.2 Natural-Boundary Chunking & Real Embeddings**
  - **Done When**: Files are parsed into natural-boundary chunks of target 1500 characters with 250-character overlap (shipped in `src-tauri/src/services/library/ingest.rs`) and embedded using a real quantized model (or pinned ONNX/GGUF).
  - **Evidence**: Chunking unit tests.
- [ ] **6.3 Debounced Index Watcher**
  - **Done When**: File modifications in indexed roots debounce cleanly without locking the database.
  - **Evidence**: File watcher integration test.

---

## Phase 7: Memory & RAG with Citations
- [ ] **7.1 Memory CRUD with Atomic Writes**
  - **Done When**: Notes can be created, updated, listed, read, and deleted atomically within the memory root.
  - **Evidence**: CRUD test suite.
- [ ] **7.2 Scoped Retrieval & Citations**
  - **Done When**: Chat queries retrieve relevant memory notes, embed delimited context, and output clickable numbered citations.
  - **Evidence**: Citation resolution test.

---

## Phase 8: Trustworthy Installs, Tools & Sandbox
- [ ] **8.1 Signed Model Catalog**
  - **Done When**: `catalog.json` contains SHA-256 hashes, sizes, and licenses. Shipped state: the catalog is checked by a SHA-256 integrity digest over its entries before any download starts — the code documents this as an integrity check, not a publisher signature. Ed25519 catalog signing remains the open piece of this item.
  - **Evidence**: Catalog digest verification in `src-tauri/src/services/models/catalog.rs`.
- [ ] **8.2 Resumable Streaming Downloads**
  - **Done When**: Downloads write to `.part` files, verify SHA-256 on completion, and support Range resumption.
  - **Evidence**: Interrupted download resumption test.
- [x] **8.3 Workspace Jail Sandbox**
  - **Done When**: Tools cannot read, write, or delete files outside the approved directory, including via symlinks.
  - **Evidence**: Path traversal penetration test suite.
- [ ] **8.4 Tool Confirmation UI**
  - **Done When**: File writes display full diff previews and require explicit user approval.
  - **Evidence**: Approval modal tests.

---

## Phase 9: Accessible, Refined UI
- [ ] **9.1 WCAG 2.2 AA Conformance**
  - **Done When**: Axe DevTools reports zero serious or critical issues; focus visible; contrast compliant.
  - **Evidence**: Axe accessibility report.
- [ ] **9.2 Keyboard Navigation & Shortcuts**
  - **Done When**: All features are operable via keyboard; shortcuts ignore active form inputs.
  - **Evidence**: Keyboard navigation checklist.

---

## Phase 10: Verification, Benchmarks & Fault Injection
- [ ] **10.1 Automated Test Pyramid**
  - **Done When**: Rust unit tests, TypeScript tests, and Playwright E2E tests are complete and passing.
  - **Evidence**: Coverage reports > 80%.
- [ ] **10.2 Fault Injection Suite**
  - **Done When**: App handles disk full, corrupted model file, killed worker, and locked DB gracefully.
  - **Evidence**: Fault injection test reports.

---

## Phase 11: Release Engineering & Documentation
- [ ] **11.1 Signed & Notarized Packaging**
  - **Done When**: macOS DMG is signed/notarized; Windows installer is signed.
  - **Evidence**: Release artifacts and signature logs.
- [ ] **11.2 Evidence-Linked Documentation**
  - **Done When**: Every claim in `README.md` links to test evidence or an ADR.
  - **Evidence**: Verified documentation links.

---

## Phase 12: Optional Features Gate
- [ ] **12.1 Audit & Decision on Optional Stubs**
  - **Done When**: Voice, Vision, and Modular updates are either implemented end-to-end with tests or completely purged.
  - **Evidence**: Zero stubs or dead code remaining in codebase.
