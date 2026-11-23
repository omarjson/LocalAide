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
