---
inclusion: fileMatch
fileMatchPattern: ["catalog/**", "src-tauri/src/services/models/**", "scripts/gen-catalog.ps1", "scripts/verify-catalog.mjs"]
---

# Model catalog work

Loaded when you touch the catalog. The catalog is the list of models users can install, so a wrong pin means a failed download or an unverified file.

## Use the Hugging Face MCP server

`.kiro/settings/mcp.json` connects the official Hugging Face MCP server (`https://huggingface.co/mcp`). Use it instead of guessing:

- To find candidate models: search the hub for GGUF repos of the model family, sorted by downloads or recency. Prefer official or well-known quantizers.
- To check a repo before adding it: read its details for the license, whether it is gated, and the last update. Gated repos return 401 to anonymous downloads (audit-2026-11-13 item 2), so never add one.
- Record what you found (repo, file, size, license, date) in `docs/evidence/models-2026-11.md`, or a new dated file.

The MCP server gives metadata. The pins (SHA-256 and byte size) come from the repo tree API, which `scripts/verify-catalog.mjs` reads. Do not type a hash by hand.

## Rules

- Every entry needs a real SHA-256 and size from Hugging Face, a license, and an HTTPS URL.
- `catalog/models.json` must equal `default_manifest()` in `services/models/catalog.rs`. Regenerate it with `scripts/gen-catalog.ps1` after changing the Rust list, and bump the manifest version.
- Show a RAM warning for large models. Drop any model that has no working public download.
- The `rust-and-catalog-checks` hook runs the parity test and the pin check on save. CI runs the same checks in the `catalog-pins` job. Do not call a catalog change done until both pass.
