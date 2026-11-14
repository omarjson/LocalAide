# How Kiro shaped LocalAide

A running log of spec-driven work. Each entry names the change and the Kiro artifact that drove it.

## 2026-11-13 — Honesty audit drives the whole cycle

- **Kiro artifact:** `.kiro/specs/kiroween-2026/plan.md` §3, fed by `docs/evidence/audit-2026-11-13.md`.
- **What changed:** the audit's false claims became the fix backlog. Commits `07ed24c` (catalog: 36 verified models with licenses and presets) and `aa5daf6` (folder indexing fix; honest search; design pass and icons) came straight from the audit truth table.

## 2026-11-13 — Engine upgrade to a llama-server sidecar

- **Kiro artifact:** `.kiro/specs/engine-upgrade/tasks.md` (phases, DONE/TODO per item).
- **What changed:** commit `ea9ed68` deleted the vendored 2023 in-process engine and moved chat to a `llama-server` sidecar; `1596e83` formatted the result. ADR-0007 records the decision.

## 2026-11-13 — Docs truth pass

- **Kiro artifact:** plan §4 audit item 6 ("README, scope and ARCHITECTURE contain the false claims listed in the audit").
- **What changed:** README, ARCHITECTURE, docs/scope.md, SECURITY.md and docs/evidence/scorecard.md were rewritten so every claim matches the code: no more Ed25519/"signed manifest" wording, no invented search benchmark, chunk sizes corrected to 1500/250, keyboard matrix corrected to the real ⌘1–⌘5 mapping, audit log described as clearable, and the inflated rubric scores replaced with the CI gate list. The scorecard's evidence links now point into the repository instead of a local `file:///Users/omar/...` path.

## 2026-11-13 — Online mode built from the plan decision

- **Kiro artifact:** plan §0 decision ("online mode: build it in") and §4 track E.
- **What changed:** `RouterEngine` routes between the local sidecar and a new `RemoteEngine`; both sit behind the existing `InferenceEngine` trait, so the chat path, context budget, RAG citations, and cancellation work unchanged in either mode. Providers are managed by four new commands and a Settings panel (add/edit/delete/toggle) with an explicit privacy note; the chat header shows a cloud badge whenever cloud mode is active. Base URLs must be HTTPS unless they point at the user's own machine, and the API key is stored only in the local config file, masked in the UI. Docs (README, scope, ARCHITECTURE, SECURITY) were updated in the same pass so the "local by default" claim stays true.

## 2026-11-13 — Verification round 2 (ZCode session)

- **Kiro artifact:** plan §7 tracks A–E and this log as the evidence trail.
- **What changed:** a cloud-feature code review found 14 issues; the HIGH one (cloud SSE silently swallowing mid-stream errors and clean-close truncation) plus 10 actionable items were fixed in commit `afede44` (error surfacing, seen_done check, SSE data: tolerance, 60s idle-read timeout, RemoteTarget no longer Debug, startup fallback to local when provider invalid, corrupt-config fail-loud, delete-active-provider disables cloud mode, duplicate provider rejection, save-before-apply ordering, Settings autosave race removed, ChatScreen cloud-mode empty-state honesty). A repo staleness audit confirmed `vendor/llama_cpp_rs` unreferenced and flagged the Dev Config screen and stale docs for the fixes that landed afterward. The Kiro `npm run check` hook landed in `.kiro/hooks/svelte-check-on-edit.kiro.hook` (plan track D).
- **CI:** quality gate green on `118e4a5` (fmt, clippy, cargo test including cloud tests, svelte-check, build, smoke) with Windows and Linux llama-server sidecar builds green. macOS sidecar builds were cancelled twice by operator decision (macOS runners bill 10x on the free plan) — macOS/package bundling remains intentionally unverified, never failed.

## 2026-11-13 — Merge and final docs coherence pass

- **Kiro artifact:** this spec (plan Status), the steering docs, and `.kiro/specs/engine-upgrade/tasks.md`.
- **What changed:** `kiroween/engine-and-design` merged into `main` as commit `bb0d0aa` and the branch deleted; `main` is the branch of record. A final documentation coherence pass aligned README, ARCHITECTURE, SECURITY, scope, backlog, ADRs, evidence files, and the steering docs into one voice with no contradictions; the CHANGELOG folded `[Unreleased]` into a 0.1.0 section dated 2026-11-13 and recorded that the `v0.0.1` release and its tag were deleted the same day as superseded.

