# ADR-0004: Workspace Jail Sandbox and Tool Execution Security

## Status
Accepted

## Context
The legacy tool execution implementation in `src-tauri/src/tools.rs` relied on a naive denylist approach:
- It checked if path strings contained `..` or matched specific forbidden paths (`/etc`, `/Windows`, `~/.ssh`).
- It was vulnerable to symlink traversal, directory aliasing, non-canonical path resolution, and absolute path escapes.
- It lacked an interactive user confirmation flow with file diff preview, executing tool calls without approval.

## Decision
1. **Capability-Rooted Filesystem Jail (`cap-std` / openat)**:
   - Tool operations (reading files, writing files, directory listing) are strictly constrained to a user-configured Workspace Directory.
   - All filesystem operations are conducted using capability-based directory handles (`cap-std::fs::Dir` or equivalent canonicalization with strict path prefix checks).
   - Reject any path containing symlinks that resolve outside the designated workspace root.
   - Reject all absolute paths pointing outside the workspace.
2. **Explicit User Approval Flow**:
   - Destructive operations (file writes, deletes, modifications) require interactive frontend approval.
   - The approval modal must display:
     - The tool name
     - The target path relative to workspace
     - The full payload or unified diff for edits
     - Explicit options: "Approve Once", "Approve Always for this Workspace", or "Deny".
3. **Immutable Audit Logging**:
   - Every tool call, whether approved, rejected, or failed, is written to an append-only audit table in SQLite with a timestamp, caller context, sanitized inputs, and status.
4. **Hardened Tauri Security Policy**:
   - Set strict Content Security Policy (`default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:; font-src 'self'; connect-src 'self'`).
   - Remove wildcard permissions and shell plugin access from Tauri capabilities.

## Consequences
- **Positive**: Complete defense-in-depth against prompt-injection attacks attempting filesystem exfiltration or destruction.
- **Positive**: Complies with the principle of least privilege.
- **Negative**: User must confirm file modifications.
