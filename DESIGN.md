# LocalAide design

Status: first pass applied on 2026-11-13, semantic token pass (light theme, `edge`, `on-ember`) on 2026-11-14. Not yet checked on screen. Review it in the app before treating it as final.

## Surface and mode

- Surface: a desktop app for chat, search, memory, and model management.
- Mode: Operate. Scanning, consistency, and clear controls come first. The brand shows in small details: the logo, the ember accent, and the stitched ghost.

## Logo

- `public/logo.svg`: a stitched ghost with one plain eye and one ember eye, plus a patch. The ember eye stands for the local model.
- Used as the favicon, and beside the wordmark in the sidebar and mobile header.
- Open question: whether the mark needs a light-background version.

## Palette

Neutrals are plum-tinted "ink". The accent is "ember". All tokens live in `src/main.css` under `@theme`; the light theme re-declares the same custom properties under `:root[data-theme="light"]`, so every utility flips automatically.

| Role | Token | Dark value | Light value |
|---|---|---|---|
| Page background | `ink-950` | #120d1a | #f6f1e7 |
| Panels and inputs | `ink-900` | #1a1424 | #fbf7f0 |
| Raised fills | `ink-800` / `ink-700` | #241c33 / #463d58 | #e6ddcf / #d6cab7 |
| Borders (meaningful) | `edge` | #6b6280 (3.35:1) | #857a95 (3.58:1) |
| Muted text | `ink-400` | #aaa2b8 | #6a6155 |
| Body text | `bone` (`ink-100`) | #f3ead8 | #332d24 |
| Accent | `ember-500` | #ff7a1a | #ff7a1a |
| Accent, button fill | `ember-600` | #e8680c | #e8680c |
| Accent text on light fills | `ember-200/300` | #ffd0a3 / #ffb173 | #a34a08 / #a34a08 |
| Focus ring | fixed | #d946ef | #d946ef |

Contrast rules:

- Text on an ember fill uses `on-ember` (`#120d1a` in dark, white in light). Never hardcode white or ink-950 there.
- Meaningful borders use `edge`, sized past the 3:1 non-text floor in both themes; decorative hairlines may still use the ink ramp.
- Body text and placeholders meet 4.5:1 on the page background in both themes. Placeholders use `ink-500`.
- Status text roles (emerald/rose/amber at the 200–400 steps) darken in the light theme so success, error, and warning stay readable.

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
- The light theme is token-complete but has not been eyeballed on a real screen; dark is the designed reference. Nothing here has been checked on screen at all yet.
