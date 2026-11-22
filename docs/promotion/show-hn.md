# Show HN draft — news.ycombinator.com

## Title options (pick one, keep it factual)

- `Show HN: LocalAide – a local-first desktop AI assistant that keeps your documents on your machine`
- `Show HN: LocalAide – offline GGUF chat with local document search and citations`
- `Show HN: LocalAide – Tauri/Rust desktop AI, zero telemetry, opt-in cloud`

## Post body

Hi HN,

I built LocalAide, an open-source desktop AI assistant that runs quantized GGUF
models locally and keeps documents, conversations and memory on the machine.
It's Tauri 2 + Rust + Svelte 5 + SQLite, MIT licensed.

Why another chat app: I wanted the assistant to search my own documents without
sending them anywhere, and to be able to see where an answer came from. So it
indexes folders into SQLite FTS5, does BM25 keyword retrieval by default, and
can do semantic/hybrid ranking with a local embedding engine plus a local
reranker. Memory notes come back as clickable citations with snippet, path and
score.

Other bits: token streaming with a working Stop, image input for vision models,
skills you can invoke with `/`, a workspace-jailed file tool where writes need
approval and every call is audited, and opt-in OpenAI-compatible providers for
when you do want the cloud (off by default, your own key).

Inference runs in a `llama-server` sidecar over 127.0.0.1, not in a browser or
Electron shell. The app downloads the engine for your CPU on first model load
(13–25 MB). Installers for macOS (x64/arm64), Windows (x64/arm64) and Linux
(.deb/AppImage) are on the releases page, with checksums and an SBOM.

Honest caveats: the installers are not code-signed and I haven't run them on
real machines in the demo loop yet. Tool calling and MCP are implemented but
haven't been exercised against a real tool-calling model or MCP server. The
quality scorecard in the repo is a set of targets, not achieved numbers.

Repo: https://github.com/omarjson/LocalAide
Releases: https://github.com/omarjson/LocalAide/releases
Architecture: https://github.com/omarjson/LocalAide/blob/main/ARCHITECTURE.md

I'd especially like feedback on the local retrieval design and on the consent
model for file tools. Contributions are welcome — the repo has ADRs, an
evidence-backed audit and a CI gate on every push.

## Notes for posting

- Post from your own HN account; don't use a brand-new account.
- Reply to comments yourself — HN rewards the author being present.
- If asked about signing or real-machine testing, link to the caveats above
  rather than promising a timeline you haven't set.
- Link to the specific commit or release you posted about so the post stays
  accurate after main moves.
