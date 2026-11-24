# Requirements: 0.2.4 polish (setup, approvals, splits)

> **How this spec was made.** Written on 2026-11-24 in a Kiro Web (Vibe) session, *before* any code for the batch, from the operator's 0.2.4 brief. It follows the shape of `.kiro/specs/online-providers/`: EARS criteria here, the design in `design.md`, and a task list in `tasks.md` that maps each task to the criteria and, once shipped, to commits. The Stitch MCP server is not available in Kiro Web, so the Stitch `taste-design` checklist (google-labs-code/stitch-skills) was applied by hand; see `design.md` §Setup wizard.

## Introduction

LocalAide 0.2.3 works, but first-run setup reads like a settings page (RAM figures, three branches up front), approvals do not say which permission mode the agent runs in, and nothing happens on its own when the user steps away from a plan or a terminal request. The two biggest Svelte screens are also still hard to change safely. This batch fixes those, without adding other features and without touching the in-app updater.

## Glossary

- **Permission mode:** the chat box's permission icon (`ApprovalMode`): Plan mode (`plan`), Ask before changes (`ask`), Autonomous mode (`smart`), Full access (`auto`). Not the answering style (`ChatModeId`, also in `src/lib/chatModes.ts`).
- **Plan card:** the "The plan is ready" card above the chat box after a Plan-mode answer, with Keep planning and Build.
- **Terminal approval:** the inline "Run this command?" prompt on a terminal tool call (`ToolCallCard`, group `shell`).
- **File-change dialog:** `ToolApprovalModal`, the modal for the manual file tool.
- **Setup:** `src/screens/SetupWizard.svelte`, shown on first run.
- **Setup calls:** `api.systemProbe`, `api.getModelPresets`, `api.listModels`, `api.downloadModelPreset`, `api.cancelPresetDownload`, `api.hfSearch`, `api.hfRepoFiles`, `api.hfCustomAdd`, `api.downloadModel`, `api.activateModel`, `api.cancelModelDownload`, `api.setTheme`, and the engine download and self-test that run inside the backend when a model starts.

## Requirement 1: setup that records well

**User story:** As a new user (and as the owner recording the demo), I want first-run setup to be friendly, short and obvious, so that I get from install to a first chat with one clear choice.

1. WHEN setup opens, THE SYSTEM SHALL show the patched-ghost mascot leading every step, with a short line in plain words saying what is happening.
2. WHEN the computer check finishes and model sizes load, THE SYSTEM SHALL offer exactly one recommended path, the Tiny model, with one primary button to install it.
3. IF the Tiny size is missing from the backend list, THEN THE SYSTEM SHALL fall back to the size it recommended before this change (the largest that fits).
4. THE SYSTEM SHALL describe how well a size fits in plain words ("Fits easily", "Fits", "Too big for this computer") AND SHALL NOT show RAM figures in the default view.
5. THE SYSTEM SHALL place other sizes, online providers and community models behind a single "More options" disclosure, closed by default.
6. WHERE memory is too tight for every size, THE SYSTEM SHALL say so in plain words AND SHALL open "More options" so the online choice is visible.
7. WHILE a model downloads, THE SYSTEM SHALL show the mascot at work, an accessible progress bar with the percentage, and Cancel.
8. WHEN the download finishes, THE SYSTEM SHALL show a done moment (mascot celebrates once), the light/dark choice, and one "Start chatting" button.
9. THE SYSTEM SHALL keep "Set up later" reachable on the choose step.
10. THE SYSTEM SHALL keep both themes readable (contrast tokens unchanged) and the ember accent as the only accent.
11. WHERE the user prefers reduced motion, THE SYSTEM SHALL not animate the mascot.

**Hard constraint (R1.H):** the setup calls listed in the glossary, their arguments, their order, and the step state machine (`detect → preset → confirm/online → download → done`) SHALL stay the same. Only markup, copy, styles and which preset is highlighted change.

## Requirement 2: approvals show the permission mode

**User story:** As a user, I want every approval to say which permission mode the agent is in, so that I know how much it can do without me.

1. WHEN the agent asks for approval of a tool call, THE SYSTEM SHALL show the permission mode the answer was sent in (for example "Autonomous mode" or "Plan mode") on the approval prompt.
2. WHEN the file-change dialog opens, THE SYSTEM SHALL show the current permission mode in it.
3. WHEN the plan card shows, THE SYSTEM SHALL label it with "Plan mode" and name the mode Build switches to.
4. THE SYSTEM SHALL keep every existing approve and deny action, label and backend call unchanged.
5. THE mode labels SHALL come from one list shared with the permission menu, so the two never disagree.

