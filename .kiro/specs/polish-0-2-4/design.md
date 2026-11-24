# Design: 0.2.4 polish

See `requirements.md` for the criteria (R1.1 to R7.3) this design meets.

## Overview

```
SettingsScreen ─ tools tab ─▶ AgentApprovalSettings.svelte
                                 │ mode  ──▶ chatModes.ts shareApprovalMode() ─▶ localStorage "localaide-chat-options"
                                 │                                        └─▶ window event ─▶ every ChatScreen.setApprovalMode()
                                 │ timers ─▶ approvalTimers.svelte.ts ─invoke─▶ settings_set_approval_timers ─▶ config.json
                                 ▼
ChatScreen ─ tool_approval event: approvals[id] = { approval_id, preview, mode: sentInMode }
   ├─ MessageBubble ─▶ ToolCallCard (mode badge; shell ⇒ ApprovalCountdown ─ expire ⇒ onDecide("allow"))
   ├─ ChatComposer ─▶ PlanReadyCard (Plan mode badge; ApprovalCountdown ─ expire ⇒ onBuildPlan)
   └─ ToolApprovalModal (mode badge)
```

Nothing in the backend approval path changes: the countdown ends in the same `api.decideTool(approval_id, "allow")` the Run button sends, and the backend's own 15-minute deny timeout (`services/agent/approval.rs`) stays as the outer bound.

## Shared mode state (`src/lib/chatModes.ts`) — R2.5, R5

- `PERMISSION_MODES` moves here from `PermissionMenu.svelte` (id, label, hint, icon). `permissionLabel(mode)` returns the label; the legacy `"chat"` value maps to "Ask before changes", as the menu already did.
- `CHAT_OPTIONS_KEY = "localaide-chat-options"`, the key ChatScreen already writes its options to. The approval mode stays in that key, so there is one saved state, not two.
- `readSavedApprovalMode(storage)` parses the key and returns a valid mode or `null`.
- `shareApprovalMode(mode, storage, target)` writes `approvals` (and `buildMode` when the mode is not Plan, matching `setApprovalMode`) into the key, then dispatches `localaide:approval-mode` on `window`.
- `onSharedApprovalMode(cb)` subscribes and returns the unsubscribe. Each ChatScreen subscribes on mount and calls its existing `setApprovalMode`, so the per-tab `$effect` keeps writing the same key.
- The functions take `storage` and `target` arguments so Node unit tests can pass fakes.

## Timers in config — R3.1, R3.2, R4.1, R4.2

- `AppConfig` gains `plan_auto_accept_secs: u32` (`#[serde(default)]`, 0 = Off) and `terminal_approval_secs: u32` (`#[serde(default = "default_terminal_approval_secs")]` = 60; 0 = Wait for me). An old `config.json` without them loads with Off and 60.
- `domain/settings.rs`: `ApprovalTimers { plan_auto_accept_secs, terminal_approval_secs }`.
- `commands/settings.rs`: `settings_approval_timers` and `settings_set_approval_timers(timers)`, the `settings_compact_prompt` pattern (lock store, load, change, save). The setter clamps both values to 0..=600 seconds so a hand-edited request cannot set a day-long or negative timer; the UI offers only the listed choices.
- `src/lib/api/client.ts`: `getApprovalTimers()` / `setApprovalTimers()`; `bindings.ts`: `ApprovalTimers`.
- `src/lib/approvalTimers.svelte.ts`: one module-level `$state` (`{ plan, terminal, loaded }`), `loadApprovalTimers()` (once, falls back to the defaults on error) and `saveApprovalTimers()`. Settings writes it; ChatScreen reads it, so a change applies to the next prompt without a restart.
- Pure helpers (choices, `formatCountdown`) live in `src/lib/countdown.ts` so they are unit-tested in Node.

## `ApprovalCountdown.svelte` — R3.4 to R3.7, R4.3 to R4.7

- Props: `seconds`, `paused`, `pausedLabel`, `label` (snippet text before the time, e.g. "Runs in"), `suffix`, `onExpire`.
- Starts when mounted; parents mount it only while the prompt is visible, so "stop" is just unmounting (R3.6, R4.5).
- Counts with a 250 ms `setInterval` against `performance.now()`, subtracting paused time, so a throttled background timer cannot skip ahead. `onExpire` fires once.
- Visible text: "Runs in 0:42 unless you choose Don't run." A thin bar under it shrinks with `transform: scaleX()` (no layout work). Screen readers get the text through `aria-live="polite"` only at the start, at 10 seconds left, and when paused, not every tick.
- Pausing: the parent passes `paused` (chat not on screen, or the user is typing for the plan card); the component also pauses while `document.visibilityState === "hidden"` (R4.7).

## Approval prompts — R2.1 to R2.4, R4

- `ChatScreen` stores `mode: sentInMode` with each pending approval. `MessageBubble` passes it, plus `terminalCountdownSecs` and `active`, to `ToolCallCard`.
- `ToolCallCard` approval block: a small mode chip ("Autonomous mode", the menu icon) next to the question. For `isShell` with a countdown above 0, the "Nothing happens until you choose." line is replaced by the countdown; expiry calls `onDecide("allow")`. The three buttons and their decisions are unchanged.
- `ToolApprovalModal` gets an optional `mode` prop and shows the same chip under the title. Buttons and handlers are unchanged.
- `PlanReadyCard.svelte` (moved out of `ChatComposer`): "Plan mode" chip, the existing sentence naming the Build mode, Keep planning and Build, plus the countdown when the plan timer is on. Paused while `input.trim()` is non-empty or the chat tab is inactive.

## Settings — R3.1, R4.1, R5

