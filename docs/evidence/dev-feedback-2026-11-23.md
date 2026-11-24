# Dev feedback — 2026-11-23 (v0.2.2 draft)

Source: Omar's hands-on report on macOS after running the v0.2.2 dev build.
This is a note-taking pass only: **no code was changed**. Measurements below
are *reported by the operator*, not captured by Hermes.

## 1. Reported symptoms

| ID | Symptom | Operator's words / figures | Status |
|---|---|---|---|
| P1 | High memory use | "eating up to 420 or 450 megabytes of RAM" | reported, not profiled |
| P2 | High CPU / heat / fan | MacBook A2251, i7 10th gen, 32 GB RAM; "95 Celsius", fans "6000 RPM" | reported, not profiled |
| P3 | Rust advantage lost | "defeats the purpose of Rust and is now even worse than Chromium-like Electron full apps" | operator judgement |
| U1 | macOS title bar is "kind of the worst" | explicit dissatisfaction | reported |
| U2 | Two sidebar controls, one ineffective | "collapse and open window or open sidebar button in the title bar, which does nothing. Why are there two of them, and the one in the title bar just shows the titles without even opening the side menu?" | reported |
| U3 | Chat tabs interfere with title bar | "still that chat bar is interfering with the title bar" | reported |
| U4 | Double-click title bar does not zoom | "when I click twice on the side on the title bar, supposed to be it gets to the size of the window, but on macOS it doesn't" | reported |
| W1 | Setup repeats after every update | "when I update the app, it still goes to new setup process each and every update" | reported |
| W2 | Update UX not as expected | "auto checking for updates in-app" missing; expected: show update → show fixes/enhancements log → click update → wait → click install → auto-install → reopen | reported |
| W3 | Provider duplication | "kilocode showed it to me like three times... like it registers kilocode three times" | reported; needs reproduction |
| S1 | Windows not yet tested | operator will test next; expects some of the same issues there | pending |
| G1 | Social stakes | "it's embarrassing to me... they told me that this is embarrassing" | context, not a technical task |

## 2. Code anchors for the reported behaviour

- **Startup setup gate:** `src/App.svelte:289` sets `screen = cfg.selected_models?.length ? "chat" : "setup"`.
  An online-only setup (no local `selected_models`) would return to Setup on
  every launch even if providers, chats and preferences are intact. This is a
  strong candidate for W1 but must be confirmed against the user's config.
- **Update check exists:** `src/App.svelte:338-347` runs a quiet
  `api.checkForUpdate()` six seconds after launch, and `src/App.svelte:575-595`
  renders the update chip. `src-tauri/src/commands/updates.rs:70` and `:149-218`
  implement check, download, install and restart.
- **Draft release caveat:** `docs/release.md` states the in-app updater reads
  `releases/latest/download/latest.json` and "a draft is never offered to
  users". The v0.2.2 release is still a draft, so W2 may be partly a release
  state issue rather than a code defect. Signing keys also affect
  `can_install`.
- **Title-bar menu button:** `src/components/TitleBar.svelte:97-100` defines an
  `onMenu` button with `lg:hidden`; the sidebar has its own collapse control
  at `src/App.svelte:492-498`. The two controls and their visibility rules are
  a plausible source of U2.
- **macOS chrome:** `src-tauri/tauri.macos.conf.json` uses an overlay title bar
  with native traffic lights. No `dblclick` zoom handler exists in
  `TitleBar.svelte`; the only custom maximize action is the non-mac button at
  `src/components/TitleBar.svelte:171`. This matches U4.
- **Kilo preset:** `src-tauri/src/commands/cloud.rs:68-79` defines exactly one
  `kilo` preset. Duplication is more likely in model grouping, saved provider
  state or the picker's `providerList`/`providerModels` mapping than in the
  preset constant itself. Needs a screenshot/config dump before a fix.
- **Performance instrumentation:** `getSystemHealth()` and the request log were
  added for this exact purpose. No Activity Monitor `sample` of LocalAide and
  `llama-server` has been supplied yet, so P1/P2 remain unattributed.

## 3. Proposed action items (not started)

