# Reddit drafts

Post from your own account. Read each subreddit's self-promotion rules first;
several require a comment/post history and some ban launch posts entirely.
Answer comments in the first hour — that matters more than the post itself.

## r/LocalLLaMA — "Project" or "Discussion" flair

**Title:** `LocalAide: local-first desktop assistant with GGUF chat, local document RAG and citations (Tauri/Rust/Svelte, MIT)`

**Body:**

I've been building a local-first desktop assistant and wanted to share where
it's at, especially the retrieval side.

It runs GGUF models through a `llama-server` sidecar on 127.0.0.1 (no browser
runtime, no Electron). Cloud is off by default; if I turn it on, requests go
only to a provider I configured with my own key.

What I care about most: document search. Folders are indexed into SQLite FTS5
with natural-boundary chunking (1500 chars / 250 overlap). BM25 is the default;
when a local embedding engine is running, it switches to semantic or hybrid
with reciprocal-rank fusion, and a local reranker can reorder the fused top
candidates. Memory notes come back with clickable citations showing snippet,
path and score.

Also in there: streaming with working Stop, image input for vision models,
skills via `/`, a workspace-jailed file tool where writes need approval, an
audit log, and verified model downloads (SHA-256 streamed checks, 41-model
curated catalog plus community GGUFs).

Repo: https://github.com/omarjson/LocalAide
Releases: https://github.com/omarjson/LocalAide/releases

Caveats I'd want to know as a reader: installers aren't code-signed and haven't
been run on real machines in my demo loop; tool calling/MCP aren't tested
against a real tool-calling model yet. Happy to answer questions about the
retrieval pipeline or the sidecar setup.

## r/selfhosted — only if self-hosting is the actual angle

**Title:** `LocalAide – offline-first desktop AI chat with local document index (no server to host)`

**Body:**

Not a server app — a desktop app that keeps everything under `~/.aiassistant/`
and talks to a local `llama-server` over loopback. No telemetry, no account, no
cloud unless you explicitly add a provider and switch cloud mode on.

- SQLite WAL for the document index, notes and audit log; conversation threads
  are JSON files on disk.
- FTS5 BM25 search over folders you index; optional local embeddings + reranker.
- Model downloads are SHA-256 verified; each release ships checksums and an SBOM.
- Installers for macOS, Windows and Linux (unsigned at the moment).

Repo: https://github.com/omarjson/LocalAide

If mods remove this as promotional, no problem — I'll leave the repo link for
anyone who's curious.

## r/opensource / r/programming — contributor angle

**Title:** `Looking for contributors: LocalAide, a local-first AI desktop app (Tauri 2, Rust, Svelte 5)`

**Body:**

MIT-licensed project with typed Rust IPC, ADRs for the architecture decisions,
a test suite (Rust unit/contract/fault-injection plus frontend unit tests), and
CI on Linux/Windows for fmt, clippy `-D warnings`, tests, svelte-check, build
and smoke test.

Areas where help is most useful: real-machine installer testing, accessibility
measurement on screen, a diagnostics view for the app log, and testing tool
calls against a real tool-calling model or MCP server.

Repo: https://github.com/omarjson/LocalAide
Contributing: https://github.com/omarjson/LocalAide/blob/main/CONTRIBUTING.md

## Don't

- Don't post the same text to several subs on the same day; stagger them.
- Don't claim "production ready" or "fully tested" — the README and audit
  explicitly say otherwise.
- Don't use a fresh account with no history; most subs will remove the post.
