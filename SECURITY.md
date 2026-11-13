# Security Policy & Sandbox Guarantees

LocalAide is designed with a defense-in-depth security model to ensure safe local AI execution without exfiltration, remote code execution, or unauthorized filesystem access.

---

## 1. Sandbox Guarantees

1. **Capability-Rooted Jail**:
   - Filesystem tool execution is strictly confined to the user workspace directory (`~/.aiassistant/workspace/`).
   - Every file path is validated both lexically (rejecting `..` pop attempts) and canonically (resolving targets via `std::fs::canonicalize`).
   - Any path resolving outside the workspace root is rejected with `AppError::sandbox_denied`.

2. **Symlink Boundary Policy**:
   - Symlinks pointing outside the workspace jail are rejected.
   - Cyclical symlinks (loops) are detected and blocked.

3. **User Confirmation for Mutations**:
   - Destructive or mutating operations (`write_file`) require interactive approval in the frontend modal.
   - Non-destructive reads (`read_file`, `list_dir`, `search_files`) within the jail are logged to SQLite audit logs.

4. **Audit Log**:
   - Every tool call, whether approved, denied, or errored, is written to the SQLite `tool_audit` table with a timestamp, action, target path, and outcome. The log is user-visible and can be cleared from Settings.

---

## 2. Content Security Policy (CSP)

Tauri enforces a strict Content Security Policy defined in `tauri.conf.json`:
```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost
```
- Remote scripts and inline arbitrary script execution are strictly prohibited.
- Network requests are blocked except for explicit localhost IPC channels.

---

## 3. Supply Chain & Model Integrity

- **Catalog Integrity Digest**: The bundled catalog carries a digest over every entry's id, size, and hash, verified before presentation. A mismatched catalog aborts before any download starts.
- **Streaming Verification**: Downloads are written to `.part` files; SHA-256 digests are computed incrementally during download.
- **Atomic Rename**: Files are renamed to `.gguf` only after the SHA-256 digest matches the catalog entry. Corrupted downloads are purged immediately.

---

## 4. Online Providers (Opt-In)

- Cloud mode is **off by default** and never activates itself. Chat runs locally until the user adds a provider in Settings and switches it on.
- The provider's API key is stored in the app config file on this machine only. The backend sends it solely as a bearer token to the base URL the user configured — never to any other endpoint, and it is never returned to the frontend (only a masked hint of the last four characters).
- Base URLs must use HTTPS. Plain HTTP is accepted only for loopback addresses (`127.0.0.1`, `localhost`), where localhost proxies such as Ollama or LM Studio run.
- While cloud mode is active, the chat header shows a cloud badge so it is always visible that messages are leaving the machine. Switching cloud mode off, or loading a local model, returns all inference to the local sidecar.

---

## 5. Reporting a Vulnerability

If you discover a security vulnerability in LocalAide, please report it privately:
- Email: `security@localaide.dev`
- Or open a **Private GitHub Security Advisory** at `https://github.com/omarjson/LocalAide/security/advisories/new`

Do NOT open public GitHub issues for security vulnerabilities. We respond within 48 hours and coordinate responsible disclosure.
