# LocalAide design

Status: direction "Deep Ember" applied on 2026-11-21 (mockup: `docs/design/mockups/f-deep-ember.html`). It supersedes "At Home on Windows" (`docs/design/mockups/e-native.html`, 2026-11-14). The redesigned frontend has been checked in the browser preview harness (`preview.html`, fake backend) in both themes on the Chat (empty and conversation, with the in-chat search bar), Search, Settings and Models screens. Verified with `svelte-check` (0 findings) and the production build; not yet checked inside the real Tauri window.

## Surface and mode

- Surface: a desktop app for chat, search, memory, and model management.
- Mode: Operate. Scanning, consistency, and clear controls come first.
- Direction: warm near-black layers. The window reads as one quiet, warm space in three depths — the darkest chrome (sidebar, status bar), the content layer, and raised cards (composer, popovers, tool cards) one step above with soft elevation shadows. The system UI font stays; corners stay small. On macOS the same font stack falls back to the system font.
- The brand lives in one place: the ghost's ember eye. Ember marks the selected nav item, the live dot, the send button and the main action. Everything else stays quiet.

## Logo

- `public/logo.svg`: a stitched ghost with one plain eye and one ember eye, plus a patch, on a dark rounded tile. The ember eye stands for the local model.
- `src/components/LogoMark.svelte` draws the same tile, so the cream ghost reads on the light theme too.

## Layout shell (`src/App.svelte`)

- Nav pane, 248px, on the darkest chrome. Under the wordmark sits a **New chat** button in a soft ember tint with its shortcut. Items are an icon plus a label, 36-40px tall. The selected item gets a raised fill, semibold text, and a 3px ember pill on the left, plus `aria-current="page"`.
- Settings sits at the bottom of the pane. Update notices sit just above it.
- Content sits on the content layer (`ink-900`) with a hairline and an 8px top-left corner, warmed by a faint ember glow from the top (`.app-canvas`).
- Status bar along the bottom: a green dot with free computer memory, the model memory limit, and a `?` kbd chip for the shortcut help.

## Palette

All tokens live in `src/main.css` under `@theme`; the light theme re-declares them under `:root[data-theme="light"]`, so every utility flips automatically.

| Role | Token | Dark | Light |
|---|---|---|---|
| Chrome (sidebar, status bar) | `ink-950` | #14100c | #ece9e3 |
| Content layer, inputs | `ink-900` | #1e1914 | #f7f5f0 |
| Raised fills, hover | `ink-800` / `ink-700` | #2e261e / #3d342a | #e3dfd7 / #d3cec4 |
| Meaningful borders | `edge` | #7c6f5e (3.9:1) | #767066 (4.1:1) |
| Muted text | `ink-400` | #b3a696 (7.9:1) | #5c554b (6.1:1) |
| Placeholders | `ink-500` / `ink-600` | #a2937f / #93836f | #6d675d / #6e675c |
| Body text | `ink-100` | #f3ede3 | #201b15 |
| Accent | `ember-500` | #ff7a1a | #c2540a |
| Button fill | `ember-600` | #e8680c | #b54a00 |
| Accent text | `ember-200/300` | #ffd0a3 / #ffb173 | #9a3f00 |
| Overlay tint | `veil` | white | black |
| Focus ring | `ink-50` | warm white | near-black |

Contrast figures are WCAG 2.x ratios against the page colour, re-measured 2026-11-21 (script checked: body ≥ 15:1 dark / ≥ 14:1 light, muted ≥ 7.3:1 dark / ≥ 6.1:1 light, placeholders ≥ 4.75:1 both themes, `edge` ≥ 3.5:1).

Rules:

- Text on an ember fill uses `on-ember` (dark ink in dark, white in light). White on light `ember-600` is 5.3:1.
- Hover and quiet fills use `bg-veil/5` or `bg-veil/10`, never `bg-white/N`, which disappears on the light theme.
- Meaningful borders use `edge`; decorative hairlines use `ink-700` or `ink-800`.
- Body text and placeholders meet 4.5:1 on both the page and the content layer.
- Status colours (emerald, rose, amber) darken in the light theme.

## Type and shape

- Font: `Segoe UI Variable Text`, then `Segoe UI`, then the system font. It ships with Windows, so nothing has to be bundled and the app stays offline.
- Base size 14px. Smallest size is `text-xs`, set to 13px in `@theme`. Do not use `text-[10px]` or `text-[11px]`.
- Corners: `rounded-md` 4px, `rounded-lg` 6px, `rounded-xl` 8px, `rounded-2xl` 10px (overridden in `@theme`).

## Rules from the craft floor

- No eyebrow labels above headings.
- No gradient text. Emphasis comes from weight or size.
- Selection, caret, scrollbars, and focus rings use the palette.
- Shadows have an offset and a soft blur.
- Icons come from `src/components/Icon.svelte`, one stroke set. No emoji or Unicode glyphs as icons.

## Known gaps

- The Deep Ember pass has not been checked inside the real Tauri window yet (it was verified in the browser preview harness, both themes).
- The Search screen cannot show a folder count because the backend only returns document count and storage size.
- The Settings screen estimates words from the token limit; the model's real tokenizer can differ.
- The Dev Config screen is gone (2026-11-15). Memory for models is set in Settings → General, and the Models screen and setup flag models that may not fit.

## Shared screen kit

- `src/components/PageHeader.svelte` keeps page titles and actions aligned.
- `src/components/Button.svelte` keeps primary, secondary, quiet and danger actions consistent.
- `src/components/Tabs.svelte` handles the small segmented choices used by Search and Settings.
- `src/components/TextField.svelte` keeps labels, inputs and focus states consistent.
- `src/components/SettingsGroup.svelte`, `SettingsRow.svelte` and `StatusBadge.svelte` make settings and model rows scan quickly.
- `src/components/EmptyState.svelte` gives empty screens one clear next step.
- `src/components/ErrorBanner.svelte` shows a short error, Retry and dismiss actions.
- `src/components/ConfirmDialog.svelte` replaces native delete confirmations across the app.
- `src/components/FocusTrap.svelte` keeps keyboard focus inside open dialogs.

All screens now use the same header pattern. Advanced controls stay behind clear disclosures or More menus. Code font is limited to code, paths, IDs and keyboard keys. Screen shortcuts follow the menu order: Ctrl/Cmd+1 through Ctrl/Cmd+6.
