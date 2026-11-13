# Security Policy & Sandbox Guarantees

LocalAide's security model is local-first and least-privilege: with cloud mode off, chat runs in a local sidecar bound to loopback, tools are jailed to one directory, and nothing phones home.

## 1. Local Inference Boundary

- Chat runs in a `llama-server` child process listening on `127.0.0.1` only, on a free port, with a random 256-bit API key required on every request ([ADR-0007](docs/adr/0007-sidecar-engine.md)).
- The loopback server log is written to `~/.aiassistant/logs/llama-server.log` on your machine.

## 2. Sandbox Guarantees

1. **Capability-rooted jail** — file tools are confined to `~/.aiassistant/workspace/`. Every path is validated lexically (traversal depth, `..` escapes) and canonically (`std::fs::canonicalize`); anything resolving outside the root is rejected with `AppError::sandbox_denied`. There is no command-execution tool: the only actions are `list_dir`, `read_file`, `search_files`, and `write_file`.
2. **Symlink boundary** — symlinks resolving outside the jail are rejected; symlink loops are detected and blocked.
3. **Explicit approval for writes** — `write_file` requires interactive approval in the frontend modal ("Allow Once", "Allow for Session", or "Deny") with the tool name, action, path, and payload preview. Reads, listings, and searches are auto-approved.
4. **Audit log** — every tool call (approved, denied, or failed) is recorded in the SQLite `tool_audit` table with timestamp, action, path, and outcome. The log is user-visible and clearable from Settings; it is not append-only.

## 3. Content Security Policy (CSP)

Tauri enforces the CSP defined in `src-tauri/tauri.conf.json`:

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost
```

Remote scripts and arbitrary inline script execution are blocked; frontend network access is limited to the app's own IPC channels.

## 4. Supply Chain & Model Integrity

- **Catalog integrity digest**: the bundled catalog carries a SHA-256 digest over every entry's id, size, and hash, recomputed before presentation. A mismatch aborts before any download starts. This detects a tampered or corrupted list; it is not a publisher signature, and Ed25519 signing remains future work ([ADR-0005](docs/adr/0005-updates.md)).
- **Streaming verification**: downloads are written to `.part` files and the SHA-256 digest is computed incrementally in 64 KB buffers.
- **Atomic rename**: the destination `.gguf` file is created only after the digest matches the catalog entry. A failed verification deletes the corrupted file.

## 5. Online Providers (Opt-In)

- Cloud mode is off by default and never activates itself. Chat runs locally until you add a provider in Settings and switch it on.
- The provider's API key is stored in the local config file on this machine only. The backend sends it solely as a bearer token to the base URL you configured — to no other endpoint — and never returns it to the frontend (only a masked hint of the last four characters).
- Base URLs must use HTTPS. Plain HTTP is accepted only for loopback addresses (`127.0.0.1`, `localhost`), where local proxies such as Ollama or LM Studio run.
- While cloud mode is active, the chat header shows a cloud badge, so it is always visible that messages are leaving the machine. Switching cloud mode off, or loading a local model, returns inference to the local sidecar.

## 6. Reporting a Vulnerability

Report security vulnerabilities privately:

- Email: `security@localaide.dev`
- Or open a private GitHub Security Advisory at `https://github.com/omarjson/LocalAide/security/advisories/new`

Do not open public GitHub issues for security vulnerabilities.
