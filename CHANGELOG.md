# Changelog

All notable changes to LocalAide are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] - 2026-11-23

A large local-first upgrade: one window chrome row, a diagnostics and repair
surface, secrets in the OS keychain, durable tool policy, session search,
agents, goals, multi-run, project rules, notifications, paired sharing and
plugin panels. Settings are re-ordered, the model picker keeps the
conversation when switching models, and config writes skip unchanged saves.

### Added
- **One window chrome row.** The title bar and chat tabs are merged into a single 40px row on Windows and Linux. On macOS the tabs sit in their own row below the chrome, so they no longer crowd the title bar or the traffic lights. macOS uses an overlay title bar with native traffic lights; Windows and Linux keep the custom minimize/maximize/close buttons. The logo opens a small window menu (Minimize / Restore / Close); double-clicking empty chrome zooms the window.
- **Local diagnostics.** Settings → About shows uptime, resident memory, backend-call timings and local log/database sizes, plus a **Copy diagnostics** button that assembles a redacted, review-first report. Nothing is uploaded.
- **OS keychain for provider keys.** API keys move to the macOS Keychain, Windows Credential Manager or Linux Secret Service; `config.json` keeps only a `keyring:` marker. A headless machine with no native store keeps the owner-only file fallback rather than losing the key.
- **Session search, doctor, agents, multi-run and context references.** Search old chats with FTS5, run a local health report, manage Markdown agents, compare answers from several providers and resolve safe `@file:line` references.
- **Permission rules, tool hooks and provider routing.** Durable allow/ask/deny rules, pre/post tool hooks, MCP tool gating, provider priority and key pools.
- **Goals and usage ledger.** Goal state with a step budget and a documented flat-rate usage ledger for online calls.
- **Paired sharing and plugins.** Shared sessions now need a one-time pairing token; local HTML plugin panels are listed safely and shown in a sandbox.
- **Project rules and model-switch notes.** `AGENTS.md`-style files are injected into chat prompts when enabled, and switching models reports the recalculated context budget without losing the conversation.
- **Local notifications and sounds.** Opt-in desktop notifications with a short chime when a goal completes, a background turn finishes, a tool approval is needed, or the model asks a question. No network calls.

