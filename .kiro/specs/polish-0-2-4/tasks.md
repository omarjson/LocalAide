# Tasks: 0.2.4 polish

Requirement ids refer to `requirements.md`. Commits are filled in as tasks ship.

- [x] 1. Write this spec (requirements → design → tasks) before code
  - _Requirements: all_

- [ ] 2. Shared permission-mode state in `src/lib/chatModes.ts`
  - Move `PERMISSION_MODES` out of `PermissionMenu.svelte`; add `permissionLabel`, `CHAT_OPTIONS_KEY`, `readSavedApprovalMode`, `shareApprovalMode`, `onSharedApprovalMode`.
  - ChatScreen subscribes on mount and uses the shared key.
  - _Requirements: R2.5, R5.2, R5.4_

- [ ] 3. Approval timers in config
  - `plan_auto_accept_secs` (default 0) and `terminal_approval_secs` (default 60) in `AppConfig`; `ApprovalTimers` DTO; `settings_approval_timers` / `settings_set_approval_timers` (clamped 0..=600); register in `lib.rs`; config unit test for the defaults.
  - Client, bindings and `approvalTimers.svelte.ts`; pure helpers in `countdown.ts`.
  - _Requirements: R3.1, R3.2, R4.1, R4.2_

- [ ] 4. `ApprovalCountdown.svelte`
  - Mount-to-start, pause (prop and hidden window), single expiry, polite announcements, `scaleX` bar.
  - _Requirements: R3.4 to R3.7, R4.3, R4.5, R4.7_

- [ ] 5. Mode shown on approvals
  - Store `mode` with each pending approval; chip in `ToolCallCard`, `ToolApprovalModal` and the plan card.
  - _Requirements: R2.1 to R2.4_

- [ ] 6. Terminal countdown in `ToolCallCard`
  - Shell approvals only; expiry → `onDecide("allow")`; "Wait for me" keeps today's text.
  - _Requirements: R4.3 to R4.8_

- [ ] 7. Plan card split and auto-accept
  - Move the plan card to `PlanReadyCard.svelte`; countdown when the plan timer is on; expiry → `onBuildPlan`; paused while typing or off screen.
  - _Requirements: R3.3 to R3.7, R6.1_

- [ ] 8. Settings rows
  - `AgentApprovalSettings.svelte` in Settings → Tools and MCP: Agent mode, Plan auto-accept, Terminal commands run after; search keywords.
  - _Requirements: R3.1, R4.1, R5.1 to R5.3_

- [ ] 9. Setup wizard presentation
  - `GhostMascot.svelte`; rework the markup and copy of `SetupWizard.svelte`; Tiny as the recommended path; "More options" disclosure; plain-word fits; download and done moments; reduced motion.
  - Keep every setup call and the step machine unchanged (R1.H), with a source test that lists them.
  - Check both themes in the preview harness.
  - _Requirements: R1.1 to R1.11, R1.H_

- [ ] 10. Split ChatScreen further
  - `ChatSettingsSheet.svelte`, `ChatSearchBar.svelte`; move the test assertions that read the moved code.
  - _Requirements: R6.1, R6.3_

- [ ] 11. Split SettingsScreen / Rust files (if time allows)
  - _Requirements: R6.2, R6.3_

- [ ] 12. Verify and ship to dev
  - `npm run check`, `npm run test:unit`, `npm run build`, `npm run smoke`, `cargo check --all-targets`; push `dev` only; log the session in `kiro-log.md`.
  - _Requirements: R6.4, R7.1 to R7.3_