| ID | Outcome | Dependency | Acceptance condition | Risk |
|---|---|---|---|---|
| P1/P2 | Profile RSS and CPU on macOS and Windows; identify the top 3 consumers | Operator sample or local diagnostics capture | A written breakdown with file/function evidence, not a guess | low |
| P1/P2 | Reduce idle RAM and eliminate runaway CPU loops | profiling above | Measurable drop from the reported 420–450 MB baseline; idle CPU near zero | high — needs regression tests |
| U1/U3 | Redesign macOS chrome: separate title/drag area from chat tabs | visual review | No overlap between chat tab strip and window controls; clear draggable area | medium |
| U2 | One unambiguous sidebar control; remove or wire the title-bar button | UI review | One visible control per window size; every visible button has a real action | low |
| U4 | Double-click on macOS title area toggles zoom | Tauri macOS behaviour check | Double-click works like native title bars | medium |
| W1 | Setup completion no longer depends only on `selected_models` | config inspection | Online-only or restored-preferences users start in Chat | medium |
| W2 | Update flow shows release notes before install and keeps prefs/data | published release + signing check | Chip → notes → download → install → restart; config and chats intact | high |
| W3 | Reproduce and remove duplicate provider/model entries | screenshot/config | "Kilo Code" appears once per configured provider, not three times | medium |
| S1 | Repeat the same checklist on Windows | operator testing | Same findings recorded per platform | low |

## 4. Release implications

- v0.2.2 is a **draft**. Do not publish it until P1/P2, U1–U4, W1 and W3 are
  resolved on `dev` and the operator approves.
- `main` remains v0.2.1. Continue testing on `dev`; merge to `main` only after
  the operator says the build is no longer embarrassing.
- The next `dev` iteration should be framed as a lightweight, profile-driven
  release, not a feature-add release.

## 5. Open questions

1. Is the repeated Setup caused by online-only configuration, or by config
   being replaced during update?
2. Is the update chip absent because the release is a draft/unsigned, or
   because the check itself fails on macOS?
3. Does "Kilo three times" mean three provider rows, three model rows under
   one provider, or a saved duplicate in `config.json`?
4. Which process is hot: LocalAide WebView, `llama-server`, embedding/rerank
   sidecars, or the index watcher?

## 6. Fixes shipped on `dev` — 2026-11-23 (Batch 1 + queue/update UX)

One commit on `dev` (after `b7581de`). Verified locally: `npm run test:unit` 125 pass, `npm run check` 0 errors, `cargo fmt --check` clean, `cargo test` / `cargo clippy` via CI on push (see the `dev` run). No merge to `main`, no release publish — Omar tests on `dev` first.

| ID | What was wrong (from code, not guessed) | Fix |
|---|---|---|
| Send repeats previous prompt | `ChatInput` passed `send` bare as the click handler, so the MouseEvent landed in `send(resend, …)` as truthy. Enter calls `send()` and worked. | Button calls `() => onSend()`; `send` only resends on an explicit `true`. `onSteer`/`onQueue` wrapped the same way. |
| W1 setup repeats | `decideStartScreen` gated on `selected_models` only; online-only config has none. | Durable `setup_complete` flag in `config.json`, set when the wizard finishes or is skipped (`config_setup_complete`); startup also counts saved online providers. |
| U1–U4 title bar | Dead menu button duplicated the sidebar control; tabs shared the chrome row with the traffic lights; no double-click zoom; project name in the bar. | Logo opens a Minimize / Restore / Close menu; menu button removed; project name removed; macOS tabs moved to their own row below the chrome (Windows keeps them in the row); double-click on empty chrome zooms. Narrow windows: Ctrl+B now opens the sidebar drawer instead of toggling an invisible collapse. |
| W3 Kilo x3 | Same provider (same address + name) could be added repeatedly with different default models; each entry rendered its own picker group. | Save refuses a same-address + same-name provider; `ConfigStore::load` merges duplicates (first id kept, models unioned, active provider and favourites remapped); picker de-duplicates models by key and groups by provider id. |
| Kilo "Ultra" | Unknown online models got the full effort ladder (… max, ultra); Kilo's gateway accepts a `reasoning` parameter for nearly all its models, so every one looked like a thinking model with an Ultra rung. | Unknown online models now offer only low/medium/high. Known families keep their own ladders. |
| Steer not working | Steers are only read at the top of a later tool round; a plain answer has no later round, so the note was dropped and parked as a new message. | When a note is waiting after a round with no tool calls, the loop runs one more round with it instead of breaking. |
| Queue unclear | Edit moved the text back to the composer; no reorder; no confirmation that queueing worked. | Inline edit in the row, up/down reorder buttons, toasts on queue and on a note that arrived too late (queued as the next message). |
| Motion | Settings sections replayed a fly-in on every switch (`{#key activeTab}`); chat search scaled in; queued notes flew in. | Section fly + key removed; chat-search modal opens instantly (`motion={false}`); queued/steering rows render without animation. |
| W2 update UX | The footer chip installed immediately, with no changelog shown. | The chip opens a dialog with the release notes first, then Install (download → install → restart, data kept). Settings → About already showed the notes; the chip did not. |

Still open, not guessed at:

