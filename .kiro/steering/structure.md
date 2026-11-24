---
inclusion: always
---

# LocalAide: Repo Structure

## Top level

- `src/`: Svelte 5 frontend.
- `src-tauri/`: Rust core and Tauri config. `about.toml` and `about.hbs` drive the `cargo about` license file for releases.
- `vendor/`: deleted. The old vendored `llama_cpp_rs` fork is gone; `llama-server` is downloaded at runtime for the user's processor (`services/engine.rs`, pins in `services/externals.rs`, ADR-0012).
- `docs/`: scope, backlog, ADRs, evidence, and `release.md` (how releases are built and signed).
- `scripts/`: smoke test (`smoke-test.mjs`), catalog tools, and `ci/` (`cargo-annotate.mjs` turns cargo failures into annotations; `check-versions.mjs` is the version gate; `updater-config.mjs` and `make-latest-json.mjs` sign update packages and write the update feed), and `gen-installer-art.mjs` (installer images from the logo).
- `tests/unit/`: frontend unit tests, run by Node's own test runner (`npm run test:unit`).
- `public/`: files served as they are, including `theme-init.js`, which sets the theme before the first paint.
- `.github/`: workflows (`ci.yml`, `llama-server.yml`, `audit.yml`), `dependabot.yml`, issue templates.
- `.kiro/`: Kiro project files: `skills/impeccable/`, `skills/ui-ux-pro-max/` (MIT, vendored from nextlevelbuilder/ui-ux-pro-max-skill; run `python3 .kiro/skills/ui-ux-pro-max/scripts/search.py "<query>" --domain ux`), this `steering/` folder (`catalog.md` loads only for catalog files), `specs/` (`engine-upgrade`, `kiroween-2026`, `online-providers`), `hooks/` (svelte-check, clippy, catalog checks) and `settings/mcp.json` (Hugging Face MCP server).
- `PRIVACY.md`, `TERMS.md`: legal pages, bundled into the app and the installer.
- `src-tauri/installer/`: installer art (NSIS header and sidebar, DMG background, macOS icon source).
- `README.md`, `ARCHITECTURE.md`, `docs/scope.md`, `THIRD_PARTY_NOTICES.md`: product, design and license docs. They aim to match the code; if you find a mismatch, fix the doc and note it (see `tech.md` — verify before you claim).

## Frontend (`src/`)

- `main.ts`, `App.svelte`: entry point, global keyboard handling, screen routing, the shared message (toast).
- `main.css`: Tailwind v4 entry and the theme tokens.
- `screens/`: one file per screen. `ChatScreen`, `SearchScreen`, `MemoryScreen`, `SkillsScreen`, `ModelManagerScreen`, `SettingsScreen`, `SetupWizard`.
- `components/`: shared pieces. `Button`, `ChatInput`, `MessageBubble`, `ToolApprovalModal`, `ConfirmDialog`, `ErrorBanner`, `FocusTrap`, `Tabs`, `Settings*`, `Icon` (the single hand-rolled inline-SVG icon set), `Modal`, `MarkdownDoc` (renders shipped markdown without `{@html}`), `ModelPicker`, `EffortPicker`, `ThinkingBlock`, `ToolCallCard`, `AnswerText`, `McpSettings`, and others. `lib/chatParts.ts` builds an answer's thinking, tool and text parts from the stream (unit-tested).
- `lib/api/bindings.ts`: typed DTOs that mirror the Rust domain types.
- `lib/api/client.ts`: the typed IPC client and `errorMessage`. Screens call the backend through it.
- `lib/actions/popover.ts`: the shared menu behaviour (Escape, outside click, arrow keys, focus return).
- `lib/dev.svelte.ts` (developer mode, flags and modules), `lib/requestLog.svelte.ts`, `lib/chatModes.ts`, `lib/api/errors.ts` (`errorMessage`, Node-testable).
- `lib/legal.svelte.ts` (the bundled Privacy Policy, Terms and notices, and the shared open request), `lib/nav.svelte.ts` (palette requests into Settings), `lib/markdown.ts`.
- `lib/toast.svelte.ts`, `lib/links.ts` (open links in the browser), `lib/theme.ts`, `lib/format.ts`, `lib/motion.ts`.

Put new Svelte screens in `src/screens/`, reusable UI in `src/components/`, and every backend call through `src/lib/api/client.ts`.

## Backend (`src-tauri/src/`)

