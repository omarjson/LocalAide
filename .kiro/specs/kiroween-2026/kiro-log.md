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

## Still open

- A real-model chat and demo run on each platform (plan §8 test list, `demo-checklist.md`).
- macOS/package bundling in CI (see the status line in README.md).
- Image input exercised against a real vision model end to end.
- Inline markdown rendering in chat bubbles (fenced code blocks only today).
- Image lightbox; model catalog refresh mechanism.
