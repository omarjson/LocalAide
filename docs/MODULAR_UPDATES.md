# Modular Updates (Deferred Specification)

> [!NOTE]
> Per [ADR-0006](file:///Users/omar/Documents/new/docs/adr/0006-optional-features.md), modular in-app hot-swapping is deferred to post-v1.0. This document preserves the architectural requirements and gate criteria for when the feature is reintroduced.

## Overview
In the initial v1.0 release, application updates occur through whole-package releases signed with the official release key. 

## Re-introduction Gate Criteria
Before modular component replacement is exposed to users:
1. **Cryptographic Manifest Verification**: Component packages must be signed via Ed25519 manifests.
2. **Atomic Swap & Rollback**: Component replacement must be transactional with automated rollback on health check failure.
3. **Automated Verification**: Integration tests must verify hot-swap failure scenarios without corrupting the running application state.
