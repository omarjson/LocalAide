# ADR-0005: Cryptographic Verification of App Updates and Model Catalog

> **2026-11-28:** The Rust/Tauri backend this ADR described was replaced by the Node ESM backend under `electron/`. The decision carries over — the contracts and invariants are the same, implemented in Node.


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
3. **Application updates (wired in 0.1.2)**:
   - `tauri-plugin-updater` reads `latest.json` from the newest published GitHub Release. Packages are signed in CI with the owner's update key and verified against the public key built into the app before install.
   - Updates need two presses (check, then "Install and restart"), show release notes, and never run in the background or at startup (`src-tauri/src/commands/updates.rs`).
   - Builds without a public key fall back to linking the release page. Creating the key pair is an owner step (`docs/release.md`). Ed25519 for the catalog stays open work.

## Consequences
- **Positive**: Corrupted downloads and untrusted model weights are rejected before activation.
- **Positive**: Low memory overhead during multi-gigabyte downloads and verification.
- **Positive**: Resilient downloads on flaky connections.
- **Honesty note**: the digest proves internal consistency of the catalog, not its publisher; do not describe it as "signed" in product docs.
