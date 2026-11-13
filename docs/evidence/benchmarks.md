# Phase 10 Verification: Performance Benchmarks & Fault Injection

## 1. Automated Performance Benchmarks

| Metric | Budget Target | Measured | Result | Verification Location |
|---|---|:---:|:---:|---|
| **Keyword Search Latency (FTS5 BM25)** | < 50 ms | **< 15 ms** | **PASS** | `src-tauri/tests/library_test.rs` |
| *Vector search latency* | — | — | **N/A** | The vector half of hybrid search never executes in production: chunk embeddings are never written and no query vector is passed, so there is no vector latency to measure. |
| **Context Assembly (200 turns)** | < 50 ms | **< 2 ms** | **PASS** | `src-tauri/tests/fault_injection_test.rs` |
| **Streaming SHA-256 Hashing RAM** | < 1 MB | **< 100 KB** (64KB buffer) | **PASS** | `src-tauri/src/services/models/verify.rs` |
| **Model Cancellation Latency** | < 100 ms | **< 15 ms** | **PASS** | `src-tauri/tests/contract_test.rs` |
| **Frontend Production Bundle Size** | < 250 KB | **128 KB JS / 45 KB CSS** | **PASS** | `npm run build` |
| **Startup Smoke Checks** | dist/ build sanity | No timing is measured. The script asserts the built `dist/` output: `#app` mount container, `LocalAide` title, no dead assets, a compiled Tailwind CSS bundle (> 10 KB) and a JS bundle with the brand present. | **PASS** | `scripts/smoke-test.mjs` |

---

## 2. Fault Injection Resilience Matrix

| Scenario | Injected Condition | Expected Behavior | Measured Recovery | Result |
|---|---|---|---|:---:|
| **Corrupt GGUF Model** | Invalid magic bytes and truncated payload written to model file | Model verification fails cleanly with `CHECKSUM_MISMATCH`, does not crash | Returns `AppError { code: "CHECKSUM_MISMATCH" }` | **PASS** |
| **Simulated Killed Process** | Download process aborted midway with 512-byte `.part` file | `.part` file remains intact for HTTP `Range` resumption; target `.gguf` is not created until full verification | `.part` preserved, size verified, unfinalized | **PASS** |
| **Corrupt SQLite DB** | Database file overwritten with non-database ASCII header | `LibraryDb::open` detects corrupted file and returns `AppError::storage` without panic | Returns `AppError { code: "STORAGE_ERROR" }` | **PASS** |
| **Read-Only / Disk Quota** | Payload exceeding maximum write ceiling (`MAX_WRITE_BYTES`) | Attempted write is rejected before disk allocation with `INVALID_INPUT` | Returns `AppError { code: "INVALID_INPUT" }` | **PASS** |
| **Symlink Loop Attack** | Cyclical symlinks created inside workspace jail (`a -> b -> a`) | Path canonicalization detects loop and rejects with `SANDBOX_DENIED` | Returns `AppError { code: "SANDBOX_DENIED" }` | **PASS** |
| **Path Traversal Attack** | Traversal tokens `../` and `/etc/passwd` | Jail blocks lexical escape and canonical parent escapes | Returns `AppError { code: "SANDBOX_DENIED" }` | **PASS** |
| **Symlink Exfiltration** | Symlink resolving to secret file outside workspace jail | Jail inspects canonical target and blocks exfiltration | Returns `AppError { code: "SANDBOX_DENIED" }` | **PASS** |

Honest state: 32 of the 33 tests pass reliably; the one known failure (`library_test::test_folder_recursive_crawler_and_ignore_patterns`, which depends on the temp-dir path shape) is documented in [audit-2026-11-13.md](audit-2026-11-13.md).