### Fixed
- **The Send button sent the previous message again.** Clicking Send passed the click event into `send`'s `resend` parameter, so it resent the last message instead of the text just typed (Enter worked, because it calls `send()` with no argument). The button now calls `send()` with no argument, and `send` only resends on an explicit `true`.
- **Setup repeated after every update.** Startup only skipped setup when a local model was chosen, so online-only users saw the wizard on every launch. Setup is now recorded with a durable `setup_complete` flag when the wizard finishes or is skipped, and startup also counts saved online providers.
- **Steer did nothing on a plain answer.** Steering notes were only read at the top of a later tool round, and a plain answer has none, so the note was dropped and parked as a new message. A waiting note now runs one more round instead, so the model reads it before answering again.
- **Kilo Code listed three times, all models on Ultra.** Adding the same provider (same address and name) twice is now refused, an existing config merges the duplicates on load (keeping the first entry's id, models and favourites), and the picker de-duplicates by key. Unknown online models now offer only low/medium/high effort: the full ladder put an Ultra rung on models that do not have one.
- **Queued messages can be edited in place and reordered.** Edit now edits the row itself (it used to move the text back to the composer), with up/down buttons to reorder. Queueing and steering now confirm with a short message, so it is clear they worked.
- **Update flow shows what's new first.** The footer chip now opens a dialog with the release notes and an Install button, instead of installing straight from the chip. Install still downloads, verifies, installs and restarts, keeping chats, notes, models and preferences.
- **Extra motion is gone where it annoyed.** Chat search opens instantly, Settings sections no longer fly in on every switch, and queued notes appear without animation.
- **Library index watcher** now has automated coverage for 500ms debouncing, ignored extensions and removal forwarding.
- **Idle heat on Intel Macs (95 °C, fans full).** The WebView redrew every frame at idle: the pulsing SVG logo eye runs on WebKit's main thread (SVG has no compositor layer), so each frame re-ran layout of the whole chat tree and re-rasterized the big ember gradients on the GPU. The logo now pulses once on mount and then stays still, the empty-state icon no longer floats forever, the ember-glow panels get their own layer (rasterized once), the chat header is solid instead of a live backdrop blur, and the remaining activity-only animations (typing dots, thinking label, index sweep, effort sheen, session pulse, skeleton shimmer, progress slide) are promoted with `will-change` so they run on the compositor. Profiled from Activity Monitor samples, not guessed.
- Open chat tabs are capped live at 8 with least-recently-used eviction; the contrast gate reads the shipped focus token; tab close stays visible on touch.
- Provider keys rotate on real 401/429 responses, doctor can repair safe local scaffolding, and repeated identical config saves are skipped.
- **Online provider router and SSE engine** have loopback HTTP tests for clean and truncated streams, mid-stream errors, 401 hints, no-follow redirects and cancellation.
- **Sources sit under the finished answer, folded.** They used to render mid-stream (as citations arrived) and sprawl across the chat — 133 chips in the reported case. They now appear only when the answer is done, under its final text, collapsed behind a "Sources (N)" toggle.
- **A new chat's tab says what the header says.** The tab was renamed to "Chat" once the thread got its first saved id, while the header said "New chat". It now stays "New chat" until the model titles it.
- **The stop button no longer pulses, and stopping offers a resume.** The breathing ring is gone (one more always-on animation), and after a stop the composer shows a Resume button that tells the model to continue the stopped answer from exactly where it left off — the partial answer stays and is picked up, not regenerated. The run still reports `cancelled`, so the button also appears when the backend confirms the stop.
- **"Compact now" and `/compact` always compact.** They used to refuse when the conversation already fit the window ("This conversation already fits"). An explicit compact now always cuts to the last exchange, no matter the size; the automatic compact at the threshold is unchanged.
- **Steered notes show in the chat.** A steer used to leave only the model's truncated "Read your note: …" step (and that step is session-only). The note now lands in the transcript in full, before the answer it steers, labelled "Steered this answer", and survives reopening the chat. It is never sent back to the model as history.
- **The chat follows progress, not just the last message's text.** The auto-scroll only watched the last message's length, so subagent steps, to-dos and tool rounds did not pull the view down. It now follows a fingerprint of the answer's parts too.
- **Skins are removed, and model prices come from OpenRouter.** The skin panel, its import/export and the theme overrides are gone. The hand-typed "Model costs" table in Settings is gone too: the Usage screen now prices from OpenRouter's public `/models` list (cached ten minutes, keyed by full and short model id), so a model that is not listed prices at $0 instead of an invented number.
- **Switching settings sections starts at the top.** A short section inherited the previous section's scroll position, so it opened scrolled past its own content and looked blank. Changing sections now scrolls the page to the top.
- **Settings rows lay out by their own width.** At a narrow window the settings column is ~440px and the slider ate the label down to a word a line ("Memory / for / models"). Rows now use container queries, so they stack the control under the text when their column is narrow and sit side by side when it is wide.
- **Answer statistics obey their switch.** The switch was OR'd with the developer flag, so with developer mode on the stats showed even when the switch was off. The switch is now the control; the flag only seeds the default until the user picks.
- **The context window shows the cache hit rate.** It only appeared when the last answer's provider reported cached tokens, so local runs showed nothing. It now falls back to the recent average from the usage log, and when no run has ever reported cached tokens it says so instead of hiding the row.
- **Subagents show their token use, and open a read-only side view.** A finished subagent reports its total tokens (prompt + answer) on its card, and a View button opens a side panel with the task it was given, its step log and its report. The panel has no composer and no steer: subagents can't be talked to. The step log and token total now survive reopening the chat (they used to be dropped on save); the full turn-by-turn transcript is not kept, so the step log is the record.
- **A busy provider hands over instead of failing the turn.** When the chosen online model answers 429, 408 or 5xx, or cannot be reached, or goes quiet mid-stream, the turn retries on the next provider in the user's order — the local model is the last resort — and the chat says so in one line ("OpenRouter was busy — continued on Groq"). The switch only happens before any token of the round has landed, so a partial answer is never followed by a fresh one; a user stop, a 4xx (bad key, wrong model) or an exhausted chain still fails the turn with the original error. Same-provider retries are capped at 3 before handing over.
- **The local model is never stall-timed out.** The stream-stall budget (10 minutes of silence) no longer applies to the local server: a local model's silent thinking phase is healthy. It still applies to remote providers, where going quiet counts as stuck.
- **Models page knows the hardware.** It shows the processor, memory and graphics from the system probe, badges each model "Runs well / Runs slowly / Too large" (well = inside the model budget, slowly = fits in free memory but over the budget, too large = bigger than free memory), and recommends one model for this computer instead of three. A memory chip under the composer shows GB in use with Load/Unload for the local model. A failed probe hides the card and the badges instead of erroring.
- **Errors name their cause and their fix.** No error reads as a generic "something went wrong" any more: every failure is classified (network, not found, permission, auth, unavailable, invalid, cancelled, unknown), the banner shows the one action that fixes it, and the fix table is exhaustive by typecheck, so a new cause class without its fix is a build error. The Usage screen says which window its totals actually cover when retained history is shorter than the requested window, and the Cost card names the models it leaves out for want of a price.
- **Questions put the recommended option first.** The model's `ask_user` tool can mark one option `recommended: true`; it is shown first and marked "Recommended" in the question dock, the parsed-question buttons and A2UI forms. The free-text "Other…" answer stays available while the question is live.
- **The composer folds instead of crowding Send.** On a narrow window the pills fold to glyphs in a fixed order — Tools label, model name, effort label — and unfold last-folded-first as room returns; permission and usage are already glyphs. Send is shrink-0 and last, so it is never covered.
- **New chats start on the last-used model.** The model picked last is remembered across restarts; a new chat starts on it when it is still usable. When it is not (local model unloaded, provider key removed), the chat falls back to the active model and says so in one line instead of erroring or switching silently.
- **The chat title behaves like a title.** Clicking it opens the chat menu, double-clicking renames it, and hovering a cut-off title shows the whole title. Tabs do the same on hover. "New chat" stays until the model titles it.
- **The empty chat greets by the time of day.** Morning, afternoon and evening variants, "Nice work today" only in the evening and only when a chat had a turn today, and a gentle late-night line after 23:00. One line, re-evaluated at the next hour boundary.
- **The toast is portaled; status never defaults to Online.** The toast now renders at the document root, so reporting costs the page no layout. The header status chip only says Online when an online model is actually chosen; otherwise it says Offline on this computer. There is no autostart feature in the app, so the one-time autostart-marker rule has nothing to attach to — no autostart was built.
- **Technical text is never translated.** The UI has no Arabic strings; model ids, commands, file paths, URLs and env vars stay English by convention, and a unit test guards the source against Arabic-script text.
- **Auto-compact keeps the thread.** The threshold now defaults to 95% of the context window (was 80%). When it cuts, the cut part is summarised by the loaded local model — goal, decisions, progress, next steps — and that summary stays in what the model reads, so the conversation keeps making sense after the cut. Old tool outputs are pruned the same way: the newest turns keep a short line per tool, older ones become a placeholder. The full history stays on screen and in the saved chat; nothing is deleted. Manual compact is unchanged.
- **Small jobs run on the small local model.** Chat titles, "suggest next" and compaction summaries now prefer the local model already loaded on this computer — free, and never blocking the chat. When no local model is loaded, they use the chat's model as before.
- **Goals pause instead of spinning.** A goal now waits as Paused when its step budget runs out, a step fails, you stop the run, or a permission is declined — and as Blocked when something outside the run is in the way. Paused and blocked goals take no steps until resumed, and they survive restarts.
- **Permissions are per-capability.** Settings → Permissions now has plain-language switches for what runs without asking — read files, edit files, safe commands, all commands, browser, external tools. Read-only by default: only reading runs without asking. A missing or unknown setting always asks.
- **Errors say what happened, and the chat retries a lost connection.** A busy provider reads "is busy right now. Wait a moment and try again" instead of a status code, and a lost connection reads "Couldn't reach … — check your connection". Dropped connections retry with backoff before giving up, the same way a busy provider does — but only before any answer text arrived, and never past the 3-tries-per-provider cap (failover to the next provider is the outer loop, so the turn still gets 5+ attempts in total).
- **A shorter prompt for local models, off by default.** Settings → Performance has "Use Compact Prompt": shorter instructions for models on this computer, for faster first tokens. Off keeps the full instructions, and online models always get the full prompt.

### Changed
- Contrast checks now recompute WCAG ratios from the shipped CSS tokens in CI for both themes; the a11y evidence table carries the current palette values.

## [0.2.1] - 2026-11-22

Interactive forms in the chat, messages you can queue or steer while an answer runs, and online providers you add without leaving the chat. This release also fixes closing the window on Windows, the effort bar and the context meter, and makes installing over an older LocalAide one short wizard. Your chats, notes, models and settings are kept.

### Added
- **Interactive forms in the chat (A2UI 1.0).** When a form helps (a booking, a checklist, a quiz, settings to pick), the model can draw one with Google's open A2UI protocol, and LocalAide draws it with its own controls: text fields, checkboxes, choice chips, sliders, date pickers, tabs, cards, dialogs and buttons. Buttons stay disabled until the form's checks pass. Pressing one sends what you filled in back to the model as your next message (shown as "Sent from the form"), and a later answer can update the same form. Nothing on a form loads from the internet; a web image shows as a link you can open. If a form has a problem, the chat says what and offers "Ask to fix it". See `docs/adr/0013-a2ui-forms.md`.
- **Queue and steer while an answer runs.** The message box stays open during an answer. Enter queues a message, which is sent as soon as the answer ends; Alt+Enter (or **Steer**) hands a note to the running answer, and the model reads it before its next step. Queued messages can be sent now, edited or removed. A note that arrives after the model's last step joins the queue, so nothing you type is lost.
- **Add an online provider from the chat.** "Add an online provider" in the model menu (and "Use a free provider" on an empty chat) opens the provider form right there. Once it is saved, the chat switches to that provider's model. Adding a provider is the opt-in: its models appear at once, without a second switch in Settings.

### Fixed
- **Closing, minimizing and maximizing the window on Windows.** The custom title bar's buttons were missing their window permissions, so Close showed "Something went wrong" and left the window open. The buttons now work, and window dragging works too. A failed close falls back to closing the window outright instead of showing an error.
- **Reopening LocalAide right after closing it** no longer says "already open": a new start waits a few seconds for the closing copy to finish. The Automations scheduler stops when the app starts closing, so no run begins while the engines shut down.
- **The thinking effort bar responds anywhere on the bar.** Press or drag anywhere on the capsule to move to the nearest level; before, only the knob's top edge took clicks. The knob, its dots and the levels now line up.
- **The context meter's card opens above the meter** in the message box instead of under it, out of sight.
- **The context count is the model's own.** After each answer the meter starts from the engine's count of what was really sent (instructions, tool descriptions, history and question) and estimates only what came after. Without a count it uses a better estimate (four characters a token for English, two for other alphabets, one per Chinese, Japanese or Korean character), corrected by how far off the last estimate was. The real size of the instructions and tool descriptions replaces a fixed guess, images count, the room kept for thinking matches the chosen effort, and the breakdown on hover adds up to the meter.
- **The title bar no longer stacks on another "LocalAide" bar.** The sidebar's own logo row is gone, the narrow-window header folded into the title bar (its menu button opens the navigation), and the sliding sidebar opens below the title bar instead of covering it.

### Changed
- **Leaner context.** Tool descriptions now count toward the context budget, so long chats with tools no longer overflow small local models. Long arguments of earlier tool calls are shortened before whole turns are dropped. When the history takes more than half the room, files attached to older messages are cut to their first 2,000 characters (the newest four messages keep theirs whole). The current date and time ride on the new message instead of the system prompt, so a local model reuses its cached prompt instead of reading the whole chat again every minute.
- **Upgrading on Windows is one wizard.** When an older LocalAide is installed, the installer updates it in place, the way the in-app updater does: no "uninstall first?" page, no second uninstall wizard, and no license, folder or Start menu pages to click through. Reinstalling the same version or going back to an older one still asks.
- **Setup is shorter.** Checking the computer is no longer a step of its own; the recommended model installs in one click (other sizes sit under "Other sizes"); "Use an online provider instead" sets up a provider without leaving setup, and is suggested first when the computer has too little memory for local models; the last step picks Dark or Light. It also explains "For models" and says when the first start downloads the engine.
- In Settings → Online providers, the main switch can be turned on before any provider exists; doing so opens the form to add one.

## [0.2.0] - 2026-11-22

The biggest LocalAide release yet: chats run in parallel tabs, the assistant answers on a schedule, it builds real Word, Excel and PowerPoint files, and the Usage screen tells you exactly what every answer cost. Everything below keeps your chats, notes, models and settings.

### Added
- **Parallel chat tabs.** Every chat is a tab; several answers stream at the same time. The tab strip shows live dots for the chats that are answering, tabs keep their titles, and reopening LocalAide brings the tabs back with their conversations.
- **Automations.** Schedule a prompt for every day at 7:00, every hour, or every N minutes. Runs are read-only — nothing is written while you are away — and the run feed keeps the last 50 answers to read when you are back. "Run now" answers immediately.
- **Office Maker.** The chat can build real .docx, .xlsx and .pptx files inside the project folder: reports with headings, bullets and tables, spreadsheets with sheets of rows and real numbers, slide decks with titles and bullets. Writes ask first, like any file write.
- **ArtCraft Connect.** The ArtCraft panels already on this computer — documents, slides, sheets, images — appear in Tools and MCP as one-click servers, discovered from ZCode's own plugin cache. Nothing is downloaded.
- **Usage, told properly.** Sessions, prompts, tokens (in, out and cached), cost from your own price table, active days and the longest streak. A token-composition bar, an efficiency grid (cache hit rate, cost per session and per 1M tokens), Models seen as Tokens / Cost / Steps, tool calls on demand, windows from Today to All time, and a per-project filter with its own Projects card.
- **Context, on hover.** Rest the pointer on the context meter and it breaks the window down — messages, system tools, meta context, skills, system prompt — with the cache hit rate when the model reports one.
- **Phone pairing.** Sharing gains a QR code of the remote link and a short XXXX-XXXX code. The phone shows the same code; matching codes mean the phone and this computer are the same session.
- **Appearance.** Accent colors beyond Deep Ember (Ocean, Violet, Forest, Rose, Gold — each keeps the contrast audit), a font choice (System, Serif, Mono) and a Compact density. Applied before the first paint, so nothing flashes.
- **Plan cards.** The model's to-do list now shows item by item in the panel rail: done ticks ember, the step in progress carries the spinner.
- **Model costs.** Settings → General takes USD-per-1M prices per model (with a cache-read price); the Usage screen turns them into cost figures. Local models are free.

### Changed
- Three reply suggestions sit beside each other on one row; the stop button grew into a clearly pulsing stop circle; the wand got its sparkle back; sidebar icons each greet the pointer in their own way and pop when their screen opens.
- Light mode gained soft warm shadows instead of the dark theme's near-black depth, and all popovers share one elevation.
- The status bar quietly checks GitHub for a new release shortly after launch and offers to install from an ember chip; 0.1.9 already ships this, and 0.2.0 keeps it.

### Fixed
- Editing an old message keeps the future: the discarded tail parks on a checkpoint chip and can be restored — both paths stay flippable, and branches survive a restart.
- Model names for chats are AI-generated after the first answer, editable inline, regenerable.
- The chat's tool-call guard keeps models that emit `<tool_call>` as text from leaking tags into the transcript; the calls still run.
- Rate-limit (429 and friends) answers retry themselves with the server's wait or a growing backoff, ten tries, and the chat shows the stage.
- Deleting a project deletes its chats and closes their tabs; the sidebar no longer blocks switching while an answer streams.
- The window wears a custom title bar — drag region, real window controls — and the update chip, titles and suggestions from 0.1.x keep working with it.

## [0.1.10] - 2026-11-21

The model menu grows up: capability filters come back, the thinking bar becomes a thick ember gauge that breathes at Max, and a saved online provider now offers every model it has, not just one. This pass also clears the small stumbles found in a hard audit of the installed 0.1.9. Your chats, notes, models and settings are kept.

### Added
- **Capability filters in the model menu.** Thinks, Vision and Free chips narrow the list at once, with a match count and Clear; your filter choice is kept for next time.
- **The full thinking ladder, per model.** The effort bar now offers minimal → low → medium → high → xhigh → max → ultra, and each model lists only the rungs its family answers to (GPT-OSS keeps low/medium/high, GPT-5 and o-series keep minimal/medium/high, Grok 4 adds xhigh, always-thinking DeepSeek stays fixed, hybrid local models keep Off/On). **Medium is the default** for models that have it.
- **The effort bar is a real gauge now**: a thick rounded capsule with a springy ember fill, the knob riding inside the cap, and at the top rung it breathes with an ember glow and a sweep of light, so Max feels like Max (still, everything respects the system's reduced-motion setting).

### Fixed
- **A manually added online provider now lists all of its models.** Saving a provider asks it for its model list and keeps it, so every model it offers is selectable in the chat's model menu — not just the one typed by hand. (Pressing Load still refreshes.)
- **The chat's delete confirmation opens centred on the window** again instead of as a small card trapped over the sidebar (a transform-side-effect bug).
- **The keyboard shortcuts dialog** shows Windows keys on Windows (no more ⌘ symbols), lists the shortcuts the app really has (Ctrl+F in-chat search, Ctrl+T effort, Ctrl+B sidebar, Ctrl +/−/0 scale), and the "Keyboard shortcuts ?" chip in the status bar is clickable.
- **One name per model.** The message tag no longer shows the raw provider slug, and the provider is named once on the empty chat instead of five times on one screen.
- **Learned starters earn their place**: no bare links, no code fragments, at most two, and the sparkle moved out of the title.
- **Answer statistics read friendlier**: "716 in · 34 out · 1.6 s · first token 1.4 s" — one unit for time.
- **The Documents screen shows one human size** per file and a relevance score only when a search has actually run.
- **Memory's empty state says "No notes yet"** when nothing was searched.
- **Your own messages carry an ember tint in both themes** (the light theme washed them out) and the empty chat's starter cards follow the chat width you chose.

### Changed
- The model menu is wider (24 rem) with the active model tinted ember and full names shown on hover.

## [0.1.9] - 2026-11-21

Reading controls found in the reference apps, redrawn for LocalAide: the chat bends to how you read, Settings answers a search box, and the Memory screen shows its size at a glance. Everything here works without new backend commands, so it is complete on all platforms. Your chats, notes, models and settings are kept.

### Added
- **Reading settings** in the Chat settings sheet:
  - **Chat width** — Comfortable, Wider or Full, kept across restarts (the old experimental wide-chat flag becomes Full).
  - **Follow answers while streaming** — the app already kept the newest text in view; now you can turn that off and steer the scroll yourself. The jump-to-newest button still works.
  - **Collapse long questions** — a long prompt shows its first lines with a **Show full question** button until you expand it.

### Changed
- **Settings has a search box.** Type a word ("voice", "theme", "logs") and the section list narrows to what matches, with a match count. Enter opens the first match, Esc clears it. Sections also answer to related words, not just their titles.
- **The Memory screen shows totals**: how many notes exist and how much room they take, next to the search field.

## [0.1.8] - 2026-11-21

The "Deep Ember" design pass: warm near-black layers instead of flat Windows grey, a cleaner chat, a live run timeline for autonomous work, search inside a chat, and answer statistics for everyone. If you have 0.1.2 or later and the update key is set up, Settings → About → Check for updates installs this version; otherwise download it below. Your chats, notes, models and settings are kept.

### Added
- **Search inside a chat.** The magnifier in the chat header (or Ctrl+F / ⌘F) opens a bar above the conversation: type to count the messages that match, Enter jumps through them with a brief highlight. Esc closes it.
- **Answer statistics for everyone.** A switch in Chat settings shows tokens, time and speed under each answer (it used to be hidden behind developer mode).
- **The run timeline shows pace.** While the assistant works a plan or runs subagents, the status above the chat box shows how long the run has taken and how many tool rounds it has used, and the plan panel shows a progress bar.
- **The empty chat shows what this computer holds**: how many documents are indexed, how many memory notes exist, and how much memory models may use.
- **A browser preview harness** for development (`preview.html` + `npm run dev`): the whole UI runs in a normal browser against a fake backend, so design passes no longer need a compiled app window.

### Changed
- **New look, "Deep Ember".** The palette moves from flat Windows grey to warm near-black layers: a darker sidebar, a raised content layer, cards and popovers one step above, and the ember accent unchanged. Contrast was re-measured for both themes (body text ≥ 15:1 on dark, muted ≥ 7:1, meaningful borders ≥ 3.5:1; the light theme is warm bone paper with the same guarantees).
- **The chat reads lighter**: slimmer header with a quiet offline badge, the sidebar carries a **New chat** button with its shortcut, the composer is one raised card with a soft focus glow and an ember send button, user messages sit in a tinted bubble, the model name shows as a small tag beside the answer, completed steps get a check, sources fold into a compact **Sources [1]** row, and tool cards quietly show their result on the right.
- Starters use ember-tinted icon tiles and hover motion; learned starters carry a small sparkle.
- Buttons that glow, do so through one shared shadow token in both themes.

### Fixed
- The Search screen's library status loads its full shape again in the browser preview (a fixture gap, not an app bug).

## [0.1.7] - 2026-11-21

Models load on every processor now, chats live in the sidebar, and the chat box has plan and build modes, a new model menu, better questions, subagents, charts and mockups. Windows on ARM gets its own installer. If you have 0.1.2 or later and the update key is set up, Settings → About → Check for updates installs this version; otherwise download it below. Your chats, notes, models and settings are kept.

### Fixed
- **"The model did not start … exit code: 0xc000001d".** The installers no longer carry a llama-server built for AVX2 processors. The first time a model loads, LocalAide downloads the official llama.cpp build (b12359) for this computer's processor, about 13 to 25 MB from github.com, with a checked checksum. The x64 Windows and Linux builds pick the fastest instructions the processor has, so processors without AVX2 work too. An x64 copy of the app on Windows on ARM, or under Rosetta on a Mac, still gets the ARM64 engine. Settings → General shows the engine, and Check my setup has a step for it.
- Going to Settings and back no longer starts a new chat: the open chat, its scroll position and an answer that is still coming stay as they were. Settings comes back on the section you left, and other screens keep their scroll position.

### Added
- **Windows on ARM** installer (`_arm64-setup.exe`), with its own update package.
- **Chats in the sidebar**, under Chat. Start a chat in a project folder (the folder button next to Chats) and its file tools and commands work in that folder; chats are grouped by folder, plain chats come after. The whole disk, the home folder itself and LocalAide's own data folder cannot be picked. Hover a chat to delete it.
- **Permission mode as one icon in the chat box:** Plan mode (looks around with read-only tools and writes a plan; **Build** then carries it out), Ask before changes, Autonomous (asks only when there is risk: deleting or moving files, anything outside the folder, publishing, system installs, replacing an existing file, MCP tools) and Full access. Blocked commands never run in any mode.
- **New model menu:** the provider at the top (tap it to switch, add a provider or manage models), that provider's models with a search, and the thinking effort as a slider. Ctrl+T steps through the effort.
- **Questions** come as a card: several questions with Back and Next, options with a short description, Other with your own answer, Dismiss and Submit. If you send a message instead of answering, the card stays and says that Submit sends the answer as a new message.
- **Subagents.** The assistant can hand parts of a task to helpers that only search and read, several at once. Each one shows its current step under its name, and the work status above the chat box lists them all with their steps and reports.
- **Work status** above the chat box: the assistant's plan for longer tasks ("Completed 2/4") and its subagents.
- **Learned skills.** When you show the assistant how you want a kind of task done, it saves or improves a skill on its own and says so in the chat, with **Undo**. Switch it off in Chat settings.
- **Charts, diagrams and mockups** in answers: bar, line and pie charts, Mermaid diagrams, and interface mockups. Mockups run in a sandbox with no internet and no access to your files, and the assistant can let you pick an option on them.
- **Suggested next prompt** after each answer; Tab takes it. **Improve prompt** (the wand next to Send) rewrites your draft with the chat's model, with Undo.
- **Starter prompts learned from your chats.** The empty chat offers the ways your chats most often begin, read on this computer only; the old examples fill the rest.
- **Open the files a chat made:** Open, Open with (your editors, or the system's app chooser) and Show in Finder / File Explorer. Programs and scripts are never opened with their default app. The ⋯ menu opens the chat's folder.
- **Interface scale** from 70% to 250% (Settings → General → Appearance). Ctrl + and Ctrl − change it anywhere; Ctrl 0 resets it.

### Changed
- "How LocalAide answers" moved out of the chat box; it is in Chat settings.
- The macOS disk image has a new background, sharp on Retina screens.
- Shorter screen transitions, hover motion only with a mouse, and a quieter focus ring on the chat box.

## [0.1.6] - 2026-11-21

The assistant can now run commands, search your own folders by meaning, ask you questions with buttons, listen to you, and be shared through a link. Settings shows every call that left your computer. If you have 0.1.2 or later and the update key is set up, Settings → About → Check for updates installs this version; otherwise download it below. Your chats, notes, models and settings are kept.

### Added
- **Terminal commands.** The assistant can run commands in its workspace folder. Read-only commands (ls, cat, git status and similar) run at once; commands that write, delete or install ask first; destructive ones (recursive deletes, disk formatting, piping a download into a shell, administrator rights) never run, and the assistant is told why. Each run has a time limit and an output limit (long output is saved to a file in the workspace), shows in the chat like a terminal, and is in the Activity log. Switch it off in the Tools menu.
- **Approvals switch in the chat header:** Ask every time, Allow for this chat, or Auto-approve all. Blocked commands never run in any mode, and every run is still logged.
- **Questions with buttons.** When the assistant asks you to choose, the options appear as buttons (also Yes / No, several choices, and Other). Keys 1 to 9 pick an option while the chat box is empty. Tool confirmations use the same buttons, for example Run / Allow for this chat / Don't run.
- **Library status** at the top of Search: folders, files, last scan, and whether search works by meaning or by exact words only. Folder presets for Documents, Desktop and Downloads, live progress while a folder is added, and a report of what was skipped and why. Without a meaning (embedding) model the status says so and offers the download; the chosen model is remembered and starts with the app. Questions about your own files search the library first and name the file.
- **Check my setup** (Settings → General): downloads the Tiny model if needed, loads it and gets one reply, with pass or fail and time for each step.
- **Voice** (Settings → Voice, off by default): hold the microphone button in the chat box to talk. Seven local models run on this computer through the sherpa-onnx engine (Whisper tiny, base and small, Moonshine tiny and base, Moonshine base Arabic, Parakeet TDT 0.6B v2), downloaded once with checked checksums. Online models with your own key, marked Live or Batch: Gemini 3.5 Transcribe Live and Transcribe, Deepgram Nova-3, Groq Whisper large-v3-turbo, OpenAI GPT-Transcribe, GPT-4o Transcribe and Whisper, and Soniox v6. **Local only** blocks every network use for voice. **Tidy the text** removes filler words with the local model. Read aloud is now a setting there, with voice and speed.
- **Share my assistant** (Settings → Share): a Cloudflare quick tunnel to a small local server, with a Viewer link (read conversations) and a Remote user link (chat, answer its own approvals, run Check my setup). Each link has its own token, made on this computer and shown once. Requests from a link always ask before tools change anything, and "Sharing is on" shows at the bottom of the window while it runs. cloudflared is downloaded on request, with a checked checksum.
- **What left this computer** (Settings → Online providers): every call to an online service (chat, voice, web search, page reads, model lists, downloads), with time, provider, model, kind and a short preview. Keys are removed before anything is saved.

### Changed
- **Online providers** in Settings: one card per provider with its status (Connected, Not checked, Needs a key, Not reachable), where messages go, the models it offers and a **Test connection** button. Adding or editing a provider is one form: name, address, key, Load models, then pick a model or type it when the list is empty.
- Search results from your library name the file and say when the search matched words only.

## [0.1.5] - 2026-11-20

Polish after 0.1.4: a context meter, slash commands, full-chat export and copy, downloads you can pause and resume, and a fix for chats that sometimes would not save. If you have 0.1.2 or later and the update key is set up, Settings → About → Check for updates installs this version; otherwise download it below. Your chats, notes, models and settings are kept.

### Added
- **Context meter.** A ring beside the model menu fills as the chat uses the model's context window. Click it for the chat, the answers, the instructions, the room kept for the next answer, and what is left. **Compact now** stops sending older messages.
- **Auto-compact** (Chat settings → Context window). When the chat reaches the level you choose (80% by default), or the next answer would not fit, older messages stop being sent to the model. They stay on screen and in the saved chat, behind a line that says so, with **Send them again** to undo.
- **Slash commands.** Type `/` for `/clear`, `/compact`, `/model`, `/settings`, `/export`, `/copy`, `/goal`, `/scale`, `/by-the-way`, `/remember`, `/create-skill`, `/web`, `/notes` and `/tools`, next to your saved skills (which now also match by tag).
- **Copy and export the whole chat.** The ⋯ menu copies the conversation or saves it as Markdown or JSON. Copy and Markdown are the same text: answers, the model's thinking (folded), each tool call with its input and result, attachments, and numbered references.
- **Pause and resume downloads.** Pause keeps what was downloaded; Resume continues where it stopped, also after the app was closed. The model screen shows downloaded and total size, what is left, speed and about how long it will take. Cancel discards the partial download.
- The model filters (Thinks, Sees images, Uses tools, Free) are remembered, with a count and **Clear**.
- **One switch for online models** (Settings → Online providers, and Settings → General). Off keeps every chat on this computer: providers are greyed out as not available, none appear in the model menu, and an online model cannot be picked or sent to. Your providers and keys stay saved.
- **Conversation memory follows the model.** The slider goes up to the context window of the model you use (up to about one million tokens), shows tokens and words, and offers **Reload model** when the loaded model still uses the old size.
- **New experimental features** (Settings → Developer): **Read answers aloud** with your system's voice, **Send with Ctrl+Enter**, and a **Wide chat column**.

### Changed
- **References fold under the answer** as a row of numbers, `[1] [2] [3]`. Click to open the list and click one to open it.
- The chat header always says where answers come from, **Offline · this computer** or **Online · provider**, in the app's orange. Warnings and the online-providers notice also use orange; yellow was hard to read in the light theme.
- The chat window stays put; only the conversation scrolls.
- Download progress for vision models now counts the model and its image projector together, so the size and time left are for the whole download.
- A starred model appears once in the model menu, under Starred.
- Every switch in Settings looks and moves the same way, including developer features, modules and MCP servers, which used checkboxes before. Sliders have one style with an orange filled track in both themes.
- **Use this** is gone from the provider list; pick the model in the chat's model menu instead.
- Web search lives only in the chat box's **Tools** menu (and `/web`), not as a second button.
- The buttons in the chat box are smaller, so the box takes less room.
- Model download sizes use GB, like the memory they need, and include companion files.

### Fixed
- **"The chat could not be saved: invalid type: floating point".** How long the model thought was saved as a fraction of a millisecond, which the chat file did not accept, so some chats with thinking failed to save. Times are now whole numbers, and older values with fractions load too.
- A quick Resume after Pause could start a second download into the same file and break it. Only one download per model runs at a time, and Resume waits until Pause has finished.
- Pause and Cancel now act within a fraction of a second, also while the connection is silent (it could take up to a minute).
- Pausing a community model and then refreshing showed Download instead of Resume.
- The General screen's online switch drew its knob outside the track when on.
- With more than eight slash commands listed, the arrow keys moved to commands that were not shown.

## [0.1.4] - 2026-11-19

Tools the model chooses for itself, thinking you can open and read, one model menu for every provider, file attachments, and MCP servers. If you have 0.1.2 or later and the update key is set up, Settings → About → Check for updates installs this version; otherwise download it below. Your chats, notes, models and settings are kept.

### Added
- **The model decides when to use tools.** Web search, page reading, your notes and documents, saved skills and workspace files are now tools. A simple question gets a direct answer; a question about today's news gets a search. Each tool call shows as a card under the answer with its input and result. The **Tools** button in the message box switches each tool on or off for the chat, and **Web** still decides whether the web may be used at all.
- **Ask before change.** Writing a file, saving a note and MCP tools wait for you in the chat: Allow once, Allow for this chat, or Don't allow. Every tool call is in the activity log.
- **Thinking.** Models that think (Qwen3, DeepSeek R1, gpt-oss, Gemini 2.5 and others) show a "Thinking…" line while they think and "Thought for 12 s" after. Click it to read the thinking. A **Thinking** button sets the effort (Off, Low, Medium, High, or what the model offers) for each model.
- **One model menu for everything.** The model button in the message box lists the models on this computer and the models of every online provider you switched on, with icons for thinking, images and tools, the context size, and a Free tag. Search the list, filter it, star favourites, and refresh a provider's list. Ctrl/⌘+Shift+M opens it. Settings → Online providers has an **In model menu** switch per provider, so any number can be on at once.
- **Attach files.** The + button attaches text, code, Markdown, CSV, JSON and PDF files (and images); you can also drop files on the chat or paste images. The model reads the file text with your message, and saved chats keep it.
- **MCP servers** (Settings → Tools and MCP). Add programs that run on this computer (stdio) or web addresses (Streamable HTTP), with environment variables or headers for tokens. Test a server to see its tools, mark it to run without asking, or paste a config from Claude Desktop, Cursor, VS Code, OpenCode or Crush. Ready-made entries for Context7, DeepWiki, Exa, Microsoft Learn, Hugging Face, GitHub, Files, Fetch, Git, Time, Memory, Sequential thinking and Playwright.
- Answers show which model wrote them, and the model knows today's date and your time zone.

### Changed
- The model menu moved from the top bar into the message box; the top bar shows an **Online** badge while an online model answers.
- The default "Longest answer" is 2,048 tokens (was 512). Thinking gets its own room on top, so it no longer uses up the whole answer.
- Online providers refuse fewer requests: when a provider rejects an optional setting (thinking effort, token limit name, temperature, images), LocalAide sends the message again without it. A model that cannot use tools gets web results and notes gathered for it, as before.
- OpenAI's API gets `max_completion_tokens` and no temperature for reasoning models, which it requires.
- Autonomous mode allows up to 16 tool rounds.

### Fixed
- Thinking models looked stuck: their thinking was thrown away, so nothing showed until the answer began.
- Failed answers ("I could not answer that") were sent back to the model as history.
- Citation cards no longer use a thick coloured left border.
- Backups now leave out MCP tokens unless you include keys.

## [0.1.3] - 2026-11-19

A new chat, chat modes, developer mode, web search that works without keys, and free providers from Kilo Code and OpenCode. If you have 0.1.2 and the update key is set up, Settings → About → Check for updates installs this version; otherwise download it below. Your chats, notes, models and settings are kept.

### Added
- **New chat design.** A full-height conversation with a centred reading column, answers without bubbles, a rounded message box that grows as you type, a round send button, a typing indicator, and source cards under answers. Saved chats open in a side panel with search, grouped by Today, Yesterday, This week and Older.
- **Chat modes.** Pick how LocalAide answers from the message box: **Standard**; **Strict** (careful, cites sources, says when it is unsure, lower temperature); **Unrestricted** (direct answers without lectures or needless refusals; the model's own rules and the law still apply); **Autonomous** (plans, gathers sources from the web and your files, works through the task, then checks the result). Each answer shows what LocalAide did first, such as "Searching the web" and "Reading rust-lang.org".
- **Web in chat.** A **Web** switch in the message box searches the web for your message and cites the results. A **Notes** switch decides whether chat looks in your notes and documents.
- **Developer mode** (Settings → Developer): edit the system prompt and see each mode's instruction, sampling controls, answer statistics (tokens, time, speed), a plain-text view of answers, a request log with timings, a switch to turn off animations, and a compact layout.
- **Modules** (Settings → Developer): switch off Notes and file recall, the Search screen, Web in chat, Skills, Online providers, the OpenCode link or the command palette. Switched-off parts disappear from the app.
- **Collapsible sidebar.** Shrink the sidebar to icons with the button at its bottom or Ctrl/⌘+B. LocalAide remembers your choice.
- **Kilo Code** as a free online provider with no key or sign-up (`kilo-auto/free` picks a free model for you).
- **OpenCode.** OpenCode Zen's free models as an online provider (its shared "public" key is filled in). Settings → Integrations links an OpenCode server on this computer (`opencode serve`); the code button under any message then starts an OpenCode session with that task. OpenChamber and other OpenCode front ends see the same session.
- **Skill descriptions.** Discover now shows what each skill in a collection does, read from its SKILL.md, and search matches descriptions too.
- The UI UX Pro Max design skill (MIT) in `.kiro/skills/ui-ux-pro-max/`, used for this redesign.

### Changed
- **Web search works without keys.** Exa runs keyless through its public endpoint, and Firecrawl through its free keyless tier; a key only raises the limits. Firecrawl can also use a server you run yourself (for example `http://localhost:3002`). The new default, **Automatic**, tries DuckDuckGo, then Exa, then Wikipedia, and each result says which one found it. Reading a page falls back from Jina Reader to Exa.
- Softer, more modern corners, larger click targets, a faint warm light behind each screen, and thin scrollbars.
- The Privacy Policy and Terms of Use describe the new network uses (Web in chat, OpenCode, skill descriptions) and the chat modes.

### Fixed
- **Web search found nothing.** DuckDuckGo's results page writes `href` before `class` and uses single quotes, and the reader expected the opposite, so every search came back empty. It now reads attributes in any order and quote style, and the fixture is the real page layout.
- Releases attach only the installers; 0.1.2's draft also picked up the `llama-server` build.

## [0.1.2] - 2026-11-18

Installs, updates and polish, from testing 0.1.1 on an Intel Mac and on Windows 11. This is the first version that can update itself, so people on 0.1.0 or 0.1.1 install it by hand once. On Windows the new installer replaces the old version; on macOS drag the new app over the old one. Your chats, notes, models and settings in `~/.aiassistant/` are kept.

### Added
- **In-app updates.** Settings → About → "Check for updates" shows the new version and what changed. "Install and restart" downloads it, checks its signature, installs it and keeps your data. Nothing is checked when the app starts.
- **Privacy Policy and Terms of Use.** Readable in Settings → About → Legal, the command palette and the setup screen. The Windows installer shows the terms as its license page.
- **Discover skills.** The Skills screen can browse and add skills from Anthropic Skills, GitHub Awesome Copilot, prompts.chat (about 2,600 prompts), or any public GitHub repository with a `skills` folder. Each added skill keeps its source and license.
- **Free and local online providers.** Pollinations works with no key. 9Router, OmniRoute, LM Studio and Ollama run on your own computer. Google Gemini, OpenRouter, Groq, Cerebras, Hugging Face, NVIDIA, Mistral, LLM7 and Chutes have free keys; OpenAI, DeepSeek and Together are paid. "Load models" shows what a provider offers today and marks the free models.
- **Hugging Face browsing.** Community models open on what is trending, and can be sorted by downloads, likes or newest. "Unfiltered only" shows uncensored and abliterated models, with a warning.
- **Command palette actions.** Ctrl/⌘+K can start a new chat, switch the theme, open online providers, check for updates and open the legal pages. Arrow keys and Enter work in the list.

### Changed
- **Icons and installers.** The app and installer icons are the LocalAide ghost; they were the default Tauri icons. The Windows installer has a branded header and side panel and installs for the current user, without an administrator prompt. The macOS disk image has a branded background.
- **Motion.** Screens, settings sections, setup steps and tabs animate. Lists appear one item after another, cards lift on hover and loading shows placeholders. All of it turns off when the system asks for reduced motion.
- **Fuller screens.** Chat, Skills and Setup have proper starting screens. Setup shows which step you are on.
- An API key is no longer required to add an online provider. A provider that needs one says so in plain words.
- Dependency updates: reqwest 0.13 (HTTPS now uses rustls with the operating system's certificate store instead of native-tls), sha2 0.11, rusqlite 0.42, getrandom 0.4, tauri-plugin-updater 2.14 (new), four smaller Rust updates, and six GitHub Actions updates. Stored file hashes and the catalog digest are unchanged.

### Fixed
- Search boxes no longer show a second box inside them when focused, or two clear buttons. Password fields no longer show Windows' extra reveal and clear buttons, and dropdowns show one arrow.
- The `llama-server` engine is built without llama.cpp's web chat page. That page is downloaded at build time and its files disappeared, which broke every build on 2026-11-18. LocalAide never used it.

### Release
- Each GitHub Release now uses its section of this changelog as its notes, and the release job fails if the section is missing.

## [0.1.1] - 2026-11-15

Merges the `design/windows-theme` work (UI redesign and production-hardening Phases 0 to 5, audit in `docs/evidence/audit-2026-11-14.md`). A `[build-all]` CI run on `6bd9194` passed every job, including all four `llama-server` builds and the macOS, Windows and Linux installers.

### Added
- UI redesign: shared components (buttons, tabs, page headers, settings rows, confirm dialogs, focus trap, toasts), plain copy, Ctrl+1 to Ctrl+6 screen shortcuts, native folder and file pickers, and chat export through a save window. Mockups live in `docs/design/mockups/`.
- Release pipeline: tag builds create a draft GitHub Release with installers, `SHA256SUMS.txt` and an SPDX SBOM; a version gate checks `package.json`, `Cargo.toml`, `tauri.conf.json` and the tag agree; portable `llama-server` builds (no native CPU flags, no OpenMP, static MSVC runtime) must start in CI; optional code signing through repository secrets. See `docs/release.md`.
- Third-party notices and a `cargo-about` license file ship inside the app; Dependabot and a weekly dependency audit.
- A Windows Rust job (clippy and tests) and readable CI failure annotations.
- Web search and page reading (ADR-0011): the Search screen's Web mode queries the open internet — DuckDuckGo (lite results page), Wikipedia REST search, and the Jina Reader work keylessly; Exa and Firecrawl run after the user pastes an API key in Settings → Web search. "Read page" pulls a result's content as markdown (Firecrawl when keyed, the keyless Jina Reader otherwise). Every request is an explicit click; keys live in the local config, travel only to their own provider, and are excluded from backups unless included. HTTP-level tests exercise every provider against loopback servers.
- Online-provider presets from the open 9Router registry (ADR-0011): the Settings provider form can prefill from LLM7, Synthetic, api.airforce, Groq, Cerebras, NVIDIA NIM, OpenRouter, Chutes, Mistral, or the user's own local Ollama, each with a note and a key link. Free tiers are labeled as changing without notice; loopback proxies may be saved without a key.
- Light theme: the ink ramp inverts to bone paper with dark ink text, accent text roles darken to stay readable, and the choice persists in config with a Settings toggle. Dark remains the designed default; the light theme has not yet been eyeballed on a real screen.
- First-run wizard community step: besides the RAM presets, the wizard can search public Hugging Face GGUF repositories, expand a repo's files, and install one through the same hash-verified download pipeline.
- Backup stance confirmed: the `.labak` archive is a portable file the user can upload anywhere; built-in passphrase-verified restore was already shipped and stays the only restore path.

### Changed
- Responsiveness: heavy commands run on worker threads, the model loads after the window opens, one load at a time, and Stop works before the first token.
- Library indexing skips unchanged files, streams vectors, reads UTF-16/Windows-1252 text and PDFs, caps file sizes, depth and counts, and refuses symlinks and junctions. Deleted folders leave the library.
- Downloads have timeouts, a free-space check, off-thread checksums and HTTP 416 recovery.
- Backup format v2 (Argon2id 64 MiB / 3 passes in an authenticated header); v1 archives still open.
- Rust toolchain pinned to 1.100.0 in both `rust-toolchain.toml` and CI.
- Meaningful borders moved from the `ink-800`/`ink-700` tokens (1.2:1 / 1.9:1 — both below the 3:1 non-text floor) to a dedicated `edge` ramp sized to clear 3:1 in both themes; text on ember fills now uses the semantic `on-ember` token that flips with the theme. Arithmetic recorded in `docs/evidence/a11y.md`; on-screen re-measure still open.

### Fixed
- Engines stop on exit, and child processes join a kill-on-close job object on Windows, so no `llama-server` is left running. Single-instance lock and a minimum window size.
- Download Cancel now matches the job it targets.
- Path checks on model ids, note paths, thread ids, backup entries and companion file names.
- Using a model loads it; downloads no longer mark a model active; nothing contacts GitHub at launch.

### Security
- Cloud chat follows no redirects, needs the key re-entered when a provider's host changes, and only `cloud_mode_set` turns online chat on. Local-network pages are never sent to page readers.
- Stricter CSP, links open in the browser for http/https only, random provider ids, engine key passed through `LLAMA_API_KEY`, folder approvals expire after 30 minutes, and refused tool actions are logged.

### Removed
- The dev-only Developer config screen, unused commands (`config_save`, `config_compute_ram`, `model_existing_size`, `chat_thread_export`), the unused prompt builder, `OPTIMIZATIONS.md`, `PACKAGING.md` and the lock-update workflow. Old config and state files still load.

## [0.1.0] - 2026-11-14

First published release of LocalAide, tagged `v0.1.0`. Every platform stage — llama-server sidecar builds and app bundles (`.dmg` for Apple Silicon and Intel, `.msi`/NSIS for Windows, `.deb`/`.AppImage` for Linux) — completed green in CI on the tagged commit, and the installers are attached to the GitHub Release.

### Added
- llama-server sidecar engine (ADR-0007): chat runs in a llama.cpp `llama-server` child process on `127.0.0.1`, built from the pinned llama.cpp tag v1.0.0 by CI (`.github/workflows/llama-server.yml`) and bundled via the `tauri.sidecar.conf.json` overlay.
- Image input for vision models: attach up to 4 images; the sidecar loads the model's `mmproj` projector and streams image content parts to the server.
- Opt-in Online Providers mode: `RouterEngine` routes between the local sidecar and a `RemoteEngine` for OpenAI-compatible providers configured in Settings with the user's own API key. Base URLs must be HTTPS; plain HTTP is accepted only for loopback proxies. Off by default, and a cloud badge in the chat header shows while it is active.
- Kiroween 2026 work merged to `main` (`bb0d0aa`): spec-driven engine upgrade and design pass (`.kiro/specs/kiroween-2026/`), a 36-model catalog verified against Hugging Face, the folder-indexing fix, and the stitched-ghost design tokens.
- Full platform verification (2026-11-14): a `[build-all]` run completed every platform stage for the first time — macOS Apple Silicon and Intel sidecars built on `macos-latest` (the retired `macos-14` label cancelled jobs at queue time) and bundle artifacts produced for macOS (`.dmg`, both architectures), Windows (`.msi`/NSIS), and Linux (`.deb`/`.AppImage`).
- Community models from the Hugging Face hub (ADR-0010): a search box on the Models screen lists public GGUF repositories (downloads, likes, license, gated badge), shows each repo's pinned GGUF files, and registers any file as a downloadable model — split GGUFs carry their shards, vision projectors ride along, and every download is verified against the hash the hub serves. Community models are badged as such; the weights and licenses are not vetted by LocalAide. A model picker in the chat header switches between installed models (loading now resolves through the filesystem, which also makes freshly downloaded and split-shard models loadable).
- Model catalog refresh (ADR-0008): an explicit "Refresh catalog" action fetches `catalog/models.json` from this repository over a pinned HTTPS URL, validates it (integrity digest, structural checks, version anti-rollback) before swapping it in, caches it next to `config.json`, and offers a Reset that restores the built-in list. A parity test keeps the JSON and the compile-time list identical.
- Catalog v8 (2026-11-14, operator review): the newest verified models per role — Spark-X2.6 4B and 1.7B (October 2026 on-device agent models), GLM-5.3 Flash (320B-total/18B-active MoE with vision), Mistral Small 4 (119B MoE, native MXFP4, vision), and Nomic Embed v2 MoE. Split-GGUF support: a catalog entry can now name its primary file and carry shard companions, so multi-shard llama.cpp models download and load end to end; the pin verifier checks shard totals.
- Companion downloads fixed: vision projector (`mmproj`) files now download with their model, and the Models screen shows a Vision badge plus kind filters (Chat/Code/Embedding/Reranker). Deleting a model also deletes its companions.
- Semantic and hybrid retrieval: a local embedding engine (a second `llama-server --embedding` instance) vectorizes library chunks at ingest and powers Semantic/Hybrid search modes with reciprocal-rank fusion; memory recall and chat RAG embed queries too. Includes a one-click backfill for previously indexed chunks and a truthful `embedding_status`.
- Reranker second stage: a third `llama-server --rerank` instance re-orders the top fused search candidates by true query-document relevance through `/v1/rerank` whenever its engine is running — on the Search screen, memory search, and chat RAG alike. A rerank failure degrades to the fused ranking instead of failing the search; the reranker model (the catalog's Qwen3 Reranker 0.6B, or a community one) is started from a chip on the Search screen and shown in diagnostics.
- Skills: reusable prompt templates as local markdown files (`~/.aiassistant/skills/`) with a full editor screen; typing `/` in chat opens a skill picker. Custom prompts reused three or more times surface a local "save as skill?" suggestion — heuristics only, nothing leaves the machine.
- Persisted chat preferences: system prompt, sampling temperature, response cap, and persona survive restarts, with a context-window slider in Settings.
- Engine context is now a setting (default 8192 tokens) and is capped per model by the catalog's native context hint, replacing the hard-wired 2048.
- Diagnostics: a Settings → Diagnostics tab with local subsystem health, a llama-server log viewer, and a display-only update check against GitHub Releases (the app never downloads anything by itself).
- Encrypted local backups (ADR-0009): one AES-256-GCM archive (`.labak`) of config, memory notes, and conversations under an argon2id-derived key from a passphrase that is never stored. Local-folder targets ship working (including desktop-synced OneDrive/Drive folders, with zero OAuth — the portable archive can be uploaded anywhere the user chooses); provider API keys are excluded by default; restore verifies the passphrase before touching any file and never clobbers a newer config.

### Changed
- Toolchain currency pass: Node 20 → 24 (removed from GitHub runners 2026-09-23), GitHub Actions checkout/setup-node v4 → v5, Vite 6 → 8, vite-plugin-svelte 5 → 7, vite-plugin-checker 0.8 → 1.0, svelte-check 4.1 → 4.7, TypeScript → 5.9, notify 6 → 8, and the llama.cpp sidecar pin v0.9.0 → v1.0.0 (the semver milestone). Unused `autoprefixer`/`postcss` devDependencies were removed and an `engines` field pins Node ≥ 24.
- Platform builds (sidecar, macOS/Windows/Linux bundles) run only for release tags, manual dispatch, or commits marked `[build-all]` — routine pushes verify with the quality gate alone, keeping pushes fast.
- Documentation truth passes: steering docs, ADRs, README, PACKAGING, scope, and evidence files aligned with the shipped code (chunk sizes, search honesty, catalog digest, sidecar build steps).

### Removed
- Dead code: the in-process llama_cpp_rs engine and its vendored fork, the legacy tantivy/sled indexer, the old top-level watcher/embeddings/voice/components modules, and the obsolete sandbox path (the jail uses `std::fs::canonicalize` plus lexical checks).

## Earlier milestones

Recorded here because these milestones were never published as releases under the version headings they originally carried.

- **2026-10-30 planning pass** (originally headed `0.2.0-alpha`): scope contract and backlog (`docs/scope.md`, `docs/backlog.md`), ADR-0001 through ADR-0006, the rubric scorecard, and the PR template.
- **Initial prototype** (originally headed `0.1.0`): Tauri v2 + Svelte 5 + Tailwind scaffold, a model-download scaffold, and a Tantivy/Sled search prototype, since replaced by SQLite FTS5 (ADR-0002).
