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
- **Local by default.** An opt-in Online Providers mode lets the user bring their own OpenAI-compatible API key and route chat to that provider; the key stays in the local config file and is sent only to the provider's base URL. The mode is off unless the user turns it on (decided 2026-11-13, `.kiro/specs/kiroween-2026/plan.md` §0). No subscription is required for any shipped feature.
- Network access happens only for explicit user actions: downloading a model from the built-in catalog, and — when the user switches cloud mode on — chat requests to the provider they configured. No update check is wired yet (ADR-0005).

## Key features (0.1.0, verified in code)

- **Local chat.** Streaming tokens, stop with Esc or the Stop button, and a sliding context budget that drops the oldest turns first and keeps the system prompt and latest message.
- **Image input with vision models.** Up to 4 attached images; the sidecar loads the model's `mmproj` projector. Text-only models reject images with a readable error. Not yet exercised against a real vision model end to end.
- **Model setup and manager.** A setup wizard picks a preset from the machine's RAM. The model manager downloads GGUF files from the built-in catalog.
- **Verified downloads.** Each file is hashed with SHA-256 in 64 KB chunks and checked against the catalog value before it becomes active. Downloads resume with HTTP Range requests and write to `.part` files first.
- **Document library.** The user adds folders. Files are chunked (1500 characters, 250 overlap) and stored in SQLite. Search runs FTS5 BM25 keyword matching. See the known gap below.
- **Memory notes.** Markdown notes in `~/.aiassistant/memory/`, indexed in SQLite. Chat recalls notes above a relevance score of 0.010 and shows numbered citations.
- **Workspace tools.** Read, list and search files under `~/.aiassistant/workspace/`. Writes need explicit user approval and are logged to the `tool_audit` table.
- **Keyboard navigation.** Ctrl/Cmd+K command palette, Ctrl/Cmd+N new chat, `/` search, Ctrl/Cmd+1 to 5 screens, Esc to stop or close, `?` for help.

## Known gaps (checked in code; keep docs honest about them)

- **Hybrid search is keyword-only.** Chunk embeddings are never written, and the search path never passes a query vector. The vector side and the RRF fusion code exist but never run. The embedding engine also falls back to a deterministic placeholder until a real model file is present.
- **The catalog "signature" is a SHA-256 digest, not Ed25519.** It checks the catalog for internal consistency only. It does not prove who published it.
- **Tool actions are list, read, write and search.** Command execution does not exist, so there is no execution approval path.
- **The audit log is not append-only.** A `clear_tool_audits` function exists.
- **Image input is code-complete but not runtime-verified.** No real vision model has been driven end to end yet; add that check before demoing the feature.

## Non-goals for this release

- Voice (speech to text and text to speech). Deferred by ADR-0006.
- Vision beyond the shipped image-input path: no screen capture, no Candle vision models.
- Cloud inference is never a silent fallback. The only cloud path is the opt-in Online Providers mode above; the app never ships a provider, a key, or an automatic failover to the cloud.
- Hot-swapping components in the app. Updates ship as whole packages; updater signing is future work (ADR-0005).
- Any telemetry.

## Principles

- Every shipped control must work. No dead buttons, no mock data.
- Every error shows a readable message. The app must not crash on a failed model load, download or index.
- Local data stays local. Keep paths under `~/.aiassistant/`.
