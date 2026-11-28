# The GOAT renderer, vendored into LocalAide

This folder is GOAT's React app, vendored from
[github.com/goat-si/goat](https://github.com/goat-si/goat) (MIT, see
`GOAT-LICENSE`). It was taken from branch `dev`, commit `fdfb67d1`, on
2026-11-28.

GOAT is a control surface for coding agents that run on other machines. It
talks to a GOAT server over a WebSocket carrying the Effect RPC protocol.
LocalAide is a local-first assistant whose backend answers one `localaide:invoke`
channel with a flat table of 204 commands. The renderer therefore keeps GOAT's
design system and its screens, and replaces its data layer.

## What came over unchanged

- `src/index.css` — the theme tokens and the Tailwind entry.
- `src/appearance/liquidGlass.css` — the glass appearance.
- `src/components/solar/` — the icon set.
- `src/components/ui/` — the button, dialog, sidebar, sheet, toast, tabs,
  tooltip and other primitives.
- `src/lib/utils.ts` — the `cn` helper and the tailwind-merge rules the
  design system relies on.

## What was changed, and why

| Vendored file | Change | Why |
| --- | --- | --- |
| `src/main.tsx` | Rewritten | GOAT started twenty services and loaded Clerk's sign-in shell when it had a cloud account. LocalAide runs on the user's own computer. |
| `src/routes.ts` | New, in code | GOAT's routes were files with a generated route tree. LocalAide writes the tree in code, so no code generation step runs. The history is a hash history, because the Electron window loads the app from a file. |
| `src/components/shell/` | New | GOAT's rail, sidebar and shell were written against a server that could hold many environments and projects. LocalAide's rail lists the places the app can go. |
| `src/hooks/useSettings.ts` | Rewritten | GOAT read these from a server that pushed a settings stream. |
| `src/hooks/useLocalStorage.ts` | Rewritten | GOAT validated every value with an Effect Schema; LocalAide stores plain JSON. |
| `src/components/ui/sidebar.tsx` | Three lines | The Effect Schema calls became plain JSON calls. |
| `src/components/ui/toast.tsx` | One function | GOAT's threads lived inside an environment and a draft could stand in for one; LocalAide has neither. |
| `src/i18n/gtShim.tsx` | New | GOAT ships nine locales; LocalAide ships one, so the calls pass their English text through. |
| `src/lib/chatParts.ts` | Moved | The helper that builds an answer from the chat stream. |

## What was left behind

GOAT's parts that need a GOAT server: the Effect RPC session and the
orchestration state atoms, the provider packages (Claude Code, Cursor, Codex
and the rest), source control, the hub, the fleet, pull requests, the decision
inbox, the GOAT bot and guest links. LocalAide's backend has no equivalent,
so the renderer asks the backend for what LocalAide has instead.

The shared runtime that replaced GOAT's WebSocket RPC lives in
`packages/client-runtime`: the bridge over `localaide:invoke`, the typed
client for the command table and the backend's wire types.
