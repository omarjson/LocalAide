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
