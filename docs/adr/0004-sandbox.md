# ADR-0004: Workspace Jail Sandbox and Tool Execution Security

## Status
Accepted, amended 2026-11-13 when the jail shipped: capability handles were replaced by canonicalization plus lexical checks, and the audit log is clearable, not append-only.

## Context
The legacy tool execution implementation in `src-tauri/src/tools.rs` used a naive denylist:
- It checked whether path strings contained `..` or matched specific forbidden paths (`/etc`, `/Windows`, `~/.ssh`).
- It was vulnerable to symlink traversal, directory aliasing, non-canonical path resolution, and absolute path escapes.
- It executed tool calls without an interactive confirmation flow.

## Decision
1. **Capability-rooted filesystem jail (`WorkspaceJail`)**:
   - Tool operations (read, write, list, search) are confined to `~/.aiassistant/workspace/` — a fixed root under the app directory, not a user-chosen one.
   - Shipped mechanics: lexical traversal checks (component depth for `..`) plus `std::fs::canonicalize` on every existing target and the nearest existing ancestor of new files. An early `cap-std`/openat handle design was replaced by this simpler canonicalization approach before it shipped.
   - Reject any symlink that resolves outside the root, and any absolute path that resolves outside it.
2. **Explicit user approval flow**:
   - Mutating operations (`write_file`) require interactive frontend approval.
   - The modal shows the tool name, operation, target path, and payload preview, with three options: **Allow Once**, **Allow for Session**, **Deny**.
   - Reads, listings, and searches are auto-approved and bounded (1 MB reads, 1000-entry listings, 4 MB writes).
3. **Audit logging**:
   - Every tool call — approved, denied, or failed — is written to the SQLite `tool_audit` table with timestamp, tool name, action, path, approval state, and status. The log is user-visible and clearable from Settings; it is not append-only.
4. **Hardened Tauri security policy**:
   - Strict CSP (`default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost`).
   - No wildcard permissions and no shell plugin access in Tauri capabilities.

## Consequences
- **Positive**: Defense-in-depth against prompt-injection attempts to read or write outside the jail.
- **Positive**: Complies with the principle of least privilege; no command-execution tool exists to abuse.
- **Negative**: The user must confirm file writes, and canonicalization rejects otherwise valid paths that cross the jail boundary through links.
