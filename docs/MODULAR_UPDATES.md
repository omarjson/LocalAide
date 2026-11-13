# Modular Updates (Deferred Specification)

> [!NOTE]
> Per [ADR-0006](adr/0006-optional-features.md), modular in-app hot-swapping is deferred beyond the current release. This document preserves the requirements and gate criteria for when the feature is reintroduced.

## Overview

Today, application updates are whole-package releases. The updater itself is not wired yet; updater signing is future work tracked in [ADR-0005](adr/0005-updates.md).

## Re-introduction Gate Criteria

Before modular component replacement is exposed to users:
1. **Cryptographic Manifest Verification**: component packages must be signed with Ed25519 manifests.
2. **Atomic Swap & Rollback**: component replacement must be transactional with automated rollback on health-check failure.
3. **Automated Verification**: integration tests must cover hot-swap failure scenarios without corrupting the running application state.
