# LocalAide pitch

## One-liner

A private, local-first desktop AI assistant: your documents and chats stay on
your machine, no telemetry, optional cloud with your own keys.

## 30-second pitch

LocalAide is an open-source desktop app (Tauri 2, Rust, Svelte 5, SQLite) that
runs quantized GGUF models locally through a `llama-server` sidecar over
`127.0.0.1`. Cloud mode is off by default; switch it on and requests go only to
a provider you configured yourself, with a cloud badge visible the whole time.

It isn't just a chat box: it indexes local folders into SQLite FTS5 (with
semantic/hybrid retrieval when a local embedding engine is running), recalls
memory notes with clickable citations, has a workspace-jailed file tool with
explicit consent for writes, reusable skills, web search, and a model catalog
with SHA-256 verified downloads. Installers exist for macOS, Windows and Linux.

## Who it is for

- **Privacy-minded users** who want an assistant that works offline and keeps
  documents, conversations and memory under `~/.aiassistant/`.
- **Local LLM hobbyists** who already download GGUF models and want a desktop
  front-end with retrieval, tools and citations rather than a bare server.
- **Contributors** who like Rust + Tauri + Svelte and want a codebase with typed
  IPC, ADRs, evidence-backed docs and a CI gate on every push.

## Differentiators worth leading with

1. **Actually local by default.** No browser runtime, no Electron shell, no
   telemetry; the inference engine is a local sidecar.
2. **Retrieval you can inspect.** FTS5 BM25 by default; semantic + hybrid with
   reciprocal-rank fusion and a local reranker when those engines are enabled.
   Memory notes render citation pills with snippet, path and score.
3. **Consent and jail.** File tools are confined to a capability-rooted
   workspace; reads are open, writes ask, and every call lands in a SQLite
   audit log.
4. **Opt-in online with no lock-in.** OpenAI-compatible providers, presets from
   the open 9Router registry, or local Ollama with no key.
5. **Verifiable distribution.** SHA-256 streaming checks on downloads, an
   integrity digest over the model catalog, checksums and an SBOM per release.

## Honest limitations to state up front

- Installers are **not code-signed** and have **not been run on real machines**
  in the demo loop yet. First launch shows OS warnings.
- Tool-calling and MCP have **not been exercised against a real tool-calling
  model or MCP server**.
- Catalog Ed25519 signing is future work; today it is an integrity digest.
- The quality scorecard lists **targets**, not achieved scores.
- The app log is not yet in the Diagnostics view; that view shows llama-server
  logs only.

## Short descriptions by channel

- **Repo description / metadata:** `Private, local-first desktop AI assistant — local GGUF models, RAG over your documents, zero telemetry. Tauri 2 + Rust + Svelte 5.`
- **One sentence for a forum signature:** `LocalAide — a local-first desktop AI assistant that keeps your chats and documents on your machine (MIT, Tauri/Rust/Svelte).`
- **Contributor hook:** `Looking for contributors: typed Rust IPC, Svelte 5 UI, local retrieval and a test-backed CI gate. Good-first issues welcome.`