## Requirement 3: auto-accept timer for plans

**User story:** As a user who starts a plan and walks away, I want the option to have the plan accepted after a while, so that work continues without me, but only if I turn it on.

1. THE SYSTEM SHALL offer a "Plan auto-accept" setting with Off, 1 minute and 5 minutes, saved in `config.json`.
2. WHEN the config has no value, THE SYSTEM SHALL use Off.
3. WHILE the setting is Off, THE SYSTEM SHALL wait for Build or Keep planning, as today.
4. WHILE the setting is on AND the plan card shows, THE SYSTEM SHALL show a visible countdown ("Builds in 0:59") on the card.
5. WHEN the countdown reaches zero, THE SYSTEM SHALL do exactly what Build does.
6. WHEN the user presses Keep planning or Build, or a new message is sent, THE SYSTEM SHALL stop the countdown.
7. WHILE the user is typing in the chat box, or the chat is not on screen, THE SYSTEM SHALL pause the countdown and say it is paused.

## Requirement 4: terminal approval countdown

**User story:** As a user, I want a terminal request to run on its own after a set time unless I decline it, and to always see how long I have.

1. THE SYSTEM SHALL offer a "Terminal commands run after" setting with Wait for me, 30 seconds, 1 minute, 2 minutes and 5 minutes, saved in `config.json`.
2. WHEN the config has no value, THE SYSTEM SHALL use 1 minute (60 seconds).
3. WHILE a terminal approval waits AND the setting is not Wait for me, THE SYSTEM SHALL show a countdown that says the command runs when it ends ("Runs in 0:42 unless you choose Don't run").
4. WHEN the countdown reaches zero, THE SYSTEM SHALL answer the approval with Run (`allow`), through the same call the Run button uses.
5. WHEN the user chooses any option first, THE SYSTEM SHALL stop the countdown and apply that choice.
6. WHILE the setting is Wait for me, THE SYSTEM SHALL wait for a choice as today ("Nothing happens until you choose").
7. WHILE the chat is not on screen, THE SYSTEM SHALL pause the countdown, so a command never runs where the user cannot see it counting.
8. THE countdown SHALL apply only to terminal approvals; other tool approvals keep waiting for a choice.
9. Blocked commands SHALL still never reach an approval, in any mode (unchanged backend rule).

## Requirement 5: mode switch in Settings

**User story:** As a user, I want to switch between Plan mode and Autonomous mode from Settings too, so that I can set how the agent works without opening a chat.

1. THE SYSTEM SHALL show an "Agent mode" control in Settings with Plan mode and Autonomous mode.
2. WHEN the user picks one, THE SYSTEM SHALL save it as the permission mode every chat uses (the same saved state the chat box's permission icon uses) AND SHALL update open chats at once.
3. WHEN the saved mode is neither Plan nor Autonomous, THE SYSTEM SHALL name the current mode and leave both options unselected.
4. THE state and its helpers SHALL live in `src/lib/chatModes.ts`.

## Requirement 6: smaller files, same behaviour

**User story:** As the maintainer, I want the biggest files split into focused modules, so that changes stay small and reviewable.

1. THE SYSTEM SHALL move sections of `ChatScreen.svelte` (the chat settings sheet, the in-chat search bar, and the composer's plan card) into components, with no behaviour change.
2. IF time allows, THE SYSTEM SHALL do the same for `SettingsScreen.svelte`, `services/agent/tools.rs` and `config.rs`.
3. WHEN code moves, THE tests that read it SHALL move with it, and every existing test SHALL pass.
4. THE log SHALL describe splitting as a maintainability change and SHALL NOT claim it makes the app faster (the runtime win came from message virtualization in 0.2.3).

## Requirement 7: verify and ship to dev only

1. THE batch SHALL pass `npm run check`, `npm run test:unit`, `npm run build` and `npm run smoke`, plus `cargo check --all-targets` for the Rust changes.
2. THE batch SHALL be pushed to `dev` only: no commit to `main`, no tag, no release.
3. THE session SHALL be logged in `.kiro/specs/kiroween-2026/kiro-log.md` with commit hashes.

## Non-functional requirements

- **No new features** beyond R1 to R6. The in-app updater is out of scope and is not tested here.
- **Typed boundary:** the new settings commands take and return a concrete DTO with `Result<T, AppError>` (ADR-0001).
- **Heat:** no infinite animation on SVG children (the 0.2.2 heat fix). Mascot motion moves the HTML wrapper with `transform`/`opacity` only, and only while a download runs.
- **Safety default:** the plan timer defaults to Off. The terminal timer defaults to 60 seconds because the operator asked for it; this is recorded as a deliberate trade-off in `design.md`.
