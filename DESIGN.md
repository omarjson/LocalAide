# LocalAide design

Status: direction "GOAT" applied 2026-11-27. LocalAide now takes its look and feel from [github.com/goat-si/goat](https://github.com/goat-si/goat): neutral near-black layers, an ink primary, one blue accent, hairlines, 8px controls and a glass composer. It supersedes "Deep Ember" (2026-11-21) and "At Home on Windows" (2026-11-14). The palette, sidebar, composer, buttons and empty state were migrated, and the 2026-11-27 sweep finished the job: every decorative ember reference became ink (selected chips, card tints, glows, primary fills, body text), leaving ember-blue only on links, focus, live state, information and data. Verified in the real Electron window on this machine.

## Surface and mode

- Surface: a desktop app for chat, search, memory, and model management.
- Mode: Operate. Scanning, consistency, and clear controls come first.
- Direction: neutral near-black layers. The window reads as one quiet space in three depths — the darkest chrome (sidebar, title bar, status bar), the content canvas, and raised cards (composer, popovers, tool cards) one step above with soft elevation shadows. No warm cast anywhere.
- The brand lives in one place: the ghost's blue eye. Filled primary actions are **ink** (white in dark, near-black in light); blue is reserved for links, focus, live state and information. Everything else stays quiet.
- Motion is a colour change or a short fade. No springy icon transforms, no perpetual loops outside real progress.

## Logo

- `public/logo.svg`: a stitched ghost with one plain eye and one blue eye, plus a patch, on a `#181818` rounded tile. The blue eye stands for the local model.
- `src/components/LogoMark.svelte` is the chrome mark: a monochrome ghost in `currentColor`, one eye punched out and one in `ember-500` (blue). It sits beside the `LOCALAIDE` wordmark at the top of the sidebar, GOAT style.
- `src/components/GhostMascot.svelte` is the same ghost at setup size, drawn in theme tokens so it reads on both themes.

## First-run setup (`src/screens/SetupWizard.svelte`, 0.2.4)

- The patched ghost (`src/components/GhostMascot.svelte`, the logo ghost without its tile, outlined for the light theme) leads each step from a speech bubble. Moods change only its eyes: hello, looking (checking the computer), working (downloading), done.
- One recommended path, Tiny, with one ink button. Fit is said in words (Fits easily, Fits, Too big for this computer), never in GB. Other sizes, online providers and community models wait behind one "More options" disclosure, which opens by itself when nothing fits.
- Motion: the ghost floats only while a download runs; the done check pops once with one blue ring. Both move HTML elements with transform/opacity, never SVG children (the 0.2.2 heat fix), and stop under reduced motion.
- The Stitch `taste-design` checklist was applied by hand (no Stitch MCP in Kiro Web); the two items not taken (perpetual micro-loops, Geist/Satoshi fonts) and why are in `.kiro/specs/polish-0-2-4/design.md`.

## Layout shell (`src/App.svelte`)

- Nav pane, 256px, on the darkest chrome. The top row is the ghost glyph plus a 13px bold `LOCALAIDE` wordmark in `tracking-[0.04em]`. Under it sits a quiet **New chat** row (`h-8 rounded-md`, `hover:bg-veil/5`) with its shortcut, not a tinted CTA.
- Items are an icon plus a label, `h-9 rounded-md gap-2.5 px-2.5`. The selected item is a soft `bg-veil/6` fill with `font-medium` text, plus `aria-current="page"`. No left pill.
- Settings and Collapse sit at the bottom of the pane behind a hairline. Update notices sit just above them.
- Content sits flat on `ink-900` with a `border-l border-ink-800` hairline. No glow, no rounded corner.
- Status bar along the bottom: a green dot with free computer memory, the model memory limit, and a `?` kbd chip for the shortcut help.

## Palette

All tokens live in `src/main.css` under `@theme`; the light theme re-declares them under `:root[data-theme="light"]`, so every utility flips automatically.

Values mirror GOAT's `apps/web/src/index.css`.

| Role | Token | Dark | Light |
|---|---|---|---|
| Chrome (sidebar, title bar, status bar) | `ink-950` | #141414 | #f7f7f7 |
| Canvas | `ink-900` | #181818 | #ffffff |
| Raised fills, cards, composer base | `ink-800` | #212121 | #f3f3f3 |
| Meaningful borders | `edge` | #5c5c5c (3.1:1) | #767676 (4.6:1) |
| Hairlines | `ink-700` / `ink-800` | #2d2d2d / #212121 | #e4e4e4 / #f3f3f3 |
| Muted text | `ink-400` | #afafaf (7.4:1) | #5d5d5d (7.0:1) |
| Placeholders | `ink-500` / `ink-600` | #8f8f8f / #868686 | #6b6b6b / #6e6e6e |
| Body text | `ink-100` | #ececec | #0d0d0d |
| Accent (links, focus, live, info) | `ember-500` | #339cff | #0169cc |
| Primary fill | `ink-100` | #ececec | #0d0d0d |
| Text on primary | `on-ember` | #0d0d0d | #ffffff |
| Overlay tint | `veil` | white | black |

Glass tokens: `--glass-blur: 16px`, `--glass-saturation: 1.08`, `--glass-opacity: 92%`. `.surface-glass` is the utility (backdrop blur with an opaque fallback under `@supports`). `.btn-ink` is the ink-filled primary.

The composer glass uses `--glass-base`: `ink-800` (#212121) in dark, `ink-900` (#ffffff) in light, so the light composer reads as a white field held by its hairline, the way GOAT draws it. Its elevation is `--shadow-composer` (dark: `0 14px 32px -18px rgb(0 0 0 / 75%)`, light: `0 1px 2px rgb(0 0 0 / 5%)`).

Contrast figures are WCAG 2.x ratios against the page colour, re-measured 2026-11-21 (script checked: body ≥ 15:1 dark / ≥ 14:1 light, muted ≥ 7.3:1 dark / ≥ 6.1:1 light, placeholders ≥ 4.75:1 both themes, `edge` ≥ 3.5:1).

Rules:

- Text on an ink primary uses `on-ember` (dark ink in dark, white in light).
- Blue never fills a primary button; it marks links, focus rings, live dots and info banners.
- Hover and quiet fills use `bg-veil/5` or `bg-veil/10`, never `bg-white/N`, which disappears on the light theme.
- Meaningful borders use `edge`; decorative hairlines use `ink-700` or `ink-800`.
- Body text and placeholders meet 4.5:1 on both the page and the content layer.
- Status colours (emerald, rose, amber) darken in the light theme.

## Type and shape

- Font: `Inter Variable`, then `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, then the system font. Nothing is downloaded at runtime.
- Base size 14px. Smallest size is `text-xs`, set to 13px in `@theme`. Do not use `text-[10px]` or `text-[11px]`.
- Corners: `rounded-md` 6px, `rounded-lg` 8px, `rounded-xl` 12px, `rounded-2xl` 16px (set in `@theme`). The composer is `rounded-2xl`; bubbles are `rounded-2xl`.

## Rules from the craft floor

- No eyebrow labels above headings.
- No gradient text. Emphasis comes from weight or size.
- Selection, caret, scrollbars, and focus rings use the palette.
- Shadows have an offset and a soft blur.
- Icons come from `src/components/Icon.svelte`, one stroke set. No emoji or Unicode glyphs as icons.

## Known gaps

- The GOAT pass is verified in the real Electron window (CDP screenshots of Chat, Settings and Usage) plus `svelte-check` and the production build.
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