- **P1/P2 performance (420–450 MB, 95 °C, 6000 RPM).** No Activity Monitor `sample` of LocalAide and `llama-server` has been supplied, so nothing was "optimized" blind. Needs Omar's sample (or a diagnostics capture) before any change; the diagnostics panel and request log already exist for this.
- **W2, second half.** The v0.2.2 release is still a draft and signing keys may be absent, so the chip may still not appear until publish/signing even though the flow itself now shows notes first. The draft release must be rebuilt from the new `dev` head before Omar retests the update path.

## 7. Idle heat on macOS, profiled — 2026-11-23 (Batch 2)

Omar's MacBook Pro A2251 (Intel i7-1068NG7, 32 GB) sat at 95 °C with the fans
at full speed while LocalAide idled. He ran a 60 s logger (see the diag zip)
and then `sample` captures of the WebView processes while hot.

What the profile proved (not guessed):

- LocalAide's own process: 6–17% CPU after a 1 s startup spike; RSS 61 → 91 MB
  over 55 s. No `llama-server` ran at all — local inference is not the cause.
- WebKit GPU process: 30–80% CPU sustained. WebContent: 13–33%.
  WindowServer: 32–48% sustained.
- WebContent main thread: ~13% of samples in a repeating
  `RemoteLayerTreeDrawingArea::updateRendering` timer → full flexbox layout
  of the whole page (`RenderFlexibleBox::layoutBlock`, `computeFlexBaseSize`)
  on every frame.
- GPU process `CA::CG::Queue`: `fill_shading` → `Mosaic::draw` → Metal
  `waitUntilCompleted` — the big radial-gradient backgrounds re-rasterized
  into an IOSurface every frame, saturating the on-die Iris GPU.
- Console: only macOS TextInputUI candidate chatter while typing. No panics,
  no LocalAide errors.

Root cause: the frame loop never stopped at idle. The always-on infinite
animations kept WebKit scheduling a rendering update every frame, and the
pulsing logo eye is an SVG `<circle>` — SVG animations run on WebKit's main
thread, so each frame did style + layout + paint of the page instead of a
cheap composited transform. The full-viewport gradients then re-rasterized
on every one of those frames.

Fix (this commit, `dev` only):

- `LogoMark`: the eye pulses once on mount, then stays still.
- `EmptyState`: the floating icon no longer loops forever on the idle screen.
- `ember-glow` panels: own layer (`translateZ(0)`), rasterized once.
- Chat header: solid background; the `backdrop-blur` (re-sampled every
  frame) is gone.
- Activity-only animations (typing dots, thinking label, index sweep, effort
  sheen, session pulse, skeleton shimmer, progress slide) get `will-change`
  so they run on the compositor.
- Left alone on purpose: the `box-shadow` breathers (effort glow, share
  pulse) — small elements, only while active. The stop button's pulse was
  removed in the 2026-11-24 batch (§8): a plain solid circle, with a Resume
  button after a stop instead. The `.app-canvas` root gradients were NOT
  promoted: a transform there would make it the containing block for
  `position: fixed` descendants (modals, popovers).

Re-measure: run the 60 s logger again on the new build and compare GPU %
and temperature. If `fill_shading` persists, the next step is replacing the
root radial gradients with a pre-rendered static image.

## 8. Chat feedback — 2026-11-24 (Batch 3, `dev` only)

Omar's screenshot (three subagents working a Black Friday search, 133 source
chips mid-answer) plus a list of fixes. No build was sent for this batch.

- **Sources under the final answer, collapsed.** `MessageBubble` rendered
  citations whenever they arrived, so they showed mid-stream, above the
  still-running answer. They now render only when the answer is done
  (`!streaming`), under its final text, behind a `Sources (N)` toggle that
  starts folded (`sourcesOpen = false`). The rail's Sources section is
  untouched.
- **Title bar: the tab said "Chat", the header said "New chat".**
  `tabThreadStarted` renamed a new tab to "Chat" once the thread got its
  first saved id. The rename is gone; an untitled thread is "New chat" in
  the tab and in the header until the model titles it.
- **Subagent visibility — verified, no code change.** Omar asked whether
  the main model can watch the subagents live, and whether subagents can
  talk to each other. The answer from `services/agent/mod.rs`
  (`run_subagents`): no, on both counts, by design. Each subagent gets a
  fresh context (system prompt + its task only, no chat history, no sibling
  tasks) and runs in its own thread; the main model waits for all of them
  and gets only their final reports ("Report from the subagent …") in its
  next round. The one-line `subagent_step` events go to the chat UI, not to
  the model. Subagents cannot message each other; the only path between
  them is through the main model after they finish. Changing that (live
  progress into the model's context, or a shared channel between
  subagents) is a design decision, not a bug fix — left for Omar to ask
  for.
