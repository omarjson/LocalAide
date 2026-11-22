# ADR-0013: Interactive Forms with A2UI 1.0

Date: 2026-11-22. Status: accepted.

## Context

The chat already draws charts, diagrams and sandboxed ```html mockups, and a
mockup can send one choice back with `localaide.choose`. Forms, pickers and
checklists drawn as HTML run model-written code in a frame, look unlike the
app, and send back only a string. A2UI 1.0 (stable November 11, 2026;
<https://a2ui.org>, specification under Apache-2.0) lets an agent describe a
screen as JSON that the app draws with its own components, with a data model,
bindings, checks and typed actions, and no code to run.

## Decision

1. **Transport: the chat itself.** The model writes A2UI messages in a
   ```a2ui block, one envelope per line (a JSON array also works). A pressed
   button becomes the user's next message: a readable line plus a
   ```a2ui-event block with the renderer-to-agent `action` envelope and,
   when the surface set `sendDataModel`, the renderer data model
   (`{"version":"1.0","surfaces":{...}}`). This works with any model and
   provider, with or without tool calling. While an answer runs, the action
   waits in the message queue.
2. **Catalog: the Basic Catalog v1, drawn natively.** All 18 components and
   the catalog functions (`required`, `regex`, `length`, `numeric`, `email`,
   `formatString`, `formatNumber`, `formatCurrency`, `formatDate`,
   `pluralize`, `openUrl`, `and`, `or`, `not`, and `@index`). Other catalogs
   are refused with a readable problem. The core is
   `src/lib/a2ui.ts`; the drawing is `A2uiNode.svelte`/`A2uiSurface.svelte`;
   `a2ui.svelte.ts` keeps one chat's surfaces, applying each finished block
   once, in chat order, so a later answer can update or delete a surface.
3. **Local first.** No A2UI path makes a network call. Images load from
   `data:` URLs only (the CSP allows nothing else); an http(s) image, video
   or audio address shows a link the user can open in the browser.
   `openUrl` runs only from a press and only for http(s).
4. **Errors go back in the spec's shape.** Problems show under the form in
   plain words, and "Ask to fix it" sends an A2UI `error` envelope
   (`VALIDATION_FAILED`) so the model can correct itself (the spec's
   prompt-generate-validate loop).
5. **Not supported:** `callRendererFunction` and `callAgentFunction` (no
   function channel in a chat; reported and skipped), progressive drawing of
   an unfinished block (a block draws when its closing fence arrives), and
   keeping typed form values across restarts.

## Consequences

- The system prompt grows by the A2UI guidance (about 600 tokens) while
  "Rich output" is on. It is part of the measured instructions in the
  context meter.
- The 43 Basic Catalog v1 examples and the spec's contact-form stream build
  with no problems and render without errors (checked while writing this).
