# Tasks: 0.2.4 polish

Requirement ids refer to `requirements.md`. Commits are filled in as tasks ship.

**Verified on 2026-11-24 (Kiro Web sandbox):** `npm run check` (0 errors), `npm run test:unit` (215/215, 12 new in `tests/unit/polish-0-2-4.test.ts`), `npm run build`, `npm run smoke`, and `cargo check`/`clippy -D warnings`/`fmt --check` on Rust 1.91.1 with stub pkg-config files. Not run here: `cargo test` (no WebKitGTK to link against) and the pinned Rust 1.100.0; CI runs both. In the preview harness (`preview.html`, fake backend) with Playwright: setup in both themes (choose, More options, download, done); the terminal countdown at 30 s ran the command by itself through `decideTool("allow")`; the plan countdown paused while typing and built the plan at zero; the Settings mode switch moved an open chat to Autonomous mode; the moved chat settings sheet and search bar behaved as before.

**Known limits:** a countdown restarts from full if its message row is scrolled out of the virtualized list and back (it never runs while unmounted, so this only gives the user more time). The terminal default of 60 s is the operator's choice; see design.md.

- [x] 1. Write this spec (requirements → design → tasks) before code
  - _Requirements: all_

- [x] 2. Shared permission-mode state in `src/lib/chatModes.ts`
  - Move `PERMISSION_MODES` out of `PermissionMenu.svelte`; add `permissionLabel`, `CHAT_OPTIONS_KEY`, `readSavedApprovalMode`, `shareApprovalMode`, `onSharedApprovalMode`.
  - ChatScreen subscribes on mount and uses the shared key.
  - _Requirements: R2.5, R5.2, R5.4_ · `390b053`; build fix `0cc22a7`

- [x] 3. Approval timers in config
  - `plan_auto_accept_secs` (default 0) and `terminal_approval_secs` (default 60) in `AppConfig`; `ApprovalTimers` DTO; `settings_approval_timers` / `settings_set_approval_timers` (clamped 0..=600); register in `lib.rs`; config unit test for the defaults.
  - Client, bindings and `approvalTimers.svelte.ts`; pure helpers in `countdown.ts`.
  - _Requirements: R3.1, R3.2, R4.1, R4.2_ · `390b053`

- [x] 4. `ApprovalCountdown.svelte`
  - Mount-to-start, pause (prop and hidden window), single expiry, polite announcements, `scaleX` bar.
  - _Requirements: R3.4 to R3.7, R4.3, R4.5, R4.7_ · `390b053`; spacing fix `cc946a1`

- [x] 5. Mode shown on approvals
  - Store `mode` with each pending approval; chip in `ToolCallCard`, `ToolApprovalModal` and the plan card.
  - _Requirements: R2.1 to R2.4_ · `390b053`

- [x] 6. Terminal countdown in `ToolCallCard`
  - Shell approvals only; expiry → `onDecide("allow")`; "Wait for me" keeps today's text.
  - _Requirements: R4.3 to R4.8_ · `390b053`

- [x] 7. Plan card split and auto-accept
  - Move the plan card to `PlanReadyCard.svelte`; countdown when the plan timer is on; expiry → `onBuildPlan`; paused while typing or off screen.
  - _Requirements: R3.3 to R3.7, R6.1_ · `390b053`

- [x] 8. Settings rows
  - `AgentApprovalSettings.svelte` in Settings → Tools and MCP: Agent mode, Plan auto-accept, Terminal commands run after; search keywords.
  - _Requirements: R3.1, R4.1, R5.1 to R5.3_ · `390b053`

- [x] 9. Setup wizard presentation
  - `GhostMascot.svelte`; rework the markup and copy of `SetupWizard.svelte`; Tiny as the recommended path; "More options" disclosure; plain-word fits; download and done moments; reduced motion.
  - Keep every setup call and the step machine unchanged (R1.H), with a source test that lists them.
  - Check both themes in the preview harness.
  - _Requirements: R1.1 to R1.11, R1.H_ · `cc946a1`

- [x] 10. Split ChatScreen further
  - `ChatSettingsSheet.svelte`, `ChatSearchBar.svelte`; move the test assertions that read the moved code.
  - _Requirements: R6.1, R6.3_ · `7cb8d4d`

- [x] 11. Split SettingsScreen / Rust files (if time allows)
  - _Requirements: R6.2, R6.3_ · `879ac28` (WebSearchSettings), `8c6b18a` (config/permissions.rs), `4e4e47e` (agent/tools/specs.rs)

- [x] 12. Verify and ship to dev
  - `npm run check`, `npm run test:unit`, `npm run build`, `npm run smoke`, `cargo check --all-targets`; push `dev` only; log the session in `kiro-log.md`.
  - _Requirements: R6.4, R7.1 to R7.3_