- **Stop button: no pulse, plus Resume.** The `stop-breathe` ring is
  removed from `ChatInput` (`.stop-btn` is a plain solid circle now). After
  a stop the composer shows a Resume button (new `play` icon). Resume
  re-enters the same run: the backend takes `resume: true` on
  `ChatStreamRequest` and replaces the new message with a fixed "continue
  from exactly where you stopped" turn, while the history already ends with
  the partial answer; the frontend appends the new tokens to that partial
  message instead of starting a new one. The running answer is held by
  object in `send()`, not by index, so a steering note inserted before it
  cannot shift what the stream appends to; the `done` handler re-reads the
  answer's current index for its estimate. `stoppedAnswer` is set both by
  the stop itself and when the run reports `cancelled`, and cleared by any
  new send, a new chat, or switching threads.
- **"Compact now" and `/compact` always compact.** `compactConversation`
  takes a `force` flag (pure helper `forcedCompactionStart` in
  `contextUsage.ts`: keep only the last exchange). The manual path passes
  it; the automatic compact at the threshold does not. A chat with only the
  last exchange left reports "Nothing left to compact".
- **Steered notes show in the chat.** `steerText` now inserts the note
  into the transcript as a user message with `steer: true`, before the
  answer it steers, rendered small on the left with a "Steered this
  answer" label. The flag is a real `ChatMessage` field (Rust + IPC
  bindings), so it survives saving and reopening. Steers are excluded from
  the history sent to the model, from the context meter and fit checks
  (`sentMessages`), and from edit-and-resend.
- **The chat follows progress.** The follow-stream effect only tracked the
  last message's length, so subagent steps, to-dos and tool rounds did not
  pull the view down. It now also tracks a fingerprint of the answer's
  parts (tool id/status/step count, to-do count, text/reasoning length,
  question answers).
- **Skins removed.** `SkinPanel.svelte` and the whole skin system are
  gone: the theme module's skin functions, the restore at startup, the
  Settings panel. Accent, font, density and light/dark theme stay — those
  are appearance settings, not skins.
- **Model prices come from OpenRouter.** The hand-typed "Model costs"
  table in Settings is gone, with it the `settings_model_prices` /
  `settings_set_model_prices` commands and the `model_prices` config
  field. The new `openrouter_model_prices` command fetches OpenRouter's
  public `/models` list (no key, cached ten minutes) and returns USD per
  1M tokens keyed by full id and short id; `ProviderModel` now keeps the
  `pricing.prompt` / `completion` / `input_cache_read` values the parser
  used to drop. The Usage screen prices from that; a model OpenRouter does
  not list prices at $0. The developer Usage ledger panel keeps its own
  manual rate field — it is a diagnostic for recording arbitrary calls,
  not the user-facing price table.

Tests: `tests/unit/feedback-2026-11-24.test.ts` (source-text regressions
for every item above, plus unit tests for `forcedCompactionStart` and the
OpenRouter price parsing). 135 unit tests pass; `svelte-check` 0 errors
(the 1 warning is the pre-existing `context="module"` deprecation in
`Icon.svelte`); `cargo fmt --check` clean. Rust verification is CI's job
(no local GTK/WebKit libs): clippy + cargo test on the CI run.

## 9. Screenshot feedback — 2026-11-24 (Batch 4, `dev` only)

Omar's four screenshots (Settings online section, the Memory sliders,
answer stats under a message with the switch off, the Context window
popover) plus two subagent asks, all on the `dev` build from Batch 3.

- **Settings opened scrolled past its content.** `selectSection` swapped
  the section without touching the scroll position, so a short section
  inherited a tall section's `scrollTop` and the browser clamped it —
  the section's own content sat above the viewport and the page looked
  blank. `selectSection` now calls an `onSectionChange` hook and App
  scrolls the canvas (`.app-canvas`) to the top. Per-screen scroll
  memory in `navigate()` is untouched: coming back to Settings still
  keeps the place.
- **Settings rows wrapped a word a line.** At Omar's ~1024px window the
  settings column is ~440px wide (220px section nav + sidebar +
  padding) and the slider + value ate ~265px of it, leaving ~105px for
  the title and description. `SettingsRow` used a viewport breakpoint
  (`sm:flex-row`), which cannot see the column. The content column is
  now a container (`@container`) and the row switches on `@md`
  (28rem): stacked under the text when the column is narrow, side by
  side when it is wide. Verified in the production CSS build
  (`container-type:inline-size` + `@container (width>=28rem)` rules).
- **Answer stats showed with the switch off.** `showStats` was
  `answerStats || flag("answer_stats")`, and the developer flag forces
  it on — so with developer mode on, the switch could never turn the
  stats off. The switch is now the control (`showStats = answerStats`);
  the flag only seeds the default, once, when developer settings load
  and the user has never chosen (no stored choice).
