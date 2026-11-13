# LocalAide Engineering Backlog & Phase Tracking

This backlog tracked the path to the current release. Each task is defined with "Done When" acceptance criteria and required verification evidence. Checkboxes reflect the shipped state; where the shipped scope differs from the original wish, the note says so.

---

## Phase 0: Contract and Scope Freeze
- [x] **0.1 Scope Contract & Non-Goals**
  - **Done When**: `docs/scope.md` is approved, enumerating shipped core, explicit non-goals, platform tiers, and privacy stance.
  - **Evidence**: `docs/scope.md` committed.
- [x] **0.2 Architecture Decision Records (ADRs 0001–0007)**
  - **Done When**: ADRs for Typed IPC, Unified Storage, Inference Engine, Sandbox Jail, Verified Updates, Feature Gating, and the sidecar engine are documented.
  - **Evidence**: `docs/adr/0001`–`0007` committed.
- [x] **0.3 Truthful Documentation & Removal of False Claims**
  - **Done When**: `README.md`, `PACKAGING.md`, `docs/MODULAR_UPDATES.md`, and `CHANGELOG.md` are audited, with status claims linked only to verified implementations.
  - **Evidence**: Clean diffs in documentation files; standing audit in `docs/evidence/audit-2026-11-13.md`.
- [x] **0.4 PR Template & Quality Gates Definition**
  - **Done When**: `.github/pull_request_template.md` and `docs/evidence/scorecard.md` are established.
  - **Evidence**: Templates in repository.

---

## Phase 1: Boot Verification & Dead Surface Removal
- [x] **1.1 Svelte 5 Bootloader Fix** — `src/main.ts` uses `mount(App, { target })`.
- [x] **1.2 CSS & Tailwind Pipeline Import** — `main.css` imported in `main.ts`; compiled bundle carries full Tailwind utilities (asserted by the smoke test).
- [x] **1.3 Window Title, Identity & Metadata Fix** — window title and product name are "LocalAide"; no dead `/vite.svg` reference.
- [x] **1.4 Startup Crash Prevention** — the legacy Tantivy/Sled startup panics were removed with the storage engine itself (ADR-0002).
- [x] **1.5 Dead Control Removal** — "Add model", microphone, and reload-hack controls removed; every visible control has a backend.
- [x] **1.6 Frontend Error Boundary** — unhandled exceptions render a recoverable screen instead of a white screen.

---

## Phase 2: Quality Harness & Continuous Integration
- [x] **2.1 Standardized Developer Tooling** — `rust-toolchain.toml`, `.editorconfig`, `.nvmrc`, and the `Makefile` with `check`, `test`, `lint`, `build`, `smoke`, `release` recipes.
- [x] **2.2 `svelte-check` Clean** — `npm run check` exits 0.
- [x] **2.3 Multi-OS CI Workflow** — GitHub Actions quality gate runs `cargo clippy -- -D warnings`, `cargo test`, `npm run check`, `npm run build`, and the WebKitGTK deps on Linux. See the status line in `README.md` for the one stage that has not completed (macOS bundling).
- [x] **2.4 Startup Smoke Test Automation** — `scripts/smoke-test.mjs` asserts the built `dist/` output and writes `docs/evidence/smoke-test.log` on each run (the log is gitignored, not committed).

---

## Phase 3: Typed IPC & State Management
- [x] **3.1 TypeScript Bindings** — Rust command DTOs are mirrored in `src/lib/api/bindings.ts`; screens call the backend only through `src/lib/api/client.ts`.
- [x] **3.2 Structured `AppError` Taxonomy** — stable machine-readable codes across all commands; serialization tested in `contract_test.rs`.
- [x] **3.3 Single AppState Registration** — `AppState` holds jobs, models, inference, library, memory, tools, and config (see `ARCHITECTURE.md`).
- [x] **3.4 Tauri Channels for Streaming** — chat tokens and download progress emit on `tauri::ipc::Channel` instead of global events.

---

## Phase 4: Production Model Lifecycle & Inference
- [x] **4.1 Inference Off the Async Runtime** — chat generation runs in the `llama-server` sidecar child process (ADR-0007); commands drive it off the async runtime.
- [x] **4.2 Real Model Activation & Swap** — activating a model starts the sidecar on the downloaded GGUF (SHA-256 verified first); switching or unloading kills the previous server process.
- [x] **4.3 Interruptible Generation & Cancellation** — Stop applies at the next streamed token via the atomic cancel flag.
- [x] **4.4 Context Window Management** — over-budget conversations trim oldest turns deterministically; covered by `inference_test.rs`.

---