`AgentApprovalSettings.svelte`, rendered in Settings → Tools and MCP, just before `PolicyPanel` (Permissions), in the existing `SettingsGroup`/`SettingsRow`/`Tabs` kit:

| Row | Control | Saved in |
|---|---|---|
| Agent mode | Tabs: Plan mode · Autonomous mode; the description names the current mode when it is Ask or Full access | `chatModes.ts` shared mode |
| Plan auto-accept | Tabs: Off · 1 min · 5 min | `config.json` `plan_auto_accept_secs` |
| Terminal commands run after | Tabs: Wait · 30 s · 1 min · 2 min · 5 min | `config.json` `terminal_approval_secs` |

The Tools section keywords gain "plan autonomous mode timer countdown approval auto-accept" so Settings search finds the rows.

**Trade-off recorded (R4.2):** a 60-second default means a terminal command the agent proposes will run if nobody answers. The operator chose it. The mitigations are: the countdown text says so in plain words, the timer pauses while the chat is off screen or the window is hidden, it applies to terminal requests only, Plan mode never reaches a terminal approval (changing tools are skipped there), blocked commands never reach an approval, and "Wait for me" is one click away in Settings.

## Setup wizard — R1

### Constraint (R1.H)

The `<script>` keeps every call, argument and state transition: `detect`, `install`, `installRecommended`, `select`, `cancelInstall`, `wfSearch`, `wfToggleRepo`, `wfInstall`, `cancelCommunity`, `onlineSaved`, `retrySetup`, `chooseTheme`. The only logic-adjacent edit is `recommended`: `presets.find(p => p.id === "tiny")`, falling back to the old largest-that-fits rule (R1.2, R1.3). `fittingPresets`, `tightMemory` and the step machine are untouched. New code is presentation-only: a `moreOpen` flag, a `fitWords(p)` helper and copy.

### Layout

- **Mascot:** `GhostMascot.svelte`, the LogoMark ghost without its tile, outlined so it reads on both themes, with `mood`: `hello`, `looking`, `working`, `done` (eyes change; `done` shows happy arcs and the ember patch). A speech bubble beside it carries each step's one line.
- **Choose:** left-aligned hero (mascot + "Hi, I'm your LocalAide. Let's get me set up."), then one recommended card: "Tiny", the plain-words fit ("Fits easily on this computer"), what it is good for, one ember button "Install Tiny". Below it, one `<details>` "More options" with Other sizes (fit in words), Use an online provider, and Community models. "Set up later" stays as a quiet link.
- **Checking:** mascot `looking` with a sweeping indeterminate bar (no circular spinner).
- **Download:** mascot `working` bobbing gently (wrapper `transform` only, `prefers-reduced-motion` turns it off), the shared `ProgressBar` with percent in tabular numbers, the existing status line, Cancel.
- **Done:** mascot `done` with a one-shot pop and an ember ring that expands once, "All set!", the status line, theme choice, "Start chatting".
- **Steps:** three small labelled dots (Choose · Download · Say hi) instead of the numbered bar, since the content is a sequence.

### Stitch `taste-design` checklist, applied by hand

The Stitch MCP server cannot run in Kiro Web, so the checklist from `plugins/stitch-utilities/skills/taste-design/SKILL.md` was read and applied manually against `DESIGN.md` (which already follows the DESIGN.md format that skill produces):

| Checklist item | Applied |
|---|---|
| One accent, no neon or purple glow | Ember only; the done ring is a soft ember stroke, not a glow shadow |
| No pure black | Ghost ink is `#1f1630` (existing logo ink) |
| Hierarchy by weight, not size | Headline is 20px semibold; body 14px |
| One primary CTA | "Install Tiny" is the only filled button on the choose step |
| No circular spinners | Checking uses a sweeping bar |
| No emoji, no AI copy clichés, no invented numbers | Plain copy; no figures the backend did not return |
| No centered hero (variance > 4) | Hero is left-aligned: mascot left, words right |
| Touch targets ≥ 44px | Primary buttons are 44px tall |
| Motion via transform/opacity only | Yes |
| **Not applied:** perpetual micro-loops, `Geist`/`Satoshi` fonts | The repo's heat fix forbids infinite animation on idle screens (95 °C on the operator's Mac), and DESIGN.md keeps the system font so the app ships no font files and stays offline. Motion runs only during the download. |

## Splits — R6

| From | To | Notes |
|---|---|---|
| `ChatScreen.svelte` chat settings sheet | `components/ChatSettingsSheet.svelte` | Bindable toggles; the `Slider min={50} max={95}` test assertion moves with it |
| `ChatScreen.svelte` in-chat search bar markup | `components/ChatSearchBar.svelte` | The search logic stays in ChatScreen (it drives the virtualizer) |
| `ChatComposer.svelte` plan card | `components/PlanReadyCard.svelte` | Also where R3's countdown lives |
| `SettingsScreen.svelte` web-search tab | `components/WebSearchSettings.svelte` | If time allows |
| `services/agent/tools.rs`, `config.rs` | submodules | If time allows; `cargo check` only here (no WebKitGTK to link tests), CI runs the tests |

Splitting is for maintainability. It does not make the app faster; that came from message virtualization in 0.2.3.

## Testing

- `tests/unit/polish-0-2-4.test.ts`: `formatCountdown`, timer choices, `readSavedApprovalMode`/`shareApprovalMode` with fake storage and target, `permissionLabel`; source checks that setup keeps every setup call, that the wizard has one "More options" disclosure and no `formatBytes(ramTotal)`, that `ToolCallCard` expires to `"allow"`, and that config defaults are Off and 60.
- Existing tests that read moved code are pointed at the new files.
- Visual check of setup in the preview harness (`preview.html`, fake backend) in both themes with Playwright.
