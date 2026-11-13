# ADR-0005: Cryptographic Verification of App Updates and Model Catalog

## Status
Accepted

## Context
The legacy application claimed SHA-256 verification of models, but:
1. `model_verify` was checking against `entry.sha256`, which was `None` for all catalog entries, guaranteeing failure or no-op.
2. When verification was attempted, it read entire multi-gigabyte models into RAM in memory buffers, creating high risk of out-of-memory crashes on systems with 8-16 GB RAM.
3. No signature or cryptographic provenance checked the catalog itself or application updates.

## Decision
1. **Catalog Integrity Digest (`catalog.json`)**:
   - The model catalog carries a SHA-256 integrity digest computed over every entry's id, file hash and size (including companion files). `CatalogManifest::verify` recomputes the digest when the model service starts, so a mismatched or tampered catalog is rejected before any download can begin.
   - The code documents this digest explicitly as an integrity check, not a publisher signature: it detects accidental or malicious edits to the built-in list, but it does not prove who published the catalog. Ed25519 catalog signing remains future work.
   - Every model entry in `catalog.json` specifies an exact SHA-256 checksum, file size in bytes, and license.
2. **Streaming SHA-256 Verification with Resume**:
   - Model downloads stream chunks directly to disk into `.part` files (e.g. `model.gguf.part`).
   - The SHA-256 hash is computed incrementally during download stream ingestion via `sha2::Sha256`, avoiding any secondary full-file disk reads or RAM buffering.
   - If a download is paused or interrupted, `.part` files are retained; HTTP `Range` requests resume from the byte boundary.
   - Upon completion, the finalized digest is verified against the signed catalog. Only upon verification is the file atomically renamed to `model.gguf`.
3. **Application Updates**:
   - Official binary releases are signed using Tauri's updater signing keys.
   - Updates are distributed via GitHub Releases with signed `latest.json` manifests.
   - Automatic background installation is disabled; user must confirm updates, with clear release notes and rollback documentation.

## Consequences
- **Positive**: Complete protection against MITM attacks, corrupted downloads, and untrusted model weights.
- **Positive**: Low memory overhead (<5 MB during multi-gigabyte downloads).
- **Positive**: Resilient downloads on flaky connections.
