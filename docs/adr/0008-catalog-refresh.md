# ADR-0008: Model catalog refresh

Date: 2026-11-14 · Status: Accepted

## Context

The model catalog — the list of downloadable GGUF models with licenses,
sizes, SHA-256 pins and preset groupings — was a compile-time list only
(`CatalogManifest::default_manifest()`). Adding or updating a model required
rebuilding and redistributing the app, so a user's Models screen silently
aged with every release they skipped.

The list needed a way to move without a rebuild, which means accepting a
fetched manifest at runtime. That is a new network-trust surface in an app
whose posture is "local by default, explicit opt-in for anything remote."

## Decision

- **Source is pinned.** The manifest comes from exactly one URL compiled
  into the app: `https://raw.githubusercontent.com/omarjson/LocalAide/main/catalog/models.json`
  — this project's own repository, served over HTTPS by GitHub. There is no
  user-configurable catalog source, so no UI flow can point the app at a
  hostile feed.
- **Refresh is explicit.** Nothing fetches the catalog automatically at
  startup or on a timer. The only triggers are the "Refresh catalog" button
  in the Models screen and the `model_catalog_refresh` command.
- **Validation is fail-closed.** A fetched manifest is accepted only if it:
  1. is at most 512 KB, streamed with a hard byte cap;
  2. parses as the full schema (the `signature` field is required, so a
     truncated or hand-rolled JSON is rejected outright);
  3. passes the integrity self-digest (`CatalogManifest::verify`), which
     catches corruption and hand-edits that did not recompute the digest;
  4. passes structural validation (`CatalogManifest::validate`): known
     kinds, unique non-empty ids, 64-hex SHA-256 pins, non-zero sizes,
     plausible RAM estimates, HTTPS-only download URLs without embedded
     credentials, bounded entry and companion counts;
  5. is not older than the currently active catalog (`version >= active`),
     which makes rollback to a stale list impossible, including through the
     on-disk cache after an app update.
  On any failure the previously active catalog stays in place and the error
  surfaces to the user.
- **The model files are independently pinned.** Each entry's `sha256` and
  each companion's `sha256` are verified at download time (existing
  behavior), so even a compromised catalog source cannot swap model weights
  silently — it would need to also break SHA-256.
- **Persistence is atomic and resettable.** Accepted manifests are cached to
  `catalog-cache.json` next to `config.json` (temp file + rename), loaded at
  startup when still valid, and removable via "Reset" /
  `model_catalog_reset`, which restores the list compiled into the binary.
- **Single source of truth.** `catalog/models.json` in the repository is
  generated from `default_manifest()`; a CI parity test
  (`catalog_json_matches_embedded_manifest`) fails if the two drift apart,
  so built-in installs and refreshed installs always agree.

## Consequences

- Catalog updates ship at repo-push speed without a release; a user who
  never refreshes keeps the complete list they installed with.
- The repository becomes a de-facto distribution channel for model
  metadata — changes there are reviewed like code (parity test + PR review).
- Refreshed presets note: hardware presets remain compile-time; a preset
  referencing a model removed upstream fails at download time with a clear
  "not found in catalog" error rather than silently no-op-ing.
- Docs that described the catalog as compile-time-only were updated in the
  same pass.
