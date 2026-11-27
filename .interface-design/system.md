# LocalAide interface system (product screens)

Persisted decisions for Settings and other product screens. Read before changing them.

## Intent

- Who: one person on their own computer, setting up an assistant that stays local unless they say otherwise. They open Settings to answer "what can it do, and what leaves my computer?".
- Feel: a calm workbench, after GOAT. Quiet neutral surfaces, ink primary actions, one blue accent for links, focus and anything that crosses the computer's boundary.
- Signature: the boundary. Anything that sends data out names the host it goes to, in the same place on every card ("Sends to openrouter.ai"), and the online log lists each call. Local things say "on this computer".

## Tokens (existing, from `src/main.css`)

- Surfaces: `ink-950` (chrome, inset logs), `ink-900` (canvas, cards), `ink-800` (raised fills, hairlines).
- Text: `ink-100` primary (near-black in light), `ink-300` secondary, `ink-400` supporting, `ink-500` metadata.
- Accent: `ember-500/600` is the blue token (links, focus, live, info); primary actions are ink (`bg-ink-100 text-on-ember`); `emerald` only for "works / connected"; `rose` only for failure.
- Depth: hairline borders (`ink-700` on cards, `ink-800` inside) and one soft elevation shadow on raised surfaces. No glows.
- Radius scale: 6px controls, 8px chips/inner panels, 12px cards (outer = inner + padding).
- Spacing: 4px base. Card padding 16px; gaps 8 / 12 / 16 / 28 (section).
- Type: Inter Variable, then the system stack; 15px section titles semibold, 14px body, 13px metadata (the smallest size); tabular numbers for counts and times.

## Patterns

- Provider card: name + status on the first line; "Sends to <host>" and key state on the second; models it offers as chips; actions in one row at the bottom (switch left, buttons right). Same layout for every provider; greyed with "Not available" when online models are off.
- Status words: Connected (green dot), Not checked (grey), Needs a key (amber), Not reachable (rose, with the reason).
- Forms are one flow, top to bottom: start from a preset (optional) → name → address → key → Load models → pick a model (or type one when the list is empty) → Save.
- Progress: determinate bars when the total is known (downloads), a sweeping bar when it is not (indexing), always with counts in tabular numbers.
- Logs (activity, online calls): inset `ink-950` table, newest first, one line per entry, preview in monospace only for literal content.
- No all-caps eyebrows, no numbered markers unless the content is a sequence (self-test steps are).
