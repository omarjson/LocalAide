# Social posts

Short posts that work on X, Mastodon and Bluesky. Include a screenshot or a
15-second clip of the chat + citations if you have one; text alone converts
much worse for a desktop app.

## Thread (X / Mastodon)

**1/5**

I spent the last while building LocalAide: a local-first desktop AI assistant.
Your chats, documents and memory stay on your machine. No telemetry. Cloud is
off unless you turn it on with your own key.

MIT, Tauri 2 + Rust + Svelte 5 + SQLite.

**2/5**

It runs GGUF models through a `llama-server` sidecar over 127.0.0.1 — no browser
runtime, no Electron shell. The engine downloads on first model load (13–25 MB).

**3/5**

The part I care about most: it indexes your folders into SQLite FTS5, searches
with BM25 by default, and can use local embeddings + a local reranker for
semantic/hybrid ranking. Memory notes come back with clickable citations.

**4/5**

Files are jailed to a workspace folder. Reads are open; writes ask first. Every
tool call lands in an audit log you can view and clear.

**5/5**

Installers for macOS, Windows and Linux, with checksums and an SBOM.
Caveats: unsigned installers, not yet run on real machines in my demo loop;
tool calling/MCP not tested against a real tool-calling model.

Repo: https://github.com/omarjson/LocalAide
Releases: https://github.com/omarjson/LocalAide/releases

## Single posts

- `LocalAide is a local-first desktop AI assistant: GGUF models via a local sidecar, SQLite document search, citation pills, zero telemetry. MIT. https://github.com/omarjson/LocalAide`
- `Looking for contributors: LocalAide (Tauri 2 / Rust / Svelte 5) needs real-machine installer testing, a11y measurement and tool-calling validation. ADRs + CI gate in the repo. https://github.com/omarjson/LocalAide/blob/main/CONTRIBUTING.md`
- `LocalAide v0.2.1 is out: Windows/macOS/Linux installers, SBOM + SHA256SUMS. Offline by default, your documents never leave the machine. https://github.com/omarjson/LocalAide/releases`

## Timing

- Post a thread on a weekday morning (EU or US, depending on your audience).
- Reply to the first replies yourself within minutes.
- Pin the repo or the release post to your profile for a week.
- Re-post the contributor version a week later, not the same day as the launch.
