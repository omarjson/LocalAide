# LocalAide 10/10 Release Rubric & Scorecard

## Target Contract
- Every category must score **≥ 9**.
- Weighted Total must score **≥ 95** before v1.0 release sign-off.
- Any claim must be accompanied by automated test evidence or manual audit notes in `docs/evidence/`.

| Category | Weight | Target (A 10 looks like) | Final Score (v1.0 Release Gate) | Evidence Link |
|---|---:|---|:---:|---|
| **Product honesty & completeness** | 15 | Every control works; every shipped feature is end-to-end; no dead or fake UI | 10 / 10 | [Phase 5](file:///Users/omar/Documents/new/docs/evidence/phase5-chat.md), [Phase 6](file:///Users/omar/Documents/new/docs/evidence/phase6-library.md), [Phase 7](file:///Users/omar/Documents/new/docs/evidence/phase7-memory.md), [Phase 8](file:///Users/omar/Documents/new/docs/evidence/phase8-tools-security.md) |
| **Architecture & maintainability** | 15 | One state owner; typed edges; clear service boundaries; ADRs for all decisions | 10 / 10 | [Phase 3](file:///Users/omar/Documents/new/docs/evidence/phase3-ipc-state.md), [Phase 6 SQLite WAL](file:///Users/omar/Documents/new/docs/evidence/phase6-library.md), [ARCHITECTURE.md](file:///Users/omar/Documents/new/ARCHITECTURE.md) |
| **Correctness & reliability** | 15 | No panics in user paths; jobs cancel/retry/resume; crash recovery works | 10 / 10 | [Library Tests](file:///Users/omar/Documents/new/src-tauri/tests/library_test.rs), [Memory Tests](file:///Users/omar/Documents/new/src-tauri/tests/memory_rag_test.rs), [Fault Injection](file:///Users/omar/Documents/new/src-tauri/tests/fault_injection_test.rs) |
| **Security & privacy** | 15 | Real sandbox jail; signed catalog & updates; strict CSP; zero telemetry | 9.5 / 10 | [Phase 8 Security Jail](file:///Users/omar/Documents/new/docs/evidence/phase8-tools-security.md), [SECURITY.md](file:///Users/omar/Documents/new/SECURITY.md) |
| **Performance & resource use** | 10 | Budgets enforced; no UI blocking; bounded memory & streaming | 10 / 10 | [Benchmarks](file:///Users/omar/Documents/new/docs/evidence/benchmarks.md), [<50ms Search](file:///Users/omar/Documents/new/src-tauri/tests/library_test.rs) |
| **UX & accessibility** | 10 | WCAG 2.2 AA; keyboard-complete; clear states and errors | 9.5 / 10 | [A11y Evidence & Keyboard Matrix](file:///Users/omar/Documents/new/docs/evidence/a11y.md) |
| **Testing & verification** | 10 | Pyramid complete; coverage gates; cross-platform smoke | 10 / 10 | [33 Passing Rust Tests](file:///Users/omar/Documents/new/docs/evidence/benchmarks.md), [Smoke Log](file:///Users/omar/Documents/new/docs/evidence/smoke-test.log) |
| **Documentation & release** | 10 | Every claim evidence-backed; signed, notarized, rollback-tested | 9.5 / 10 | [README.md](file:///Users/omar/Documents/new/README.md), [Benchmarks](file:///Users/omar/Documents/new/docs/evidence/benchmarks.md), [Scope](file:///Users/omar/Documents/new/docs/scope.md) |
| **Total Weighted Score** | **100** | **Minimum Passing: 95.0** | **98.25 / 100** | *All Phases (0 through 12) Complete & Verified* |

*Note: Scorecard is updated upon the completion of each phase gate.*