- **Context window had no cache hit rate.** `lastCacheRate` is set only
  when the last answer's `Done` event carries both `prompt_tokens` and
  `cache_tokens`; the local engine on older builds never sends
  `prompt_tokens_details.cached_tokens`, so the row never rendered. The
  chat now also loads the ledger average (`usage_summary`, same math as
  the Usage screen) and the meter shows the last answer's rate, else
  the recent average (labelled as one), else "—" with a note that the
  model does not report cached tokens.
- **Subagents: token use and a read-only side view.** The subagent
  thread now emits a final `SubagentStep` with `tokens` (prompt +
  answer, from the run's `RunResult`) when its run finishes; exact
  tokens only exist at the end of a round, so no mid-run guess is
  shown. The event, the IPC bindings and the saved tool part all carry
  the total, and `ToolCallCard` shows it by the subagent's step. A View
  button (sibling of the card header, not nested in it) opens
  `SubagentTranscript.svelte`: a right slide-over with the task (parsed
  from the tool call arguments), the step log, the report and the token
  total — no composer, no steer. The full turn-by-turn transcript is
  not kept by the backend (only the report comes back), so the step log
  is the record; that limit is stated in the panel. The Rust
  `MessagePart::Tool` also gained `steps` and `tokens`, because saved
  threads round-trip through it and both used to be dropped on save —
  reopened chats lost the subagent's step log.

Tests: `tests/unit/feedback-2026-11-24-batch4.test.ts` (source-text
regressions for every item above, plus unit tests for `addSubagentStep`
with tokens). 141 unit tests pass; `svelte-check` 0 errors (the 1
warning is the pre-existing `context="module"` deprecation in
`Icon.svelte`); `cargo fmt --check` clean; `npm run build` passes and
the built CSS contains the container-query rules. Rust verification is
CI's job (no local GTK/WebKit libs): clippy + cargo test on the CI run.

## 10. Inspiration backlog, NOW items — 2026-11-24 (N1–N3, `dev` only)

From Omar's approved inspiration backlog
(`workspace/goals/kiroween-2026/files/inspiration-backlog.md`): the three
NOW items, all small and off the Tiny demo path — the demo runs on the
local model, and failover only ever triggers for a remote target.

- **N1 — provider failover.** `stream_with_retries` now marks the error
  retryable only for 429 / 408 / 500 / 502 / 503 / 504, for a network
  failure ("Could not reach…") and for a stalled remote stream; 4xx and
  user stops are never retryable. Same-provider retries are capped at 3
  (`SAME_PROVIDER_RETRIES`, was 10) so a provider that stays down hands
  over instead of burning ten waits. In `run_inner`, a retryable error
  before any token of the round has landed walks the user's provider
  order (pure `failover_chain`, unit-tested in Rust: skips the current
  and already-tried providers, keeps the user's order) and ends on the
  local model; the same round re-runs on the new provider, the
  flavor-dependent request options are rebuilt (tools state and the
  last-round tool ban are kept), and the chat gets one line:
  "{from} was busy — continued on {to}". Fail-open throughout: an
  unreadable config, an unbuildable endpoint, or no loaded local model
  leaves the original error standing, unchanged. A local chat never
  fails over to a cloud provider (that would send the conversation
  somewhere the user did not choose), and a failure after the first
  token never fails over (the new provider would answer from scratch
  and its text would land after the partial answer). One known
  inaccuracy, left as-is for this batch: the usage record attributes
  the turn to the originally chosen provider, not the one that finally
  answered.
- **N2 — never stall-timeout local models.** The 10-minute
  `STREAM_STALL_LIMIT` no longer applies when the endpoint flavor is
  `LlamaServer`. The rule is written at the constant and at the check:
  a local model's silent thinking phase is healthy. `0` = disabled
  wherever a timeout is configurable. The 60-minute whole-request
  timeout is untouched — it is a hard bound, not a stall budget.
- **N3 — `__omit__` for `reasoning_effort`: no code change.** Checked
  first, as instructed: `adjust_for_error` already drops the effort
  (and then the field itself) when a provider refuses it with 400/422,
  and the existing test proves the retry goes out without
  `reasoning_effort`. No model is known to need the field omitted
  up-front, so there is nothing to add.

Tests: `tests/unit/inspiration-now.test.ts` (10 tests: retryable classes,
retry cap, failover arm conditions, chain shape, the one-line message,
fail-open guarantees, stall exemption, N3 already-handled) plus three
Rust unit tests for `failover_chain`. 151 unit tests pass;
`svelte-check` 0 errors (the 1 warning is the pre-existing
`context="module"` deprecation in `Icon.svelte`); `cargo fmt --check`
clean; `npm run build` passes. Rust verification is CI's job (no local
GTK/WebKit libs): clippy + cargo test on the CI run.

## 11. Inspiration backlog, NOW items Batch A — 2026-11-24 (N6, N7, N12–N18, `dev` only)

UI/rules batch; nothing here touches the Tiny demo path (no streaming,
no provider routing, no model loading changes beyond a Load/Unload button
that calls the existing commands).

- **N6 — hardware-aware Models page.** `system_probe` now also returns
  the CPU brand and physical core count (sysinfo was already a
  dependency; no new crate). `gpu_vendor` stays `None` on purpose: no GPU
  source exists in this stack, so the page says "Not detected" instead of
  inventing one. The fit heuristic lives in `src/lib/hardware.ts` and is
  documented there: "runs well" = inside the model budget, "runs slowly"
  = fits in free RAM but over the budget, "too large" = bigger than free
  RAM; with no budget known only free RAM decides. One "Recommended for
  you" (the most capable chat model that runs well, installed first on a
  tie; the smallest slow one when nothing runs well; none when nothing
  fits) replaces the old three-model list. The memory chip under the
  composer shows GB in use with Load/Unload for the local model. A failed
  probe hides the card, the badges and the chip — the page still lists
  models.