## 2026-11-13 — Open-gap round (after the polish round)

- **Kiro artifact:** this entry; `docs/evidence/project-map.html` feature board updated to match.
- **What changed:** watch stop (`index_watch_stop` drops the notify watcher, disconnecting the debouncer channel and ending the thread — the Search chip is now a stop button); preset cancel (`model_preset_cancel` sets every per-model job's cancel flag, with Cancel controls in the wizard and Models screen; cancelled installs are shown as cancelled, not failed); the previously-unwired `ToolApprovalModal` is now reachable — the Chat tool panel gained a write_file mode, a write that requires approval opens the modal, and Allow Once / Allow for Session (session token) / Deny all execute against the jail and write audit rows; Settings shows the app version (Tauri `getVersion`); Search can index a single file, not just folders.

## Decisions recorded (2026-11-13)

- Kiroween category: **Costume Contest**.
- Online mode: **build it in** as opt-in (plan §0).
- Speech: **defer** (plan §0, engine-upgrade Phase 5).

## 2026-11-14 — Inline markdown, lightbox, and the clippy gate made real

- **Kiro artifact:** this entry; `docs/evidence/project-map.html` board updated.
- **What changed:** chat bubbles render inline markdown (inline code, bold, italic, bare links) via a token pass through `{#each}` — never `{@html}` — so the strict CSP holds by construction. Image attachments open in a fullscreen lightbox (Esc/backdrop closes).
- **The clippy gate is now real:** `#![allow(clippy::all, unused)]` removed from `lib.rs`. The first unshielded clippy run reported 38 findings (6 unused imports; a dead method and struct field orphaned by the dead-code rounds; 18 collapsible-if converted to Rust 2024 let-chains; 3 missing `Default` impls; `sort_by_key`; `div_ceil`; `matches!`; `as_chunks`) plus 3 svelte a11y warnings on the lightbox dialog. All fixed across `0c85b7f`, `8c66e14`, `65bda98`.
- **CI:** Quality Gate GREEN on `65bda98` with `clippy -D warnings` earned, not suppressed. Windows + Linux sidecar builds green; macOS cancelled again by operator decision.

## 2026-11-14 — Modernization round: late-2026 currency, catalog v7, real retrieval, skills

- **Kiro artifact:** this entry; the six-track modernization plan approved by the operator; CHANGELOG [Unreleased].
- **Currency pass:** Node 24 (runners dropped Node 20 in September), actions v5, Vite 8, vite-plugin-svelte 7, vite-plugin-checker 1.0, TypeScript 5.9.3 (5.10/5.11 do not exist — pinned from the live registry), notify 8, llama.cpp sidecar v1.0.0, dead devDeps removed. Platform builds gated behind tags/`[build-all]` so routine pushes stop burning 10× macOS minutes.
- **Catalog v7:** six additions from live HF metadata (gpt-oss-20b MXFP4, DeepSeek-R1-0528-Qwen3-8B, Mistral Small 3.2, Llama 3.2 3B, Phi-4-mini, BGE-M3) — every one of the 42 pins re-verified against the Hugging Face API, and a permanent `catalog-pins` CI job now keeps them honest. Companion (mmproj) files finally download with their model — vision via the catalog was broken until now.
- **The dead hybrid search is alive:** the audit found `SearchMode::Semantic/Hybrid`, cosine + RRF, and a `chunks.embedding` column all shipped but unwired, with `embedding_status` admitting "not-wired". A local embedding engine (second `llama-server --embedding` instance, spawn pattern shared with the chat sidecar) now vectorizes chunks at ingest, backfills old ones, embeds queries for Search/Memory/chat RAG, and reports truthfully. Reranker second-stage is the planned follow-up.
- **Skills + honest adaptation:** markdown skill files with a slash-menu in chat and a full editor screen; persisted chat prefs (prompt/temperature/persona/context); a local usage counter suggests saving prompts you keep re-typing. No profiles, no network, no fake learning claims.
- **Diagnostics:** Settings → Diagnostics tab (subsystem health, llama-server log tail, display-only update check); a11y fix for the lightbox image that newer svelte-check flags as an error.
- **Deferred:** light theme (needs visual verification — no local Node toolchain), cloud backup (needs operator app registrations), reranker second stage.
- **CI:** gate cycles during this round were diagnosed through check-run annotations (svelte-check output, npm ERESOLVE logs); the diagnostics emitters stay in ci.yml as permanent failure reporting. The gate surfaced 10+ rustfmt diffs from the round's new code — applied via a temporary ubuntu-only fmt workflow (same push-to-branch trick as the lockfile) rather than hand-transcribed diffs — and then 7 real clippy/compile findings (missing trait import, u64/usize mismatch, deprecated `Nonce::from_slice` in aes-gcm 0.11, a test fixture behind the new field, a missing `mut`). Full job logs are readable with the stored git credential, which beats annotation caps.

## 2026-11-14 — Catalog v8: the operator rejected the v7 additions as outdated

- The v7 additions filled missing families with old generations (Llama 3.2 Sep 2024, Phi-4-mini Feb 2025, DeepSeek-R1-0528 May 2025, Mistral Small 3.2 Jun 2025, gpt-oss Aug 2025, BGE-M3 Jan 2024). The operator reviewed and rejected them; late-2026 research found newer, superior models in every role.
- v8 replaces all six: Spark-X2.6 4B/1.7B (Oct 2026, on-device agent models, Apache-2.0), GLM-5.3 Flash (Aug 2026, 320B-A18B MoE, vision, MIT), Mistral Small 4 (Mar 2026, 119B MoE, native MXFP4, vision, Apache-2.0), and Nomic Embed v2 MoE (official llama.cpp quantization). DeepSeek's V4.1-Flash was rejected on size — its smallest quant is 217 GB — and no small V4-family distill exists; Qwen3-VL-Embedding was rejected pending llama.cpp embedding-support verification.
- Split-GGUF support: the catalog entry gained an optional primary `dest` override plus shard companions, so llama.cpp's `-00001-of-000NN` shard naming survives download and the engine loads all shards; the ModelInfo vision flag now requires an actual mmproj companion (shard companions are not vision); `verify-catalog.mjs` verifies shard totals against the hub.
- The catalog generator now lives in the repo (`scripts/gen-catalog.ps1`): it parses catalog.rs into models.json and recomputes the integrity digest, replacing the session-local script from the v7 round.

## 2026-11-14 — Model catalog refresh (ADR-0008)

- **Kiro artifact:** the open-gap list this log tracks; ADR-0008 records the trust model.
- **What changed:** the Models screen can now refresh the catalog without an app update. The manifest lives at a pinned URL in this repository (`catalog/models.json`, generated from `default_manifest()` and kept identical by a CI parity test). A refresh is explicit, streamed under a 512 KB cap, verified against the integrity digest, structurally validated (known kinds, unique ids, hex SHA-256 pins, HTTPS-only URLs, bounded counts), and rejected if older than the active version. Accepted manifests persist to `catalog-cache.json` (atomic temp+rename) and load at startup; Reset restores the built-in list. New commands: `model_catalog_status`, `model_catalog_refresh`, `model_catalog_reset`. The Models screen shows catalog version/source with Refresh and Reset controls, and model cards now carry their real kind (chat/code/embedding/reranker) instead of a hardcoded "LLM" label.
- **CI:** Quality Gate GREEN on `2684235` — first CI run of `cargo test` with the parity test proving `catalog/models.json` ≡ `default_manifest()`. Getting there surfaced two compile-time failures (a `Result<CatalogManifest>` vs `Arc<CatalogManifest>` signature mismatch and a contract-test fixture missing `ModelInfo.kind`); a diagnostics branch plus check-run annotations made CI compile errors readable without admin log access, then the scaffolding was removed once green.

## Still open

- A real-model chat and demo run on each platform (plan §8 test list, `demo-checklist.md`).
- macOS/package bundling in CI (see the status line in README.md). Platform builds are now gated to release tags, manual dispatch, or `[build-all]` commits — they no longer run on every push.
- Image input exercised against a real vision model end to end (companions now download with the model, so this is testable for the first time).
- Light theme (needs visual verification, which requires a local Node toolchain).
- Cloud backup providers (OneDrive/Drive need operator app registrations; local-folder backups shipped with ADR-0009).
- Binary code signing / notarization (ADR-0005).
