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

## 4. Reporting a Vulnerability

If you discover a security vulnerability in LocalAide, please report it privately:
- Email: `security@localaide.dev`
- Or open a **Private GitHub Security Advisory** at `https://github.com/omarjson/LocalAide/security/advisories/new`

Do NOT open public GitHub issues for security vulnerabilities. We respond within 48 hours and coordinate responsible disclosure.