- **N7 — typed empty states + honesty rules.** `src/lib/api/errors.ts`
  now classifies every failure into a cause class (`ErrorCause`) with one
  fixing action per class (`ERROR_FIXES`, exhaustive by `satisfies
  Record<ErrorCause, string>` — a new cause without its fix is a type
  error). The generic fallback is gone: an unclassifiable error reads
  "Unknown error — LocalAide couldn't tell what went wrong." `ErrorBanner`
  takes an optional `fix` line; the Usage screen wires it. The Usage
  screen also names the window its totals actually cover when retained
  history is shorter than the requested window (`windowCoverage` in
  `src/lib/usage.ts`; the ledger only keeps days that had answers), and
  the Cost card names the models it leaves out for want of a price.
- **N12 — interactive-question conventions.** The model's `ask_user`
  tool can mark one option `recommended: true` (schema + description in
  `tools.rs`, parsed in `parse_questions`, carried on `QuestionOption`);
  it is shown first and marked "Recommended" in the QuestionDock, the
  parsed-question buttons and A2UI ChoicePicker forms, all through one
  `orderOptionsRecommended` helper. Nothing is marked recommended unless
  the model said so. The free-text answer already existed in both
  question UIs ("Other…" focusing the composer; the dock's typed-answer
  box) and is now named in the buttons' hint line. A2UI forms submit
  structured data, so no free-text box was added there.
- **N13 — composer bar-fit.** Measured-width, not container queries:
  the pills span ChatInput and snippets owned by ChatScreen, so a
  `ResizeObserver` in ChatInput re-measures the row and sets a
  `data-fold` level (0–3); the rules live in `main.css` so they reach
  every pill, and the fold CSS is verified present in the vite build
  output. Fold order: Tools label, model name, effort label; permission
  and usage are already glyphs, team and context have no pill here.
  Pills unfold last-folded-first as room returns; a folded pill keeps
  its name (aria-label/title). Send is shrink-0 and last. The row keeps
  `flex-wrap` as the ultimate fallback (a wrapped Send is still visible),
  because `overflow-hidden` would clip the attach/tools menus that open
  upward from the row.
- **N14 — "New chats start on".** The last-used model is remembered in
  localStorage on every pick (`src/lib/lastModel.ts`); a new chat starts
  on it when it is still usable (an online model in the picker, or a
  local one the engine has loaded). Otherwise the chat falls back to the
  active model and says so in one line on the empty chat. There is no
  separate "configured default" setting in the app; the active (loaded)
  model is the fallback. Per-chat saved models are untouched.
- **N15 — header manners.** The chat title is a button: one click opens
  the chat menu (after a 250 ms beat, so a double-click can cancel it),
  a double-click renames; hovering a cut-off title shows the whole
  title. Tabs show their full title on hover too. "New chat" stays until
  the model titles it.
- **N16 — time-of-day greeting.** `greetingAt` in `src/lib/greeting.ts`:
  morning / afternoon / evening, "Nice work today" only in the evening
  and only when a chat was updated today, a gentle late-night line after
  23:00. One line, re-evaluated at the next hour boundary. The empty
  chat uses it; `EmptyState` takes an optional `greeting` line (used by
  the Models screen's no-model state, where no chat data exists so the
  worked-today variant never claims work).
