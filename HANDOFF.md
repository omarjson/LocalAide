# Handoff — the GOAT rebuild

Written 2026-11-28. Two things are true at once today: the Electron migration **shipped and is
released**, and Omar **reset the direction of the app**. Part 1 records where the repo stands.
Part 2 is the future — read it first if you only read one thing.

## TL;DR

- **Migration done and released.** All 204 renderer commands handled by the Electron backend
  (`electron/`), `src-tauri/` deleted, the full CI pipeline green including all five platform
  builds, and a draft release "LocalAide 0.2.5" waiting on GitHub for the publish click.
- **The pivot:** Omar does not want "LocalAide in Electron with GOAT styling." He wants
  **GOAT as the product** — GOAT's real design *and* structure, verbatim — with LocalAide's
  differentiating features rewired into it. The next phase rebuilds the **renderer** in GOAT's
  image. The Electron backend is the engine and stays.
- **The governing rule: rewire, don't graft.** Wherever GOAT already has a surface (its own
  personal-agent UX, memories, sessions, fleet…), GOAT's version wins: keep GOAT's UI exactly
  and point it at this backend. LocalAide's parallel implementations get retired, not displayed
  next to GOAT's. Build new only where GOAT lacks the feature.

## Part 1 — Where the repo stands (all verified on this machine and in CI)

- **Backend:** `electron/` — main.mjs (window, CSP, `localaide-file://`), preload.cjs (the
  `__LOCALAIDE__` + `__TAURI_INTERNALS__` bridge), ipc.mjs (204-command table, one
  `localaide:invoke` channel), core/ (config, db, errors, log, paths, channels), services/
  (chat, modes, threads, engine, embedding, rerank, download, models, indexer, library, memory,
  websearch, cloud, mcp, share, voice, backup, skills, assist, attach, probe, updates,
  automations, developer, doctor, selftest, opencode, usage, stream, toolSpecs, tools).
- **Renderer:** `src/` — Svelte 5, unchanged from the Tauri era. `@tauri-apps/api/*` specifiers
  resolve to the shim (`src/lib/platform/tauri.ts`) through the Vite alias; all commands funnel
  through `src/lib/api/client.ts` as `invoke<T>("cmd")` calls. **This is the layer the GOAT
  rebuild replaces.**
- **Data layer facts a rebuild must not break:** config field names are Rust-era exact
  (`use_compact_prompt`, `plan_auto_accept_secs`, `web_search.provider`, …) and live in users'
  `config.json`; model files sit flat in `models/` as `<dest ?? id>.gguf`; tool binaries unpack
  to `~/.aiassistant/tools/<tool>-<version>/`; channel frames arrive as `{index, message}`.
- **Release state:** tag `v0.2.5` (at 3159499) built green end to end — quality gate, catalog
  pins, version gate, macOS arm64/x64 + Windows x64/arm64 + Linux builds, draft publish with
  installers + SHA256SUMS.txt + SPDX SBOM. Publishing the draft is a human click. Releases are
  cut by: fix CHANGELOG → `node scripts/bump-version.mjs x.y.z` → commit → push main → push
  annotated `v*` tag.
- **Branches:** `main` is the release line. `experiment` (d6be504) carries the refactoring
  scaffold: `tests/unit/ipc-surface.test.ts` (extracts every literal `invoke()` call under
  `src/` and every key of the ipc.mjs `commands` table, asserts exact bidirectional equality —
  the guardrail while any rewrite touches the command surface) and `docs/REFACTORING.md`
  (backend refactor charter + discipline). Unpushed.
- **Verification loop:** `corepack pnpm --config.verify-deps-before-run=false run
  check|build|smoke|test:unit`; the real window over CDP (`electron.exe .
  --remote-debugging-port=9222` → `Runtime.evaluate` of `window.__LOCALAIDE__.invoke(...)`);
  loopback-mock suites in `C:\Users\Omar\AppData\Local\Temp\la-*-test.mjs`.

## Part 2 — The pivot: GOAT is the product, LocalAide is the feature set

What Omar said, in intent: *"this is just the same LocalAide but with Electron. I was wanting
LocalAide to be GOAT — GOAT looks and all stuff — but with memory and embeds and local-first and
the personal agentic look. Not LocalAide in Electron."* And the rule: *"if there is stuff like
the personal agent already in GOAT, just rewire it to have backend coherence."*

**What was wrong with what shipped:** the renderer was never rebuilt — it is still LocalAide's
own screens and flows (built in the Tauri era) with GOAT's color tokens painted on. GOAT only
existed in it as a palette. The rebuild means GOAT's layout, screens, chrome, interaction model,
agent UX — the things that make GOAT feel like GOAT side by side — become the app.

