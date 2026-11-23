# Follow-up plan — closing the partial items

This plan closes the items that were reported as partial after the first full
integration pass, and adds a local notification/sound system.

## Stream A — correctness and polish

1. Cap open chat tabs when opening or creating a tab, not only on restore.
2. Fix the contrast gate to read the shipped focus token and include
   `color-mix()`/state roles instead of the retired magenta ring.
3. Convert `Icon.svelte` from a long `if/else` chain into a typed registry
   while keeping every existing icon name and the new icons.
4. Route all component transitions through the shared duration tokens and
   presets; remove hardcoded animation durations where practical.
5. Make hover-only actions (notably the tab close button) visible on touch.

## Stream B — UI for the partial features

1. Session picker: show saved/open chats with busy state and whether a thread
   is already open in a tab; keep the existing FTS search modal.
2. Settings UI for permission rules and tool hooks.
3. Multi-run fusion: let the user keep one answer or join labelled answers.
4. A notification/sound panel with local-only settings.
5. Fire a notification and a short sound when: a goal completes, a chat/working
   turn finishes while the window is unfocused, a tool approval is requested,
   or the model asks a question. All opt-in, with no network calls.

## Stream C — backend depth

1. Wire credential-pool rotation into real 401/429 responses.
2. Add a repair path to `doctor` (report what can be fixed; never delete user
   data without an explicit request).
3. Add MCP OAuth PKCE sign-in support alongside existing transports.
4. Make goal mode advance across turns with a bounded step loop and an
   independent check before marking a goal done.
5. Cache config writes so unchanged sections are not rewritten on every
   debounced settings change.

## Integration

- One worktree per stream, disjoint file ownership.
- No local builds on the weak VM; only `cargo fmt`, Node unit tests and static
  checks.
- Verify each child commit with a line-count guard before cherry-picking.
- Wire new Rust modules/commands in one integration commit.
- Push once; collect all CI failures from a single run before fixing.