- **N17 — Tauri shell rules.** (a) Autostart: not applicable. No
  autostart feature exists anywhere in the app (no plugin, no setting,
  no marker in `tauri.conf.json`, `Cargo.toml` or the Rust sources, and
  no startup hook beyond the saved-model engine load), so the one-time
  marker rule has nothing to attach to — no autostart was built from
  scratch. (b) Status: already compliant — the header chip only says
  "Online" when an online model is actually chosen; otherwise it says
  "Offline · this computer". (c) Toast: portaled to the document root in
  `App.svelte`, so reporting costs the page no layout.
- **N18 — i18n rule.** Convention, adopted as a rule: technical text is
  never translated — model ids, commands, file paths, URLs, env vars,
  HTTP headers, CLI samples. Checked first: the UI has no Arabic strings
  at all (the app ships English; Arabic only reaches it as user content
  and speech, rendered with `dir="auto"`), so there was no
  currently-translated technical text to fix. A unit test guards the
  source against Arabic-script text.

Tests: `tests/unit/inspiration-now-batch-a.test.ts` (19 tests: fit
heuristic, recommendation, hardware card, memory chip, no-generic-error,
exhaustive fixes, classification, banner fix line, window coverage, cost
caveat, recommended-first ordering, question-UI wiring, fold order +
built CSS, last-used resolution + fallback line, header manners,
greeting variants, portaled toast, Offline default, no-autostart,
no-Arabic-script) plus one Rust unit test for the `recommended` flag
parsing. 170 unit tests pass; `svelte-check` 0 errors (the 1 warning is
the pre-existing `context="module"` deprecation in `Icon.svelte`);
`cargo fmt --check` clean; `npm run build` passes. Rust verification is
CI's job (no local GTK/WebKit libs): clippy + cargo test on the CI run.

## 12. Inspiration backlog, NOW items Batch B — 2026-11-24 (N4, N5, N8–N11, `dev` only)

Behaviour batch. The send path was touched in three places, all
fail-open (see each item); the Tiny demo path is unaffected — the
compact prompt defaults off and the auto-compact default only moves the
threshold, it does not change what a fresh short chat sends.

- **N4 — auto-compact at ~95% + pruning.** (a) The threshold now
  defaults to 95% (was 80; the slider already topped out at 95; a saved
  value is kept). (b) Anchored summary: when auto-compact cuts the
  history, the cut part is summarised by the loaded local model for free
  (`summarize`, instruction: goal, decisions, progress, next steps) and
  the summary rides on the system prompt of later sends, so the model
  keeps the thread. It folds in the previous summary, so the anchor
  covers the whole cut span. If no local model is loaded, or
  summarising errors, the summary stays as it was and the plain cut
  stands — the send never waits on it. The summary is session-only,
  like the cut: the full history stays on screen and in the saved chat;
  nothing is deleted by compaction, ever. Manual `/compact` is
  unchanged: it always cuts and writes no summary. (c) Pruning: tool
  parts now travel with the history (`ChatScreen` sends `parts`,
  `build_history` keeps them) and `build_messages` folds them in —
  the newest two messages keep one short line per tool (title + summary,
  200 chars each, at most 4 per message), older ones become
  "[Tool output from an earlier turn was omitted to save space.]".
  A history with no tools builds exactly what it built before
  (Rust test). Note: before this, tool outputs never reached the model
  in later turns at all — they lived only in the on-screen parts. Now
  the model sees recent tool results as short lines, which is the
  backlog's intent, and old ones are pruned instead of silently dropped.
