# LocalAide interface system (product screens)

Persisted decisions for Settings and other product screens. Read before changing them.

## Intent

- Who: one person on their own computer, setting up an assistant that stays local unless they say otherwise. They open Settings to answer "what can it do, and what leaves my computer?".
- Feel: a calm workbench. Quiet surfaces, one warm accent (ember) for action and for anything that crosses the computer's boundary.
- Signature: the boundary. Anything that sends data out names the host it goes to, in the same place on every card ("Sends to openrouter.ai"), and the online log lists each call. Local things say "on this computer".

## Tokens (existing, from `src/main.css`)

- Surfaces: `ink-950` (inset: inputs, logs), `ink-900` (cards), `ink-800` (hairlines, chips).
- Text: `ink-50` primary, `ink-200` secondary, `ink-400` supporting, `ink-500` metadata.
- Accent: `ember-500/600` for primary actions and boundary crossings; `emerald` only for "works / connected"; `rose` only for failure.
- Depth: borders only (`ink-700` on cards, `ink-800` inside). No shadows on cards.
- Radius scale: 6px controls, 10px chips/inner panels, 12px cards (outer = inner + padding).
- Spacing: 4px base. Card padding 16px; gaps 8 / 12 / 16 / 28 (section).
- Type: system UI stack; 15px section titles semibold, 14px body, 12px metadata; tabular numbers for counts and times.

## Patterns

- Provider card: name + status on the first line; "Sends to <host>" and key state on the second; models it offers as chips; actions in one row at the bottom (switch left, buttons right). Same layout for every provider; greyed with "Not available" when online models are off.
- Status words: Connected (green dot), Not checked (grey), Needs a key (ember), Not reachable (rose, with the reason).
- Forms are one flow, top to bottom: start from a preset (optional) → name → address → key → Load models → pick a model (or type one when the list is empty) → Save.
- Progress: determinate bars when the total is known (downloads), a sweeping bar when it is not (indexing), always with counts in tabular numbers.
- Logs (activity, online calls): inset `ink-950` table, newest first, one line per entry, preview in monospace only for literal content.
- No all-caps eyebrows, no numbered markers unless the content is a sequence (self-test steps are).
