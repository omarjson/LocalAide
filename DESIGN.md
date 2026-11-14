# LocalAide design

Status: direction "At Home on Windows" applied on 2026-11-14 (mockup: `docs/design/mockups/e-native.html`). The built frontend has been checked in a headless browser preview, with a fake backend, in both themes on the Chat, Search, Memory, Skills, Models and Settings screens. The eight-step UI pass is complete. Not yet checked inside the real Tauri window.

## Surface and mode

- Surface: a desktop app for chat, search, memory, and model management.
- Mode: Operate. Scanning, consistency, and clear controls come first.
- Direction: the app runs on this computer, so it looks like part of it. Windows 11 manners: neutral Mica greys, the system UI font, small corners, a nav pane with a selection pill, a status bar. On macOS the same font stack falls back to the system font.
- The brand lives in one place: the ghost's ember eye. Ember marks the selected nav item, the live dot, and the main action. Everything else stays quiet.

## Logo

- `public/logo.svg`: a stitched ghost with one plain eye and one ember eye, plus a patch, on a dark rounded tile. The ember eye stands for the local model.
- `src/components/LogoMark.svelte` draws the same tile, so the cream ghost reads on the light theme too.

## Layout shell (`src/App.svelte`)

- Nav pane, 256px, on the page colour. Items are an icon plus a label, 40px tall. The selected item gets a raised fill, semibold text, and a 3px ember pill on the left, plus `aria-current="page"`.
- Settings sits at the bottom of the pane, Windows style. Update notices sit just above it.
- Content sits on a raised layer (`ink-900`) with a hairline and an 8px top-left corner.
- Status bar along the bottom: free computer memory, the model memory limit, and a pointer to the `?` shortcut help.

## Palette

All tokens live in `src/main.css` under `@theme`; the light theme re-declares them under `:root[data-theme="light"]`, so every utility flips automatically.

| Role | Token | Dark | Light |
|---|---|---|---|
| Page (Mica) | `ink-950` | #202020 | #eef0f3 |
| Content layer, inputs | `ink-900` | #2b2b2b | #fbfbfc |
| Selected and raised fills | `ink-800` / `ink-700` | #383838 / #454545 | #e3e6ea / #d2d6dc |
| Meaningful borders | `edge` | #7a7a7a (3.8:1) | #80858d (3.3:1) |
| Muted text | `ink-400` | #ababab (7.1:1) | #5b5f66 (5.6:1) |
| Placeholders | `ink-500` / `ink-600` | #9c9c9c / #969696 | #5f636a / #64686f |
| Body text | `ink-100` | #f3f3f3 | #1b1b1f |
| Accent | `ember-500` | #ff7a1a | #c2540a |
| Button fill | `ember-600` | #e8680c | #b54a00 |
| Accent text | `ember-200/300` | #ffd0a3 / #ffb173 | #9a3f00 |
| Overlay tint | `veil` | white | black |
| Focus ring | `ink-50` | white | near-black |

Contrast figures are WCAG 2.x ratios against the page colour.

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

- The built frontend has not been checked inside the real Tauri window yet.
- The Search screen cannot show a folder count because the backend only returns document count and storage size.
- The Settings screen estimates words from the token limit; the model's real tokenizer can differ.
- The Dev Config screen is out of the sidebar. Open it from the command palette (Ctrl/Cmd+K) as "Developer config".

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
