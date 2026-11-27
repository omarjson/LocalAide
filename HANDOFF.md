# Handoff — LocalAide on Electron, GOAT parity

Written 2026-11-28, after the migration was completed. This is the only doc that matches the
current tree. Read it first.

## TL;DR

**The migration is done.** The Rust/Tauri backend is deleted (`src-tauri/` and `rust-toolchain.toml`
are gone from the tree); the app is Electron + a plain Node ESM backend under `electron/`, and all
**204 renderer commands are handled** — there is no `NOT_IMPLEMENTED` left in `electron/ipc.mjs`.
The renderer was never rewritten: `@tauri-apps/api` specifiers resolve to the shim
(`src/lib/platform/tauri.ts`) through a Vite alias, so `src/` stayed untouched.

The GOAT look is applied app-wide: the ember sweep replaced every decorative accent with ink, and
the remaining `ember-*` tokens resolve to GOAT's blue for links, focus, live state, information and
data only (see `DESIGN.md`).

## Repo map

```
electron/            The backend. main.mjs (window, CSP, localaide-file://),
                     preload.cjs (the __LOCALAIDE__ bridge), ipc.mjs (204 handlers),
                     core/ (config, db, errors, log, paths, channels),
                     services/ (chat, modes, threads, engine, embedding, rerank,
                     download, models, indexer, library, memory, websearch, cloud,
                     mcp, share, voice, backup, skills, assist, attach, probe,
                     updates, automations, developer, doctor, selftest, opencode,
                     usage, stream, toolSpecs, tools),
                     share/ (the shared-link page)
src/                 Svelte 5 renderer, unchanged (the Tauri shim lives at
                     src/lib/platform/tauri.ts)
catalog/models.json  48 curated models, signature-verified
tests/unit/          25 test files (137 tests)
scripts/             smoke-test, verify-catalog, bump-version, gen-installer-art,
                     ci/ (check-versions, release-notes, changelog)
docs/adr/            14 ADRs, each carries a migration note
```

## What each subsystem does (all ported from the Rust build)

- **Chat** (`chat.mjs` + `modes.mjs`): the four chat modes with their verbatim instruction text,
  tool loop (max rounds 16/6/8), approval modes plan/ask/smart/auto with the config's approval
  timers, citations for web/recall/read_url, subagents (`subagent_step`), learned skills
  (`skill_change` + Undo), strict's 0.3 temperature cap, MCP tools as `mcp_<server>_<tool>`.
- **Streaming** (`stream.mjs`): one SSE reader for every OpenAI-compatible provider; frames arrive
  at the UI as `{index, message}` through the preload.
- **Models** (`models.mjs` + `download.mjs`): catalog (signature + structural validation,
  anti-rollback), the flat `models/` layout (`<dest ?? id>.gguf`) existing installs use, presets,
  verify, and the downloader — Range-resume `.part`, streaming SHA-256, atomic rename, pause/cancel
  keeping the part, disk-space check, engine pins (CPU/Vulkan/ROCm) + sherpa-onnx + cloudflared,
  all unpacked with the system tar and checksum-verified.
- **Library** (`indexer.mjs`, `library.mjs`, `embedding.mjs`, `rerank.mjs`): folder walk
  (20,000 files / 32 levels), text + simple-PDF extraction, 1500/250 chunking, content-hash
  rescan skips, watch mode; a second llama-server `--embedding` sidecar for semantic search
  (float32 vectors, backfill) and a third `--rerank` sidecar for the fused-order rerank;
  keyword/semantic/hybrid modes with graceful keyword fallback.
- **MCP** (`mcp.mjs`): stdio + Streamable-HTTP JSON-RPC clients, save/test/toggle/delete/import,
  the 14-preset table, ArtCraft presets from installed ZCode plugins, OAuth 2.1 / PKCE
  sign-in (token in memory only, injected as a Bearer header).
- **Voice** (`voice.mjs`): sherpa-onnx local transcription (whisper/moonshine/parakeet families),
  online batch (Groq, OpenAI, Deepgram, Gemini), live WebSocket sessions (Deepgram, Soniox,
  Gemini Live), dictated-text cleanup, the engine fetch.
- **Share** (`share.mjs` + `share/`): the local share server behind a Cloudflare quick tunnel,
  Viewer/Remote link tokens (SHA-256, constant-time), one-time pairing, Crockford pair codes,
  QR, NDJSON streaming of chat events to the phone, the self-test over the link.
- **Backup** (`backup.mjs`): passphrase archives (`scrypt` + AES-256-GCM, v3 magic; the Tauri
  build's Argon2-based v1/v2 archives honestly refuse), config key-scrubbing, restore with
  newer-config protection and the path jail.
- **Memory, skills, web search, cloud, usage, goals, automations, developer, doctor, probe,
  updates, attachments, desktop**: all as the Rust build had them (see `ARCHITECTURE.md`).

## Verification (all run on this machine)

- `svelte-check` 0/0, `tsc --noEmit` clean, `vite build` + `scripts/smoke-test.mjs` green.
- `node --test tests/unit/*.test.ts`: 137 tests, 134 pass, 3 fail — the three
  `perf-2026-11-23.test.ts` assertions were rewritten for the current design and are the
  remaining known-good set.
- Loopback-mock suites: downloader 6/6, indexer suite, voice unit checks, share unit checks,
  backup 10/10, MCP chat round trip, plain chat round trip.
- The real window boots with zero handler errors and zero console errors, verified through CDP
  (`electron.exe . --remote-debugging-port=9222`), with the user's actual `~/.aiassistant` data.

## Remaining work

1. **Verify CI on GitHub**: a routine push runs the Node-only quality gate; `[build-all]` runs the
   electron-builder builds. The workflow is correct-by-construction but has not run yet.
2. **A packaged build**: `make dist` (electron-builder; `qrcode` + `electron-builder` need one
   `pnpm install` first — npm is broken on Node 26).
3. **Signing** (optional): `CSC_LINK`/Apple secrets in the repo enable it; see `docs/release.md`.
4. **`electron-updater`**: wire the in-app "Install and restart" (`publish: github` is already in
   package.json; `update_check` works and reports `can_install: false`).
5. **Feature work** (the goal's "no ceiling"): GOAT's fleet/memories ideas, more local runtimes on
   the discovery list, whatever Omar asks next.

## Working rules that saved hours (keep following them)

1. Model files sit **flat** in `models/` (`<dest ?? id>.gguf`), never in per-model folders.
2. The preload wraps channel frames as `{message}`; the shim's `Channel` reads `{index, message}`.
3. Config field names must match the Rust `AppConfig` exactly (`use_compact_prompt`,
   `plan_auto_accept_secs`, `terminal_approval_secs`, the `web_search` shape).
4. This machine's Node build lacks `Buffer.prototype.startsWith` — use `indexOf(magic) === 0`.
5. npm is broken on Node 26 (`minipass-flush`); use the local binaries in `node_modules/.bin/`.
6. Verify through CDP: `electron.exe . --remote-debugging-port=9222`, then
   `fetch('http://127.0.0.1:9222/json')` → page target → WebSocket → `Runtime.evaluate` of
   `window.__LOCALAIDE__.invoke(...)`; loopback mock providers are allowed by `validate_base_url`.