- **N5 — per-task model roles.** `resolveChoreModel` in
  `src/lib/modelPicker.ts`: when a local model is loaded, chores run on
  it (the active local model, else the first local one); when none is
  loaded it returns null and the caller keeps its current behaviour
  instead of loading one. Wired for chat titles and "suggest next" in
  `ChatScreen`. The N4 summary already runs on the loaded local engine
  only (`summarize` errors when nothing is loaded, and the summary is
  then skipped) — no extra wiring needed there. Memory writes have no
  separate chore call to re-route: they happen inside the chat turn
  (the SaveNote tool, on the chat's model) or by the user directly
  (`memory_create`), so they are deliberately unchanged. Chores never
  block the chat: title/suggestion failures were already swallowed,
  and the summary is fire-and-forget.
- **N8 — session goals as a state machine.** `GoalStatus` gains
  `Paused` and `Blocked` (unknown values still parse as Active, so
  existing rows stay valid). Pure transitions `apply_pause` /
  `apply_block` / `apply_resume` plus `pause_at` / `block_at` /
  `resume_at` and the `goal_pause` / `goal_block` / `goal_resume`
  commands; `GoalPanel` shows the status (Paused amber, Blocked red)
  with Pause / Resume / Mark blocked buttons. Auto-pause: `drive`
  moves a goal to Paused when the step budget runs out without
  completion and to Blocked when the caller reports a blocker, and
  `decide_next` stops on Paused/Blocked (new `StopReason::Paused`);
  the panel persists the same moves — budget used up pauses, a failed
  command pauses (runNext/runAll catch), the user hitting Stop pauses,
  and a refused permission pauses (the backend tells the model "The
  user declined this action", so the runner spots those words in the
  answer; best-effort detection, and the goal only pauses, never
  auto-resumes). A paused/blocked goal takes no steps and cannot
  complete until resumed. Goals survive restarts: they already persist
  in SQLite, verified by a round-trip test that pauses, resumes,
  blocks and resumes the same row.
- **N9 — granular per-capability auto-approve, default read-only.**
  New `CapabilityAutoApprove` in config (read / edit / safe commands /
  all commands / browser / external tools), defaulting to read-only:
  only reads are auto-approved, everything else asks; a missing field
  in an old config falls back to the same default, so unknown/missing
  config asks, never auto-approves. "All commands" covers safe ones.
  Exposed as plain-language toggles in `PolicyPanel`, saved through
  the existing `policy_config_set` (now carrying
  `auto_approve_capabilities`). Consulted in the two places a tool
  approval decision is made: the agent's `run_call` (a capability
  approval skips the prompt and is recorded as "auto-approved in
  Settings", distinct from "allowed for this chat") and
  `ToolPolicy::decide_shell` (safe-commands allows a tame change,
  all-commands allows any non-blocked command; deny rules still win,
  and a blocked command can never become Allow). Reads and web lookups
  never needed approval on their own, so their toggles are posture —
  the behaviour change is edits, commands and MCP tools. Per-chat
  `allowed_tools` and the per-server `auto_approve` are untouched.
- **N10 — retry with backoff + plain-English errors. Deliberate
  deviation: the per-target cap stays at 3, not 5.** The backlog asked
  for up to 5, but the N1 failover work deliberately capped
  same-provider retries at 3 because failover is the outer loop and
  the chain already reaches 5+ attempts in total; raising it would
  burn waits on a provider that stays down instead of handing over.
  (a) Network errors now retry the same way as a busy provider:
  `stream_once` connect failures (and the stall timeout) are
  retryable, and `stream_with_retries` retries them with the same
  exponential backoff (`2^tries` seconds, provider `retry_after`
  wins, extracted as the pure `backoff_delay`), under the same cap of
  3 — but only before any token arrived (a `saw_token` guard; retrying
  mid-answer would show the start twice) and only while the user has
  not stopped the run. A mid-stream break is not retryable and still
  fails at once. (b) `http_error_message` now leads with plain English
  — "Groq is busy right now. Wait a moment and try again, or switch
  models." / "Couldn't reach Groq — check your connection and try
  again." — with no bare status code; the provider's own error text
  still follows, capped, for debugging, and the raw network error
  moves to `AppError.detail`. The same helper now serves the second
  error path (`remote.rs`, which had its own copy of the old wording),
  so the chat shows the same sentence whichever route an online turn
  took. The error reaches the UI unchanged otherwise: the `AppError`
  message is what `ChatScreen` shows in the banner / failed answer.
  One pre-existing test (`inspiration-now.test.ts`) asserted the old
  "Could not reach …" wording; it now asserts the new wording, keeping
  the retryable check.
- **N11 — "Use Compact Prompt" toggle for local models.** New
  `compose_compact_system_prompt` / `compact_tool_guidance` in
  `modes.rs` (shorter mode instruction, one-line tool guidance) and a
  `use_compact_prompt` config flag, default off, with a toggle in
  Settings → Performance ("Use Compact Prompt", next to Conversation
  memory) via new `settings_compact_prompt` /
  `settings_set_compact_prompt` commands. The compact prompt is used
  only when the target is local AND the setting is on; the default-off
  path calls the original composers, so its prompt is byte-identical
  to before (Rust test: compact is never longer than full, and agrees
  exactly for Standard). The Tiny demo path is unaffected until Omar
  turns it on.

Tests: `tests/unit/inspiration-now-batch-b.test.ts` (14 tests: 95%
default, anchored summary wiring + fail-open, manual compact
unchanged, parts in history, chore resolver + title/suggestion
wiring, paused/blocked planner + drive auto-pause, goal store +
panel, policy toggles + config/policy wiring, cap-stays-3, plain
errors + network retry, compact prompt local-only/default-off)
plus Rust unit tests for the capability defaults, goal
pause/block/resume + drive auto-pause + SQLite round-trip, compact
vs full prompts, tool-output pruning, plain errors and backoff.
184 unit tests pass; `svelte-check` 0 errors (the 1 warning is the
pre-existing `context="module"` deprecation in `Icon.svelte`);
`cargo fmt --check` clean; `npm run build` passes. Rust verification
is CI's job (no local GTK/WebKit libs): clippy + cargo test on the CI
run.