- `app/`: `state.rs` (`AppState`, the single state owner), `paths.rs` (all `~/.aiassistant/` paths), `error.rs` (`AppError`), `jobs.rs` (job registry with cancel flags; cancel by kind), `events.rs`, `blocking.rs` (`run_blocking`, `with_app_state`), `lifecycle.rs` (single instance, kill-on-close job object, console hiding, fatal startup errors).
- `commands/`: Tauri command handlers, one file per area: `backup`, `chat`, `cloud`, `desktop` (file pickers, open links), `diagnostics`, `hf`, `library`, `memory`, `models`, `settings`, `skills`, `system`, `tools`, `developer` (flags, modules, mode table), `integrations` (OpenCode link), `updates` (update check and the in-app updater), `web`, `picker` (model picker, chat target, favourites), `attach` (file attachments, drop allowlist), `mcp` (MCP server settings). Keep them thin. Each one takes a DTO and calls a service. Slow ones are `async` and use `app/blocking.rs`.
- `domain/`: DTO and domain types shared with the frontend: `chat`, `cloud`, `document`, `memory`, `model`, `settings`, `system`, `tool`.
- `services/`:
  - `inference/`: `mod.rs` (the `InferenceEngine` trait), `sidecar.rs` (local llama-server child process on 127.0.0.1), `remote.rs` (opt-in OpenAI-compatible cloud engine), `router.rs` (routes local vs cloud; cloud is off unless the user enables it), `provider_models.rs` (a provider's `GET /models` list), `modes.rs` (chat modes), `context.rs` (token budget). The model's own chat template (`--jinja`) formats prompts, so there is no prompt builder. Voice is deferred (ADR-0006); only feature-gated Cargo deps remain, no voice module.
  - `library/`: `db.rs` (SQLite schema, versioned migrations, queries), `ingest.rs` (decoding, PDF text, chunking), `search.rs` (BM25, vector top-k, RRF fusion), `embedding.rs` and `rerank.rs` (llama-server sidecars for meaning search and reranking), `watch.rs` (folder watcher), `mod.rs` (`LibraryService`).
  - `memory/`: `notes.rs` (markdown notes on disk), `rag.rs` (recall and citations), `mod.rs`.
  - `tools/`: `sandbox.rs` (`WorkspaceJail`), `approval.rs`, `mod.rs` (`ToolService`, audit).
  - `models/`: `catalog.rs` (built-in catalog and manifest check), `custom.rs` (community models), `hf.rs`, `refresh.rs`, `download.rs` (resumable downloads), `verify.rs` (SHA-256), `mod.rs`.
  - `agent/`: the chat tool loop (`mod.rs`), built-in tools (`tools/specs.rs`: the catalogue and schemas; `tools/mod.rs`: how each runs), approvals (`approval.rs`).
  - `mcp/`: MCP client (`mod.rs` sessions, `transport.rs` stdio and Streamable HTTP, `import.rs` other apps' config formats, `presets.rs`).
  - `inference/openai_stream.rs` (shared streaming: text, thinking, tool calls, refused-field retries) and `inference/capabilities.rs` (thinking, vision, tools, context, effort levels).
  - `opencode.rs` (link to a user-run `opencode serve`: health check and task hand-off).
  - `backup/` (encrypted backups, ADR-0009), `skills/` (saved prompts; `discover.rs` browses GitHub skill collections), `websearch/` (web search and page reading, ADR-0011).
- Top-level modules `model_manager.rs` (the active-model record in `models/state.json`, the setup presets, and model removal) and `config/` (`mod.rs`: config load and atomic save; `permissions.rs`: permission rules and capability auto-approve).

## Tests

- `src-tauri/tests/`: `contract_test.rs`, `fault_injection_test.rs`, `inference_test.rs`, `library_test.rs`, `memory_rag_test.rs`, `tools_sandbox_test.rs`. Unit tests sit in `#[cfg(test)]` modules next to the code.
- Run them with `cd src-tauri && cargo test --all-targets`. CI runs them on Ubuntu and Windows.

## Placement rules

- Add a new backend feature as a service in `services/`, a command in `commands/`, and DTOs in `domain/`.
- Keep UI controls wired end to end. Do not add a button without its command and handler.
- New tool actions go through `WorkspaceJail` and the approval flow, and get an audit row.
- Keep decisions in `docs/adr/`. Keep evidence in `docs/evidence/`.
