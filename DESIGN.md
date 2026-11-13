# LocalAide design

Status: first pass applied on 2026-11-13. Not yet checked on screen. Review it in the app before treating it as final.

## Surface and mode

- Surface: a desktop app for chat, search, memory, and model management.
- Mode: Operate. Scanning, consistency, and clear controls come first. The brand shows in small details: the logo, the ember accent, and the stitched ghost.

## Logo

- `public/logo.svg`: a stitched ghost with one plain eye and one ember eye, plus a patch. The ember eye stands for the local model.
- Used as the favicon, and beside the wordmark in the sidebar and mobile header.
- Open question: whether the mark needs a light-background version.

## Palette

Neutrals are plum-tinted "ink". The accent is "ember". All tokens live in `src/main.css` under `@theme`.

| Role | Token | Value |
|---|---|---|
| Page background | `ink-950` | #120d1a |
| Panels and inputs | `ink-900` | #1a1424 |
| Borders | `ink-800` / `ink-700` | #241c33 / #463d58 |
| Muted text | `ink-400` | #aaa2b8 |
| Body text | `bone` (`ink-100`) | #f3ead8 |
| Accent | `ember-500` | #ff7a1a |
| Accent, button fill | `ember-600` | #e8680c |
| Focus ring | fixed | #d946ef |

Contrast rules:

- Text on an ember fill uses `ink-950`. White on ember fails 4.5:1.
- Body text and placeholders meet 4.5:1 on `ink-950`. Placeholders use `ink-500`.
- Status colors (emerald, rose, amber) stay as they are, for success, error, and warning.

## Rules from the craft floor

- No eyebrow labels above headings. Headings carry their own weight.
- No gradient text. Emphasis comes from weight or size.
- Selection, caret, scrollbars, and focus rings use the palette.
- Shadows have an offset and a soft blur.
- Icons are drawn in one stroke style. No emoji or Unicode glyphs as icons.

## Known gaps

- Several screens still use Tailwind utility classes directly. A design-token component layer would make later changes easier.
- Icons: all icons come from `src/components/Icon.svelte`, one stroke set. Add new icons there. Do not add emoji or text glyphs as icons.
- The Dev Config screen is out of the sidebar. Open it from the command palette (Ctrl/Cmd+K) as "Developer config".
- Light mode is not designed.
