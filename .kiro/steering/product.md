---
inclusion: always
---

# LocalAide: Product

LocalAide is a private, local-first desktop AI assistant. It runs on the user's own computer.

## Who it is for

A non-technical user. They open the app, pick a small local model, and chat in plain English. Write UI copy, errors and help text in plain language. Avoid jargon the user will not know.

## Core promises

- **Offline inference by default.** Chat runs on a local GGUF model through the llama-server sidecar (ADR-0007). With cloud mode off — the default — the chat path makes no network calls.
- **Zero telemetry.** No analytics, crash beacons or remote logging.
- **Local by default.** An opt-in Online Providers mode lets the user bring their own OpenAI-compatible API key and route chat to that provider; the key stays in the local config file and is sent only to the provider's base URL. Changing a provider's address to another site requires the key again. The mode is off unless the user turns it on (decided 2026-11-13, `.kiro/specs/kiroween-2026/plan.md` §0). No subscription is required for any shipped feature.
- Network access happens only for explicit user actions: downloading a model (built-in catalog or Hugging Face search), a web search or "Read page" on the Search screen, "Check for updates" or "Install and restart" in Settings → About, browsing or importing skills from GitHub on the Skills screen, "Load models" for an online provider, a web search or page read the model makes in chat while the **Web** switch is on, a tool call to a remote MCP server the user added, "Load models"/"Refresh" for a provider, linking or sending a task to OpenCode, and — when the user switches cloud mode on — chat requests to the provider they configured. Nothing is contacted at startup.

## Key features (0.1.0, verified in code)

- **Local chat.** Streaming tokens, stop with Esc (anywhere on the Chat screen) or the Stop button, and a sliding context budget that drops the oldest turns first and keeps the system prompt and latest message. The saved model loads in the background after the window opens.
- **Image input with vision models.** Up to 4 attached images; the sidecar loads the model's `mmproj` projector. Text-only models reject images with a readable error. Not yet exercised against a real vision model end to end.
- **Model setup and manager.** A setup wizard picks a preset from the machine's RAM and the "Memory for models" share in Settings; it can be skipped. The model manager downloads GGUF files from the built-in catalog, and Cancel stops a download.
- **Verified downloads.** Each file is hashed with SHA-256 in 64 KB chunks and checked against the catalog value before it becomes active. Downloads resume with HTTP Range requests and write to `.part` files first. A free-space check runs first.
- **Document library.** The user adds folders or files with the system picker. Files are chunked (1500 characters, 250 overlap) and stored in SQLite. Keyword search uses FTS5 BM25. Meaning and hybrid search use vectors when the local embedding engine is running (Search settings → "Turn on meaning search", with an embedding model installed); otherwise they fall back to keywords. Unchanged files are skipped on re-index.
- **Memory notes.** Markdown notes in `~/.aiassistant/memory/`, indexed in SQLite. Chat recalls notes above a relevance score of 0.010 and shows numbered citations. Two notes with one title are both kept.
- **Workspace tools.** Read, list and search files under `~/.aiassistant/workspace/`. Writes need explicit user approval. Every action, including refused ones, gets a row in the `tool_audit` table; a write's row is written before the file is touched.
- **Keyboard navigation.** Ctrl/Cmd+K command palette, Ctrl/Cmd+N new chat, `/` search, Ctrl/Cmd+1 to 6 screens, Esc to stop or close menus and dialogs, `?` for help. Menus support Escape, outside clicks and arrow keys.

## Known gaps (checked in code; keep docs honest about them)

- **Meaning search is not runtime-verified.** The code writes chunk vectors and passes a query vector when the embedding engine runs, but no real embedding model was driven end to end in the 2026-11-14 audit.
- **The catalog "signature" is a SHA-256 digest, not Ed25519.** It checks the catalog for internal consistency only. It does not prove who published it.
- **Built-in workspace tool actions are list, read, write and search.** LocalAide has no shell tool. MCP servers the user adds can do more (a local MCP server is a program running with the user's permissions); their tools ask before they run unless the user trusts the server.
- **The audit log is not append-only.** "Clear" exists; it leaves one row that says how many entries it removed.
- **Image input is code-complete but not runtime-verified.** No real vision model has been driven end to end yet; add that check before demoing the feature.
- **Releases are not code-signed.** Signing is wired to optional CI secrets. The in-app updater (Settings → About, user-confirmed, signature-checked) is built in since 0.1.2, but it only offers installs once the owner creates the update key pair (`docs/release.md`); until then it links to the release page.

## Non-goals for this release

- Voice (speech to text and text to speech). Deferred by ADR-0006.
- Vision beyond the shipped image-input path: no screen capture, no Candle vision models.
- Cloud inference is never a silent fallback. The only cloud path is the opt-in Online Providers mode above; the app never ships a provider, a key, or an automatic failover to the cloud.
- Hot-swapping components in the app. Updates ship as whole signed packages (ADR-0005).
- Any telemetry.

## Tools, thinking and the model picker (0.1.4, ADR-0012)

- The model decides when to call tools (`services/agent/`). Web, Notes, Files, Skills and each MCP server are permissions, not automatic actions. Anything that changes a file or note, and MCP tools unless the server is trusted, waits for the user's OK in the chat. Every tool call gets an activity-log row.
- Thinking streams as its own event and shows in a collapsible block. Effort is sent per model.
- The picker lists local models plus every enabled provider's models. A message goes online only when the picked model is online.
- Not runtime-verified yet: real llama-server tool calls, real providers and real MCP servers.

## Chat modes and developer mode (0.1.3)

- Modes (Standard, Strict, Unrestricted, Autonomous) live in `services/inference/modes.rs`; each adds one instruction to the system prompt, and the same table feeds the UI. Unrestricted must keep its last sentence (law and serious harm). Autonomous gathers web context only when Web is on.
- Developer mode (`commands/developer.rs`): every flag and module there must change real behaviour. Modules default on and gate both the UI and, for recall and web, the backend.

## Legal

- `PRIVACY.md` and `TERMS.md` ship in the app (Settings → About → Legal, the command palette, the setup wizard) and as the Windows installer's license page. Keep them true to the code: when a feature adds a network call or stores something new, update `PRIVACY.md` in the same change.

## Principles

- Every shipped control must work. No dead buttons, no mock data.
- Every error shows a readable message. The app must not crash on a failed model load, download or index.
- Local data stays local. Keep paths under `~/.aiassistant/`.
