# GOAT rebuild progress — LocalAide renderer

Branch: `goat-rebuild` (from `main` @ 49e11e9).
Spec: `docs/evidence/goat-deep-dive-2026-11-28.md`.
GOAT source: `/home/omar/goat-source` @ `fdfb67d1` (branch `dev`), MIT licensed.

Resume rule: on start, read this file and `git log goat-rebuild`; continue from the
first slice without a `DONE` line below. Never redo a completed slice.

## Slices

1. Scaffold — vendor GOAT's React app + shared runtime, pnpm workspace + vite build,
   Electron loads the React app, IPC adapter over `localaide:invoke` (204 commands).
2. Shell + chat — GOAT shell (rail, sidebar, thread view, composer) rewired to LocalAide
   chat commands.
3. Settings — GOAT settings pages rewired (Memory, Providers + model catalog, General,
   Storage, Connections, Usage, Diagnostics, Appearance).
4. Notes — GOAT `/notes` rewired to `memory_*`.
5. ADD features — document library + embeddings, local runtimes + model manager,
   voice settings + live voice, share/tunnel, encrypted backups, goals/automations,
   skills panel.
6. Setup wizard — GOAT `WelcomeWizard`, same visuals and flow, plus LocalAide steps.
7. Retire + release prep — remove the Svelte renderer, bump to 0.3.0, full checks green.

## Status

SLICE 1: IN PROGRESS
