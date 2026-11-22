# r/LocalLLaMA — final post (ready to paste)

I'm the developer; disclosing that up front. Suggested flair: **Project** (or
**Discussion** if Project isn't available).

## Title

`LocalAide: local-first desktop AI chat with local document RAG and citations (GGUF via llama-server, Tauri/Rust/Svelte)`

## Body

I've been building a local-first desktop assistant, and the retrieval side has
finally got to a place worth showing.

The short version: it runs GGUF models through a `llama-server` sidecar over
127.0.0.1 — no browser runtime, no Electron shell — and keeps documents,
conversations and memory under `~/.aiassistant/`. Cloud mode is off by default;
if I do add an OpenAI-compatible provider, requests go only there with my own
key and the app shows a cloud badge.

The part I care about most is document search. Folders are indexed into SQLite
FTS5 with natural-boundary chunking (1500 chars, 250 overlap). BM25 is the
default. With a local embedding engine running it switches to semantic or
hybrid retrieval with reciprocal-rank fusion, and a local reranker can reorder
the fused candidates. Memory notes come back as clickable citations showing
snippet, path and score — so an answer has something inspectable behind it.

Also in there:

- token streaming with a Stop control that actually interrupts at the next token
- image input for vision models when the model has an `mmproj` projector
- skills you can pull in with `/`
- a workspace-jailed file tool: reads are open, writes ask first, every call
  lands in a SQLite audit log
- a 41-model curated catalog plus community GGUF browsing, with SHA-256 checked
  on streamed download before the file is renamed into place

Repo: https://github.com/omarjson/LocalAide
Releases (macOS/Windows/Linux installers): https://github.com/omarjson/LocalAide/releases

Caveats I'd want to know as a reader: installers aren't code-signed and I
haven't run them on real machines in my demo loop yet; tool calling and MCP are
implemented but not tested against a real tool-calling model or MCP server;
catalog "signing" today is an integrity digest, not Ed25519. All of that is in
the README and the audit rather than hidden.

Happy to answer questions about the retrieval pipeline, the sidecar setup or
why I chose FTS5 as the default.

## First comment (post right after publishing)

> If anyone wants to dig into the implementation: the retrieval design is in
> `ARCHITECTURE.md`, the storage and inference ADRs are `0002` and `0007`, and
> `docs/evidence/audit-2026-11-14.md` lists what's still open. I'd especially
> like feedback on the hybrid ranking and on the consent model for file writes.

## Reply snippets

- **"Another chat UI?"** — Fair. The distinguishing bit is local document
  retrieval with inspectable citations plus the jailed file tool; the chat is
  the way you reach it.
- **"Which models?"** — Any GGUF the sidecar can load, including vision models
  with an `mmproj` projector. The curated catalog is 41 models; community GGUFs
  can be browsed and downloaded with hash verification.
- **"Telemetry?"** — None. Everything stays under `~/.aiassistant/` unless you
  explicitly turn cloud mode on with your own provider and key.
- **"MCP?"** — Implemented, but not yet exercised against a real MCP server.
  That's an open item I've called out rather than claimed as working.
- **"Windows/macOS installers work?"** — They build in CI and are published with
  checksums, but I haven't done the real-machine demo run yet. If you try one,
  I'd appreciate a report — that's one of the current good-first issues.

## Posting notes

- Reddit's self-promotion rules allow developers posting their own work when
  they disclose it, but each sub sets its own limits. Read the sidebar first.
- Do not edit the post heavily after publishing; quote it in comments instead.
- Reply to every comment for the first hour. If a mod removes it, don't repost
  the same text — message them and ask what would fit.
- This post was prepared from the repo's README, audit and release assets; no
  claims were added beyond what's documented.
