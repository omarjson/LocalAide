# ADR-0010: Community Models from the Hugging Face Hub

Date: 2026-11-14. Status: accepted.

## Context

The app's model story was curated-only: 41 entries pinned by SHA-256 in the
built-in catalog, verified against the hub by a CI job. The operator asked for
a dynamic selector so people can use **whatever** GGUF model they like, not
just the curated set.

Downloading arbitrary weights is a supply-chain decision, so this ADR records
exactly what the app does and does not guarantee.

## Decision

1. **Two layers.** The curated catalog stays the default view and the only
   thing presets use. Alongside it, a community registry
   (`~/.aiassistant/custom-models.json`) holds entries the user added from
   the hub. Community entries reuse the `CatalogEntry` shape, so downloads,
   SHA-256 verification, `.part` resume, companion handling, deletion, and
   loading run through the exact same code paths as curated models.
2. **What is verified.** Every download — curated or community — is checked
   against the SHA-256 of the LFS object Hugging Face serves for that file
   (`lfs.oid` from the hub's tree API). This proves the bytes that arrived
   are the bytes the hub publishes: no corruption, no on-path tampering.
3. **What is NOT verified.** Weights provenance and license compliance.
   Hugging Face hosting is not an endorsement; a community repo can contain
   malicious or mislicensed content. The UI states this in plain language
   beside the search box, shows the repo's license tag, and badges community
   models in every list. The choice to run a community model is the user's.
4. **No credentials.** Gated repositories require a Hugging Face login; the
   app does not store hub credentials and reports `HF_GATED` with a clear
   message instead of working around it.
5. **No automatic downloads.** Community entries are registered with
   metadata only; bytes are fetched only when the user presses download.
6. **Bounded surface.** Repository ids are validated against a strict
   alphabet (`org/name`, no traversal), API responses are capped at 2 MB,
   the registry holds at most 50 entries, and split GGUFs may not exceed the
   existing 8-companion limit.
7. **Switching lives in two places:** the Models screen (browse, add,
   download) and a chat-header picker over installed models. Loading a model
   resolves through the filesystem (catalog entry or community entry), which
   also fixes loading freshly downloaded and split-shard models.

## Alternatives rejected

- **Vetting queue for community models** — a manual allowlist reintroduces
  the curated bottleneck the feature exists to remove.
- **Storing HF tokens to unlock gated repos** — expands the credential
  surface against the app's local-first security posture (ADR-0009 keeps
  even provider keys opt-in to backups).
- **Client-side model file parsing before download** — heavyweight; the
  SHA-256 pin plus user discretion covers the realistic threats here.

## Consequences

- Anyone can run any public GGUF, including quantizations of unknown
  quality. The RAM-estimate warning and the license tag are the guardrails.
- The curated catalog keeps its integrity digest and CI pin verification;
  community entries are pinned only to what the hub currently serves, so a
  repo owner pushing new bytes changes the expected hash — a re-download
  re-verifies against the new hash, which is the honest behavior for an
  unpinned source.