## Phase 5: Complete Chat Product
- [x] **5.1 Persistent Conversations** — threads are JSON files under `~/.aiassistant/conversations/` plus `config.json`; the document library, notes index, and audit log persist in SQLite WAL.
- [x] **5.2 Chat Action Controls** — stop, retry, edit-and-resend, copy message, and thread deletion are implemented and wired in `ChatScreen.svelte` / `MessageBubble.svelte`.
- [x] **5.3 Markdown & Code Rendering** — *shipped scope:* fenced code blocks render in styled blocks with copy buttons and messages copy to the clipboard. Raw HTML rendering, syntax highlighting, and styled tables were consciously not built; the original wish list in this item overstated what shipped.

---

## Phase 6: Document Library & Retrieval
- [x] **6.1 SQLite Database Schema & Migrations** — documents, chunks, notes, tool audit, and the FTS5 table with forward-only migrations in `library/db.rs`.
- [x] **6.2 Natural-Boundary Chunking** — paragraph/sentence chunking at 1500 characters with 250-character overlap in `library/ingest.rs`, covered by `library_test.rs`. *Real embeddings were part of this item originally; they remain unbuilt and are a declared non-goal for this release (see `scope.md`).*
- [ ] **6.3 Debounced Index Watcher**
  - **Done When**: File modifications in indexed roots debounce cleanly without locking the database.
  - **Status**: `library/watch.rs` ships (recursive watch, 500 ms debounce, ignore list) but has no automated test yet.

---

## Phase 7: Memory & RAG with Citations
- [x] **7.1 Memory CRUD with Atomic Writes** — create, read, update, delete with disk sync; `memory_rag_test.rs`.
- [x] **7.2 Scoped Retrieval & Citations** — relevance gate (0.010), token budget, numbered clickable citations; `memory_rag_test.rs`.

---

## Phase 8: Trustworthy Installs, Tools & Sandbox
- [ ] **8.1 Signed Model Catalog**
  - **Done When**: `catalog.json` contains SHA-256 hashes, sizes, and licenses. *Shipped state: the catalog is checked by a SHA-256 integrity digest over its entries before any download starts — an integrity check, not a publisher signature. Ed25519 signing remains the open piece of this item.*
  - **Evidence**: Digest verification in `src-tauri/src/services/models/catalog.rs`.
- [x] **8.2 Resumable Streaming Downloads** — `.part` files, SHA-256 on completion, HTTP Range resumption; covered by the fault-injection suite.
- [x] **8.3 Workspace Jail Sandbox** — traversal, absolute escapes, outside symlinks, and symlink loops blocked; `tools_sandbox_test.rs`.
- [x] **8.4 Tool Confirmation UI** — file writes display a payload preview and require explicit approval ("Allow Once" / "Allow for Session" / "Deny").

---

## Phase 9: Accessible, Refined UI
- [ ] **9.1 WCAG 2.2 AA Conformance**
  - **Done When**: Zero serious or critical issues; focus visible; contrast compliant.
  - **Status**: Keyboard and semantics pass ([docs/evidence/a11y.md](evidence/a11y.md)); the open finding is border contrast below the 3:1 non-text threshold, and the design pass has not been re-measured on screen.
- [x] **9.2 Keyboard Navigation & Shortcuts** — full keyboard matrix in `App.svelte`; shortcuts ignore active form inputs; documented in [docs/evidence/a11y.md](evidence/a11y.md).

---

## Phase 10: Verification, Benchmarks & Fault Injection
- [ ] **10.1 Automated Test Pyramid**
  - **Done When**: Rust unit tests, TypeScript tests, and E2E tests complete and passing.
  - **Status**: Rust suites and the frontend smoke test run in CI; no TypeScript unit tests and no browser E2E yet.
- [x] **10.2 Fault Injection Suite** — corrupt GGUF, killed download, corrupt SQLite DB, oversized write, and jail attacks handled cleanly; `fault_injection_test.rs` plus the sandbox tests.

---

## Phase 11: Release Engineering & Documentation
- [ ] **11.1 Signed & Notarized Packaging**
  - **Done When**: macOS DMG is signed/notarized; Windows installer is signed; sidecar binaries are signed inside the bundle.
  - **Status**: Not started. macOS bundling itself is unverified in CI (see the status line in `README.md`).
- [ ] **11.2 Evidence-Linked Documentation**
  - **Done When**: Every claim in `README.md` links to test evidence or an ADR.
  - **Status**: Most claims link to evidence; a final link-by-link sweep is outstanding.

---

## Phase 12: Optional Features Gate
- [x] **12.1 Audit & Decision on Optional Stubs**
  - **Done When**: Voice, Vision, and Modular updates are either implemented end-to-end with tests or completely purged.
  - **Outcome**: Voice stubs and the Candle vision path were purged (ADR-0006); modular updates are deferred by specification (`docs/MODULAR_UPDATES.md`). Image input later shipped through the sidecar's multimodal path (ADR-0007), which is a different mechanism than the deferred Candle vision plan.
