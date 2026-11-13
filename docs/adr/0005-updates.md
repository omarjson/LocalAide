# ADR-0005: Cryptographic Verification of App Updates and Model Catalog

## Status
Accepted for the model pipeline and shipped. The application updater is designed but not wired: no update check exists in the current build, and Ed25519 signing remains future work.

## Context
The legacy application claimed SHA-256 verification of models, but:
1. `model_verify` checked against `entry.sha256`, which was `None` for all catalog entries, guaranteeing failure or a no-op.
2. Verification read entire multi-gigabyte models into RAM, risking out-of-memory crashes on 8–16 GB systems.
3. Nothing checked the catalog itself, and there was no update mechanism.

## Decision
1. **Catalog integrity digest**:
   - The built-in catalog carries a SHA-256 digest computed over every entry's id, file hash, and size (including companion files). `CatalogManifest::verify` recomputes it, so a mismatched or tampered catalog is rejected before any download begins.
   - The code documents this digest explicitly as an integrity check, not a publisher signature: it detects accidental or malicious edits to the built-in list, but it does not prove who published it. Ed25519 catalog signing remains future work.
   - Every entry specifies an exact SHA-256 checksum, file size in bytes, and license label.
2. **Streaming SHA-256 verification with resume**:
   - Model downloads stream to `.part` files.
   - The SHA-256 hash is computed incrementally in 64 KB buffers during download, with no secondary full-file read and no RAM buffering.
   - Interrupted downloads keep their `.part` file; HTTP `Range` requests resume from the byte boundary.
   - Only when the finalized digest matches the catalog entry is the file atomically renamed to `model.gguf`; a failed check deletes the file and returns `CHECKSUM_MISMATCH`.
3. **Application updates (design, not yet wired)**:
   - Official binary releases will be signed with Tauri's updater signing keys and distributed via GitHub Releases with signed manifests.
   - Updates will require explicit user confirmation, with release notes; no automatic background installation.
   - Wiring this up — including the signing keys and the update check — is open work, as is Ed25519 for the catalog.

## Consequences
- **Positive**: Corrupted downloads and untrusted model weights are rejected before activation.
- **Positive**: Low memory overhead during multi-gigabyte downloads and verification.
- **Positive**: Resilient downloads on flaky connections.
- **Honesty note**: the digest proves internal consistency of the catalog, not its publisher; do not describe it as "signed" in product docs.
