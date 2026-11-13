# How Kiro shaped LocalAide

A running log of spec-driven work. Each entry names the change and the Kiro artifact that drove it.

## 2026-11-13 — Honesty audit drives the whole cycle

- **Kiro artifact:** `.kiro/specs/kiroween-2026/plan.md` §3, fed by `docs/evidence/audit-2026-11-13.md`.
- **What changed:** the audit's false claims became the fix backlog. Commits `07ed24c` (catalog: 36 verified models with licenses and presets) and `aa5daf6` (folder indexing fix; honest search; design pass and icons) came straight from the audit truth table.

## 2026-11-13 — Engine upgrade to a llama-server sidecar

- **Kiro artifact:** `.kiro/specs/engine-upgrade/tasks.md` (6 phases, DONE/TODO per item).
- **What changed:** commit `ea9ed68` deleted the vendored 2023 in-process engine and moved chat to a `llama-server` sidecar; `1596e83` formatted the result. ADR-0007 records the decision.

## 2026-11-13 — Docs truth pass

- **Kiro artifact:** plan §4 audit item 6 ("README, scope and ARCHITECTURE contain the false claims listed in the audit").
- **What changed:** README, ARCHITECTURE, docs/scope.md, SECURITY.md and docs/evidence/scorecard.md were rewritten so every claim matches the code: no more Ed25519/"signed manifest" wording, no invented search benchmark, chunk sizes corrected to 1500/250, keyboard matrix corrected to the real ⌘1–⌘5 mapping, audit log described as clearable, and the inflated rubric scores replaced with the CI gate list. The scorecard's evidence links now point into the repository instead of a local `file:///Users/omar/...` path.

## Decisions recorded (2026-11-13)

- Kiroween category: **Costume Contest**.
- Online mode: **build it in** as opt-in (plan §0).
- Speech: **defer** (plan §0, engine-upgrade Phase 5).

## 2026-11-13 — Online mode built from the plan decision

- **Kiro artifact:** plan §4 scope ("Online mode (approved 2026-11-13)") and track E.
- **What changed:** `RouterEngine` routes between the local sidecar and a new `RemoteEngine`; both sit behind the existing `InferenceEngine` trait, so the chat path, context budget, RAG citations, and cancellation work unchanged in either mode. Providers are managed by four new commands and a Settings panel (add/edit/delete/toggle) with an explicit privacy note; the chat header shows a cloud badge whenever cloud mode is active. Base URLs must be HTTPS unless they point at the user's own machine, and the API key is stored only in the local config file, masked in the UI. Docs (README, scope, ARCHITECTURE, SECURITY) were updated in the same pass so the "local by default" claim stays true.

## Pending Kiro evidence

- A Kiro hook that runs `npm run check` on save (plan track D).
- Steering docs live in `.kiro/steering/` (product, structure, tech) and keep the brand voice consistent.

## 2026-11-13 — Verification round 2 (ZCode session, continued)

- Cloud-feature code review (subagent): 14 findings; the HIGH (cloud SSE silently swallowing mid-stream errors and clean-close truncation) plus 10 actionable items fixed in commit afede44 (error surfacing, seen_done check, SSE data: tolerance, 60s idle-read timeout, RemoteTarget no longer Debug, startup fallback to local when provider invalid, corrupt-config fail-loud, delete-active-provider disables cloud mode, duplicate provider rejection, save-before-apply ordering, Settings autosave race removed, ChatScreen cloud-mode empty-state honesty).
- Repo staleness audit (subagent): vendor/llama_cpp_rs confirmed unreferenced (deletion shipped this round), Dev Config dead button + hardcoded embedding-status stub flagged for fix, stale steering/ADR/evidence claims listed for the second docs pass (this commit).
- Opportunity scan (subagent): 8 code-only additions identified; adopted now: model-manager error surfacing, Kiro npm-run-check hook, last UI honesty strings; deferred: setup wizard live progress + handoff, real Cmd+N new-thread wiring, a11y focus/aria work, RAM-budget warning badges.
- CI: quality gate green on 118e4a5 (fmt, clippy, cargo test incl. cloud tests, svelte-check, build, smoke) with Windows + Linux llama-server sidecar builds green; macOS sidecar builds cancelled twice by operator decision (macOS runners bill 10x on free plan) — macOS/package bundling remains intentionally unverified.
