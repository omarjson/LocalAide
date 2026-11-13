# LocalAide Quality Rubric & Scorecard

## Target Contract
- Every category must score **≥ 9**.
- Weighted Total must score **≥ 95** before a release sign-off.
- Any claim must be accompanied by automated test evidence or manual audit notes in `docs/evidence/`.
- A category's score is only final once its evidence link holds evidence produced by the code on this repository, verified by CI.

## Status

The standing honesty audit is [audit-2026-11-13.md](audit-2026-11-13.md). It lists every known gap between claims and code. Scores below are **targets for the current release gate, not achieved numbers** until CI and the demo run listed in `.kiro/specs/kiroween-2026/plan.md` pass on all platforms.

| Category | Weight | What a 10 looks like | Evidence |
|---|---:|---|---|
| **Product honesty & completeness** | 15 | Every control works; every shipped feature is end-to-end; no dead or fake UI | [Chat](phase5-chat.md), [Library](phase6-library.md), [Memory](phase7-memory.md), [Tools & Security](phase8-tools-security.md), [Honesty audit](audit-2026-11-13.md) |
| **Architecture & maintainability** | 15 | One state owner; typed edges; clear service boundaries; ADRs for all decisions | [IPC & State](phase3-ipc-state.md), [Library & SQLite WAL](phase6-library.md), [ARCHITECTURE.md](../../ARCHITECTURE.md), [ADRs](../adr/) |
| **Correctness & reliability** | 15 | No panics in user paths; jobs cancel/retry/resume; crash recovery works | [Library tests](../../../src-tauri/tests/library_test.rs), [Memory tests](../../../src-tauri/tests/memory_rag_test.rs), [Fault injection](../../../src-tauri/tests/fault_injection_test.rs) |
| **Security & privacy** | 15 | Real sandbox jail; catalog & download integrity; strict CSP; zero telemetry | [Security & jail](phase8-tools-security.md), [SECURITY.md](../../SECURITY.md) |
| **Performance & resource use** | 10 | Budgets enforced; no UI blocking; bounded memory & streaming | [Benchmarks](benchmarks.md) |
| **UX & accessibility** | 10 | WCAG 2.2 AA; keyboard-complete; clear states and errors | [A11y evidence & keyboard matrix](a11y.md) |
| **Testing & verification** | 10 | Full Rust test suite plus frontend typecheck passing in CI on every push | [ci.yml](../../../.github/workflows/ci.yml), [Benchmarks](benchmarks.md) |
| **Documentation & release** | 10 | Every claim evidence-backed; docs match the code; reproducible release automation | [README](../../README.md), [Scope](../scope.md), [Model evidence](models-2026-11.md) |

*Note: This scorecard replaces the earlier v1.0 score table, whose numbers had no recorded run behind them and whose evidence links pointed to local paths outside the repository. Scores will be filled in as CI runs and the manual demo checklist completes.*
