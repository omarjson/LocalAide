# Chat screen mockups (2026-11-14)

Three visual directions for the LocalAide chat screen. Static HTML, 1280×800.
They show the same conversation so only the design changes.

| File | Direction | Theme | Layout |
|---|---|---|---|
| `a-felt-and-stitch.html` | Felt & Stitch | Mid plum, dark | Sidebar (today's layout, refined) |
| `b-night-light.html` | Night-light | Lilac grey, light | Icon rail + sources panel |
| `c-lantern-room.html` | Lantern Room | Deep teal, dark | Top tabs, no sidebar |
| `d-plush.html` | Plush | Blush and plum, light | Thick outlines, offset shadows, word-labelled rail |
| `e-native.html` | At Home on Windows | Mica grey, light | Windows 11 nav pane, command bar, status bar |
| `f-ember-block.html` | Ember Block | Orange field + lilac page | Big type sidebar, answer as one card |

Fonts load from Google Fonts in these mockups only. The shipped app must
bundle the font files so it stays offline.

Design tells avoided on purpose (from the `anthropics/skills/frontend-design`
checklist): cream + serif + terracotta, near-black + one bright accent,
identical rounded cards with one grey shadow, all-caps eyebrow labels,
middle-dot meta strings, monospace data labels, `→` on buttons.
