# ADR-0009: Encrypted local backups

Date: 2026-11-14
Status: Accepted

## Context

The app's user state — config, memory notes, conversations — lives only under
`~/.aiassistant/`. The operator asked for cloud backup (OneDrive / Google
Drive). Both providers require an OAuth client registered by the operator and
recommend the authorization-code flow with PKCE for desktop apps; neither
ships a workable device-code flow for their storage scopes.

## Decision

1. **The backup engine is local-first.** A backup is one encrypted archive
   (`.labak`): AES-256-GCM over a JSON payload (config, memory notes,
   conversations), with a fresh 16-byte salt and 12-byte nonce per file and
   the archive header bytes bound as AEAD associated data. The key comes
   from argon2id over a user passphrase — the passphrase is never stored,
   never leaves the process, and there is no recovery without it.
2. **The default target is a local folder** (default `~/.aiassistant/backups`,
   user-choosable). The user can point it at a OneDrive or Google Drive
   desktop-sync folder and get cloud redundancy with **zero OAuth code and
   zero cloud credentials in the app** — the sync client moves the file.
3. **API keys are excluded from archives by default.** Provider keys are
   only embedded when the user explicitly opts in for that backup.
4. **Restore verifies before it touches anything**: the passphrase is checked
   against the archive first, memories and conversations always restore, and
   a config restore is opt-in and refuses to overwrite a config newer than
   the archive.
5. **Backups are user-triggered, not scheduled.** A schedule would require
   storing key material (or unencrypted archives) at rest, which contradicts
   the trust model; the passphrase exists only in memory while the user is
   creating or restoring a backup.
6. **Cloud provider targets (OneDrive via Graph, Drive via drive.appdata)**
   are deferred until the operator registers the OAuth clients. They will
   slot behind the same engine as additional targets; the auth flow is
   authorization-code + PKCE with a loopback redirect handled locally.

## Consequences

- No cron-style backups; the UI shows the last backup time instead.
- Losing the passphrase loses the backup — stated plainly in the UI copy.
- The archive format is self-describing (`LOCALAIDE-BACKUP-v1`) so a future
  version can migrate older archives.
- Cloud targets are additive: when registered, the network boundary in
  `docs/scope.md` gains the two upload endpoints as explicit user opt-in.