**The three lists (fill them by reading GOAT's source, not by guessing):**

- **REWIRE** — GOAT already has it → GOAT's UI stays as-is, wired to this backend. Adapt the
  backend's contracts to serve GOAT's UI (or extend the backend where GOAT's shape needs it).
  Retire LocalAide's parallel screen.
- **ADD** — GOAT lacks it → build it GOAT-native. Known ADDs from LocalAide's feature set:
  embeddings + document library (local `--embedding` and `--rerank` sidecars, semantic/hybrid
  search), the local-runtime breadth (llama.cpp CPU/Vulkan/ROCm pins, MLX, Ollama, LM Studio,
  vLLM, LocalAI, LiteLLM, and the model catalog/downloader), voice (sherpa-onnx + online +
  live), share (tunnel + pairing), encrypted backups.
- **RETIRED** — LocalAide duplicates of things GOAT has → gone.

**Identity:** GOAT's identity wins. The ghost mascot and the LocalAide persona return only if
GOAT's product has a natural slot for a persona.

**Non-negotiable (the reason LocalAide exists):** memory notes; embedding/semantic search over
your own documents; local-first — your own hardware and models, fully usable offline, cloud
optional; personal agentic controls (plan/ask/smart/auto approvals, goals, automations); voice;
share; backups. These must land as GOAT-native surfaces, not bolt-on dialogs.

## Part 3 — What GOAT is (first-pass inventory; verify everything in source)

From github.com/goat-si/goat (MIT; a fork of T3 Code):

- **Product:** a desktop/web control app for running coding agents (Claude Code, Codex, Cursor,
  OpenCode, …) on machines you own — laptop, home desktop, SSH server, cloud machine — steered
  from desktop app, browser, or phone.
- **Surfaces per README:** **GOAT Bot** (a standing project-independent conversation that spawns
  coding threads and reports back; reachable via Slack/iMessage), **cross-machine agents** (SSH
  hosts, worktrees, moving threads with uncommitted changes), **Fleet** (one view of every
  connected machine and its agents), **Memory** (finished conversations become memories scoped
  global/project/thread; importable from Codex/Claude/OMO; shared across devices via hubs),
  **orchestration MCP** (`goat_*` tools), **computer use** (goat-cu driver, per-app approvals),
  **cloud agents** (Claude Cloud, Cursor Cloud, Devin Cloud).
- **Model:** each machine runs a GOAT server whose state (projects, threads, sign-ins, memory)
  forms an *environment*; clients connect to many environments.
- **Stack:** TypeScript monorepo (pnpm workspaces, Vite), a native/Rust layer (`native/`,
  `.cargo`), a web app served by a server component; ships macOS/Windows/Linux. Note: we reuse
  GOAT's **design, IA and interaction model**, not its Rust layer — this repo's backend is the
  Node/Electron engine.

## Part 4 — Next session's plan (Omar reviews before code)

1. **Deep-dive GOAT's source** (`apps/`, `packages/`, `docs/`, `devlog/`): enumerate its real
   screens, component architecture, design system, and state model. Side-by-side identical is
   the bar, not "a cousin."
2. **Produce the three lists** and a surface-by-surface mapping: GOAT screen → backend commands
   that power it (or the gap to close).
3. **Mockup of the rebuilt shell → Omar's approval** before any component is written. Then live
   preview iteration, his harsh ranked critique each round.
4. **Rebuild the renderer in slices**, gate green every commit. The ipc-surface contract test on
   `experiment` guards the command surface: if GOAT's renderer calls commands differently,
   update both sides in the same commit — consciously.
5. **Retire LocalAide UI per the RETIRED list** as each GOAT surface lands.

## Working rules that saved hours

- **pnpm only** (`packageManager: pnpm@11.25.0`, corepack). npm is broken on this machine's
  Node build. `allowBuilds:` in pnpm-workspace.yaml gates dependency build scripts (pnpm 11 —
  not `onlyBuiltDependencies`). Local runs need `corepack pnpm
  --config.verify-deps-before-run=false run <script>`.
- **`Buffer.prototype.startsWith` does not exist** on this Node build — use
  `bytes.indexOf(MAGIC) === 0`.
- **Inline `node -e` with backticks/`${}` corrupts** — write temp scripts to
  `C:\Users\Omar\AppData\Local\Temp\` and run `node <script>`.
- **electron-builder takes `--x64`/`--arm64`, NOT `--arch`** (that broke every packaging job the
  first tag run). `.deb` refuses to build without package.json `description` + `author`. Failed
  package steps emit `::error` annotations because job logs need admin to download. Draft
  releases don't appear in the anonymous releases API.
- **CI monitoring without `gh`:** unauthenticated `api.github.com/repos/omarjson/LocalAide/...`
  (public repo). Filter runs by `head_branch` in a node one-liner — a bare grep can latch onto
  the sibling main-push run.
- This machine has **no Rust toolchain** — anything requiring one must compile in CI.
