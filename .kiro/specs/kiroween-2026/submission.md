# Kiroween 2026 submission draft

Copy these sections into the Devpost form. Anything in **[brackets]** still needs your input.

**Category:** Frankenstein
**Repository:** https://github.com/omarjson/LocalAide (MIT license, `.kiro/` committed)
**Video:** [link, under 3 minutes; follow `demo-checklist.md`]

---

## Tagline

A patched-together ghost that lives on your computer: private AI chat, document search and memory, stitched from parts that were never meant to fit.

## Eligibility note (read first)

The first commit (`890c38c`) landed on October 30 at 06:58 PDT. That is before the opening hour given in our plan, and it held a small prototype: 4,137 lines of our own Rust and Svelte, plus 140,369 lines of vendored third-party llama.cpp bindings. Nearly all of it has since been deleted or rewritten. Today 700 of the 23,047 lines of app source (3.0%) trace to that commit. Every feature in the demo was built between October 30 and November 15. The full breakdown, with commands to reproduce each number, is in [`.kiro/specs/kiroween-2026/eligibility.md`](eligibility.md). **[Say here whether the organizers confirmed eligibility.]**

## What it does

LocalAide is a desktop assistant for people who aren't technical. You pick a model size that fits your RAM, it downloads and checks the file, and you chat. Nothing leaves your computer unless you turn on online mode.

- Streaming chat with Stop, retry and edit, memory citations, and image input for vision models.
- A 41-model catalog with SHA-256-checked, resumable downloads, plus any public GGUF from Hugging Face.
- Document library with keyword, vector and hybrid search and a reranker.
- Memory notes, saved prompts ("skills"), and workspace file tools that ask before every write and log every action.
- Opt-in online providers (any OpenAI-compatible API, your own key), web search, and encrypted backups.

## Why Frankenstein

LocalAide is a stitched body. Each part comes from a different world:

- **Brain:** llama.cpp's `llama-server`, a C++ binary, run as a child process on 127.0.0.1 with a random key. It runs three times over: one copy chats, one embeds, one reranks.
- **Nervous system:** a Rust core (Tauri 2) with 204 typed commands. One `InferenceEngine` trait hides whether the answer comes from that local process or from a cloud API.
- **Skin:** a Svelte 5 web UI inside a native window, under a strict content security policy.
- **Memory:** SQLite FTS5 keyword search fused with vector search (reciprocal rank fusion), then reranked.
- **Borrowed organs:** the Hugging Face hub for models, DuckDuckGo, Wikipedia, Jina, Exa and Firecrawl for the web, and any OpenAI-compatible provider for cloud chat.

The stitches show on purpose. The logo is a patched ghost with one ember eye. The project's own history is a Frankenstein story too: on November 13 the original in-process engine (140,000 vendored lines) was cut out and the llama-server sidecar sewn in, without changing the chat code above the trait.

## How we built it with Kiro

Kiro is how this project kept its claims honest while moving fast.

**Steering** (`.kiro/steering/`)
- `product.md`, `tech.md` and `structure.md` are always included. They hold the promises (offline by default, no telemetry), the conventions (typed IPC only, no `unwrap()` on user paths, every tool path through the sandbox) and a **"Verify before you claim"** rule that lists past doc claims that turned out false.
- `catalog.md` is included only when catalog files are open. It tells the agent to research models through the Hugging Face MCP server and to never type a hash by hand.

**Specs** (`.kiro/specs/`)
- `kiroween-2026/plan.md`: the decisions, the audit fix list, and work tracks A to E, each closed with commit hashes.
- `engine-upgrade/tasks.md`: the phased move to the sidecar engine, with DONE, PARTIAL and DEFERRED per item.
- `online-providers/`: a full requirements → design → tasks spec for online mode, with EARS acceptance criteria mapped to the code and commits. It was written from the shipped code and says so. Its tasks 8 to 10 (RemoteEngine HTTP tests, OS keychain storage, router tests) closed on November 22, so the spec's acceptance criteria now map to shipped code.
- `kiroween-2026/kiro-log.md`: a dated log tying each change to the Kiro artifact that drove it.

**Audits that fed the specs** (`docs/evidence/`) **[confirm these ran in Kiro; `kiro-log.md` also mentions a "ZCode session"]**
- `audit-2026-11-13.md` checked every feature claim against the code and Hugging Face. It found broken checksums, gated models and a vector search that never ran, and that list became plan §3.
- `audit-2026-11-14.md` ran four parallel audits (security, backend, release, UX). It produced the SEC/BE/UI/REL fix list, worked through in phases 0 to 5.

**Hooks** (`.kiro/hooks/`)
- `svelte-check-on-edit.kiro.hook`: runs `npm run check` when a Svelte or TypeScript file changes.
- `rust-and-catalog-checks.json`: runs clippy with `-D warnings` when Rust code changes, and the catalog parity test plus the Hugging Face pin check when the catalog changes. These match the CI gates, so failures show up before a push.

**MCP** (`.kiro/settings/mcp.json`)
- The official Hugging Face MCP server, used for model research: finding GGUF repos, and checking license, gating and freshness before a model enters the catalog.

**Skill** (`.kiro/skills/impeccable/`)
- A third-party design skill (Apache 2.0), installed on November 2. It drove the design pass, the empty states and the accessibility fixes.

**Vibe sessions and sub-agents**
- Larger rounds ran as parallel sub-agent tracks, each with its own files (plan §7): backend fixes, design system, docs truth pass, and online mode. Each ended with a written summary and a CI run.
- The submission-readiness review on November 15 ran in a Kiro Web session. It produced `eligibility.md`, the online-providers spec, the new hooks and the MCP config.

## Challenges

- **Replacing the engine mid-project.** The in-process bindings were from 2023 and couldn't load current models. Moving to a sidecar meant process lifecycle work: a loopback-only key, cleanup on exit, and a Windows job object so a hard kill doesn't leave orphaned processes.
- **Honesty under time pressure.** The first audit showed the docs claimed features the code didn't have. We made "verify before you claim" a steering rule and rewrote the docs to match the code.
- **CI as the only Rust runner.** Some sessions had no Rust toolchain, so CI failures were turned into check-run annotations that could be read without log access.

## What's next

- Run the installers and a real-model demo on each platform.
- Signed builds.

## Known limits (stated so judges don't have to find them)

- The installers are built in CI but haven't been run on real machines.
- Stop, image input and meaning search have not yet been checked against a real model.
- API keys live in the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service); `config.json` keeps only a `keyring:` marker, with an owner-only file fallback on machines with no native store.
- Builds are unsigned.
