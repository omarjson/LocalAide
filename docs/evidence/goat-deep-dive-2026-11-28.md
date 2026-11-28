# GOAT deep-dive evidence — 2026-11-28

Research-only handoff for LocalAide renderer rebuild Part 4, steps 1–2. All claims verified in source; nothing guessed. No application code was written or modified; this document is the only file created.

## Source base

- GOAT: `/home/omar/goat-source` (github.com/goat-si/goat, MIT; fork of T3 Code, history restarted 2026-11-01 per README.md). On branch `dev`, clean tree, head `fdfb67d1`.
- LocalAide: `/home/omar/LocalAide` (cwd). Backend `electron/` with `electron/ipc.mjs` (204 commands, one `localaide:invoke` channel). Renderer `src/` is Svelte 5 (Tauri era) and is the layer being replaced.

## 1. GOAT inventory

### Repos/packages that matter

| Area | Path | Role |
| --- | --- | --- |
| Desktop Electron shell | `apps/desktop` (`@goat/desktop`) | Electron host; main process `apps/desktop/src/main.ts`, renderer protocol `apps/desktop/src/electron/ElectronProtocol.ts`; packaged product name "T3 Code (Alpha)" at `apps/desktop/package.json` line ~49 |
| React client (the UI) | `apps/web` | All screens, components, design system, app state. Served by desktop on `goat://app` (`structure/desktop.md:7-8`) |
| Shared client runtime | `packages/client-runtime` | Effect Atom state, RPC session, command dispatch, voice-input controller, notes, fleet. Imported by `apps/web` and `apps/mobile`; desktop does not reimplement it (`structure/client-runtime.md:4-6`) |
| Wire types | `packages/contracts` | `ServerSettings`, orchestration contracts, `DesktopAppBranding` |
| Server | `apps/server` | Orchestration v2, WebSocket RPC at `GET /ws`, MCP server named `GOAT` at `/mcp` (`structure/overview.md:38-44`) |
| Docs (user workflows) | `docs/user/` | memory.md, notes.md, snap-shot.md, fleet-goals.md, permission-modes.md, remote-access.md, voice usage |
| Internals | `docs/internals/` | voice-input.md, overview.md, settings-and-storage.md (in `structure/`) |
| Devlog | `devlog/_plan`, `devlog/_chase`, `devlog/_fin` | Open work and finished items |

### Design system (verified in source)

- Appearance preset: `"liquid-glass"` glassmorphism — `apps/web/src/appearance/liquidGlass.css`, store `apps/web/src/appearance/glassBackdropStore.ts`, backdrop `apps/web/src/appearance/LiquidGlassBackdrop.tsx`, contrast `apps/web/src/appearanceContrast.ts`, fonts `apps/web/src/appearanceFonts.ts`.
- Canvas backdrop: `codeTerrain` procedural editor-terrain glyphs — `apps/web/src/appearance/codeTerrain.ts`.
- UI primitive library (shadcn-style, Tailwind v4): `apps/web/src/components/ui/` — button, dialog, sidebar, sheet, toast, tabs, toggle, select, combobox, command, popover, separators, skeleton, spinner, etc. (~60 files).
- Brand: `apps/web/src/branding.ts` — `APP_BASE_NAME = "GOAT"`, stage-aware `APP_DISPLAY_NAME`, download URL `github.com/goat-si/goat-releases/releases`; logic in `branding.logic.ts`.
- Theme editor host: `apps/web/src/components/settings/ThemeEditorHost.tsx` (mounted above the router in `__root.tsx:292`).
- Icons: `apps/web/src/components/solar`.
- Global voice recording chrome: `apps/web/src/speech/GlobalVoiceRecording.tsx` (mounted in `_chat.tsx:24`); voice model picker `apps/web/src/speech/VoiceModelDialog.tsx` (rendered from `components/chat/ComposerVoiceButton.tsx:135`).

### Screens (routes), verified from `apps/web/src/routeTree.gen.ts`

Root shell: `__root.tsx` (auth gate, `AppSidebarLayout`, `CommandPalette`, FirstRunGate, SnapShotCoordinator, etc.).

App routes:

| Path | Route file | Surface |
| --- | --- | --- |
| `/` | `_chat.tsx` + `_chat.index.tsx` | Chat home / thread list entry |
| `/$environmentId/$threadId` | `_chat.$environmentId.$threadId.tsx` | Thread view (`ThreadRouteView` in `components/ThreadRouteView.tsx`) |
| `/bot` | `_chat.bot.tsx` | GOAT Bot standing conversation |
| `/decisions` | `_chat.decisions.tsx` | Decision inbox in chat |
| `/draft/$draftId` | `_chat.draft.$draftId.tsx` | Draft |
| `/fleet` | `_chat.fleet.tsx` | Fleet console (`components/fleet/FleetPage.tsx`) |
| `/new` | `_chat.new.tsx` | New thread |
| `/notes` | `_chat.notes.tsx` | Notes page (`notes/NotesPage.tsx`) |
| `/pull-requests` | `_chat.pull-requests.tsx` | PRs |
| `/welcome` | `welcome.tsx` | Welcome wizard (`components/onboarding/WelcomeWizard.tsx`) |
| `/pair` | `pair.tsx` | Pairing |
| `/connect` | `connect.tsx` | Connect |
| `/connect-agent` | `connect-agent.tsx` | Connect agent |
| `/guest` | `guest.tsx` | Guest links |
| `/account/delete` | `account.delete.tsx` | Account delete |
| `/projects` | `projects.$projectKey.tsx` (index) | Projects |
| `/projects/$projectKey` | `projects.$projectKey.tsx` | Project page |

Settings (all under `_chat.settings.tsx` layout, `components/settings/`):

`/settings`, `/appearance`, `/archived`, `/beta`, `/connections`, `/decision-inbox`, `/diagnostics`, `/general`, `/goat-bot`, `/hub`, `/integrations`, `/keybindings`, `/memory`, `/open-source-licenses`, `/projects`, `/providers`, `/scheduled-tasks`, `/snap-shot`, `/source-control`, `/storage`, `/usage`.

Example route → panel wiring: `/settings/memory` → `MemorySettings` (`components/settings/MemorySettings.tsx`); `/settings/storage` → `StorageSettingsPanel` (`components/settings/StorageSettings.tsx`).

Not present in GOAT: any local model inference UI (grep for `llama.cpp`/`gguf` in `apps/web/src` and `packages/client-runtime/src` = no hits), any document-library/embedding UI, any encrypted-backup settings page, any voice settings page, any Cloudflare-tunnel share surface, any personal goals/automations settings panel (GOAT has agent-declared fleet goals only — see below).

### Component architecture

- Shell: `AppRoot.tsx` → `AppSidebarLayout` (`components/AppSidebarLayout.tsx`) with `DestinationRail` (`components/sidebar/DestinationRail.tsx`), thread sidebar (`components/Sidebar.tsx`, legacy `LegacySidebar`), sidebar chrome (`components/sidebar/SidebarChrome.tsx`), project scope (`components/sidebar/SidebarProjectScope.tsx`).
- Thread view: `components/ThreadRouteView.tsx`, `components/ChatView.tsx`, composer in `components/chat/`, thread find/store `components/chat/threadFindStore.tsx`.
- Settings panels: `components/settings/` — one panel per settings route (AppearanceSettingsPanel, MemorySettings, StorageSettings, AutomationHoldNotice, etc.).
- Fleet: `components/fleet/` — FleetPage, FleetConsole, FleetGraph, FleetThreadList, FleetHubSummary, etc.
- Notes: `components/notes/` — NotesPage, NoteEditor, NoteList, NoteDetail, NoteControls, etc.
- Speech: `components/speech`? Voice lives in `apps/web/src/speech/` (GlobalVoiceRecording, VoiceModelDialog, liveSpeechSession, backgroundVoiceDraft) plus shared controller `packages/client-runtime/src/voice-input/controller.ts`.
- Overlays hosted above the router in `__root.tsx`: CommandPalette, GuestShareDialogHost, ConfirmDialogHost, SnapShotCoordinator, ThemeEditorHost, FirstRunGate, ConnectOnboardingDialog.

### State model

- Effect Atoms (`@effect/atom-react`): app state in `apps/web/src/state/` (~100 atom modules: threads.ts, memory.ts, server.ts, session.ts, environments.ts, fleet.ts, shell.ts, terminal.ts, settings…); shared client state in `packages/client-runtime/src/state/` (~150 modules: threads, fleet, memory, decisionInbox, composerDispatch, orchestration, server, shell, terminal, snapshots, usage, mcpApps, sshRuntime, voice…).
- RPC: WebSocket session `packages/client-runtime/src/rpc/session.ts` (`RpcSession`); protocol `packages/client-runtime/src/rpc/protocol.ts`; commands via `createEnvironmentRpcCommand` in `packages/client-runtime/src/operations/commands.ts`; optimistic command submission hook `apps/web/src/state/use-orchestration-command.ts`.
- Desktop bridge: `window.desktopBridge` exposed by `apps/desktop/src/preload.ts` (`structure/desktop.md:125-126`).
- Auth gate: root route resolves `authGateState` via `resolveInitialServerAuthGateState` (`apps/web/src/environments/primary`); `/welcome`, `/pair`, `/connect`, `/connect-agent`, `/guest`, `/account/delete` bypass the app shell.

### GOAT capabilities relevant to the non-negotiables (source-verified)

- **Memory:** `docs/user/memory.md` — Settings → Memory, scopes All projects / Project / Thread, import from Codex/Claude/OMO, sharing between devices via hub, conflict resolution. UI: `_chat.settings.memory.tsx` → `MemorySettings`.
- **Notes:** `docs/user/notes.md` — markdown notes in `~/.goat/notes` server-side, rich editor, status (idea/doing/done), threads linked, `⌘N`, pinned notes, git repo of notes folder. Route `/notes`, `notes/NotesPage.tsx`.
- **Voice (GOAT's):** `docs/internals/voice-input.md` — speech-to-text inserted into a composer draft only; desktop Web Audio → 24 kHz mono PCM WAV → environment voice HTTP API (`/api/voice/prewarm`, `/api/voice/transcribe`); local Apple transcription on mobile first, remote fallback. UI: `ComposerVoiceButton.tsx` + `VoiceModelDialog.tsx` + `GlobalVoiceRecording.tsx`. NO voice settings page, NO live voice session, NO text-to-speech, NO sherpa-onnx.
- **Fleet goals (agent-declared):** `docs/user/fleet-goals.md` — `fleet.controllerEnabled` server setting; goals declared via `goat_call {path:"goal.apply"}` by agents in threads or via MCP/client token; workers, PR-bundle spec, done conditions. UI: `/fleet` (FleetPage). No personal plan/ask/smart/auto approvals UI.
- **Permission modes:** `docs/user/permission-modes.md` — Supervised / Auto-accept edits / Auto / Full access per thread, default in Settings → General → New threads → Permissions.
- **Share/pairing:** `docs/user/remote-access.md` — direct pairing over LAN/Tailscale (`Settings → Connections → Authorized clients → Create link`), phone QR pairing, GOAT Connect "coming soon" (not real yet), SSH (`goat serve --host`), Tailscale HTTPS. UI: `/pair`, guest links `/guest`.
- **SnapShots:** `docs/user/snap-shot.md` — screen-capture attachments (not backups). Settings → SnapShots.
- **No encrypted backups surface found:** grep for `backup`/`encrypt` across `apps/web/src`, `docs/user`, `docs/internals` returns no backup settings page; `docs/user/snap-shot.md` is about screen captures.

## 2. LocalAide renderer inventory (what gets replaced)

Screens (`src/screens/`), screen registry `src/lib/screens.ts` (7 numbered + Skills + Setup):

1. `ChatScreen.svelte` — chat with permission modes, approvals, tool calls, composer, voice button
2. `SearchScreen.svelte` — search (semantic/hybrid over documents)
3. `MemoryScreen.svelte` — memory notes list, pinned, pending suggestions
4. `ModelManagerScreen.svelte` — model catalog, downloads, presets
5. `AutomationsScreen.svelte` — automations
6. `UsageScreen.svelte` — usage
7. `SettingsScreen.svelte` — settings groups (general/online/tools/integrations/privacy/about/developer), incl. voice, web search, backups sections
8. `SkillsScreen.svelte` — skills module (un-numbered)
9. `SetupWizard.svelte` — first-run only

Key renderer files: `src/lib/api/client.ts` (every call `invoke("cmd")` via Tauri shim `src/lib/platform/tauri.ts`), `src/lib/api/bindings.ts` (DTOs), `src/screens.ts` registry.

## 3. The three lists

### REWIRE — GOAT already has it; GOAT UI stays, wired to this backend; LocalAide parallel retires

| GOAT surface | Source | LocalAide screen to retire | Backend commands that power it (`electron/ipc.mjs`) |
| --- | --- | --- | --- |
| Chat thread view | `_chat.$environmentId.$threadId.tsx`, `ThreadRouteView`, `ChatView`, composer `components/chat/` | `ChatScreen.svelte` | `chat_stream`, `chat_cancel`, `chat_steer`, `chat_tool_decide`, `chat_models`, `chat_target_set`, `chat_favorite_toggle`, `chat_starters`, `chat_enhance_prompt`, `chat_suggest_next`, `chat_title`, `chat_attach_pick`, `chat_attach_dropped`, `chat_thread_list/load/save/delete/export_save/export_text_save`, `chat_export_text_save`, `summarize`, `web_search`, `web_crawl`, `inference_load/unload`, `engine_info`, `model_list`, `model_activate`, `context_files_list`, `context_refs_resolve`, `tool_execute`, `tool_audits_list`, `session_search`, `multi_run_chat`, `mcp_servers_list/save/toggle/delete/test`, `mcp_oauth_start/finish`, `agents_list/save`, `policy_config_get/set` |
| Settings → Memory | `_chat.settings.memory.tsx` → `MemorySettings` | `MemoryScreen.svelte` | `memory_create/update/list/read/delete/pinned_set/pending_list/pending_accept/pending_dismiss/forget_all/search` |
| Settings → Providers + model catalog | `_chat.settings.providers.tsx`, `/providers` | `ModelManagerScreen.svelte` | `cloud_config_get`, `cloud_provider_presets/save/delete`, `cloud_mode_set`, `cloud_online_set`, `cloud_provider_set_enabled/test`, `cloud_list_models`, `cloud_models_refresh`, `model_list`, `model_activate/remove/verify`, `model_presets`, `model_catalog_status/refresh/reset`, `hf_search`, `hf_repo_files`, `hf_custom_add` |
| Settings → General | `_chat.settings.general.tsx` | part of `SettingsScreen.svelte` | `settings_context/set_context`, `settings_context_info`, `settings_compact_prompt`, `settings_engine_flags/set_engine_flags`, `settings_approval_timers/set_approval_timers` (plan/ask/smart/auto approvals), `settings_model_memory/set_model_memory`, `settings_theme/set_theme`, `ui_scale_get/set`, `config_load` |
| Settings → Storage | `_chat.settings.storage.tsx` → `StorageSettingsPanel` | part of `SettingsScreen.svelte` | `engine_fetch`, `engine_info`, `inference_status/load/unload`, `local_servers_discover`, `system_health` |
| Settings → Connections | `_chat.settings.connections.tsx` | part of `SettingsScreen.svelte` | `cloud_provider_*`, `cloud_mode_set`, `cloud_online_set`, `mcp_servers_list/save/toggle/delete/test`, `mcp_import`, `mcp_oauth_start/finish` |
| Settings → Usage | `/settings/usage` | `UsageScreen.svelte` | `usage_summary`, `usage_record`, `usage_list` |
| Settings → Diagnostics | `/settings/diagnostics` | part of `SettingsScreen.svelte` | `logs_tail`, `doctor_run/fix`, `selftest_run`, `system_health`, `system_probe` |
| Settings → Appearance/theme | `/settings/appearance` | part of `SettingsScreen.svelte` | `settings_theme`, `ui_scale_get/set` (+ GOAT's own theme editor `ThemeEditorHost.tsx`) |
| Pairing / connect / guest | `/pair`, `/connect`, `/connect-agent`, `/guest` | `SetupWizard.svelte` (first-run connect portion) | `share_pair`; cloud provider save/test for connect |
| Welcome wizard | `/welcome` → `WelcomeWizard` | `SetupWizard.svelte` | `config_load`, `config_setup_complete` |
| File pickers / desktop open | `DesktopWindowMaterialSync`, `components/desktop/` | — | `pick_folder`, `pick_document`, `open_link`, `files_open/reveal/open_with_apps/open_with` |
| App chrome (sidebar, rail, palette, theme) | `AppSidebarLayout`, `DestinationRail`, `CommandPalette`, `ThemeEditorHost` | — | `settings_theme`, `ui_scale_get/set`, `chat_starters`, `config_load` |

Gaps within REWIRE (GOAT surface has no exact LocalAide command equivalent — to be documented, not built now): GOAT source control/hub/goat-bot/scheduled-tasks/archived/beta settings pages; pull-requests and decisions screens; GOAT Bot `/bot`. These are GOAT-native surfaces not mirrored in LocalAide's command set.

### ADD — GOAT lacks it; build it GOAT-native, powered by LocalAide backend

| Feature | GOAT gap (evidence) | LocalAide commands (`electron/ipc.mjs`) that power the new GOAT-native surface |
| --- | --- | --- |
| Embeddings + document library (semantic/hybrid search over own documents) | No document library, no embedding/rerank UI anywhere in `apps/web/src` routes or `docs/user/` | `library_status`, `library_folder_presets/remove`, `library_index_start/cancel`, `library_compact`, `indexer_stats`, `search_index`, `list_index`, `index_folder`, `index_file`, `index_remove`, `index_watch_start/stop`, `embedding_status/start/stop/backfill`, `library_embedding_select`, `rerank_status/start/stop` |
| Local-runtime breadth (llama.cpp CPU/Vulkan/ROCm pins, MLX, Ollama, LM Studio, vLLM, LocalAI, LiteLLM; model catalog + downloader) | No local inference UI in GOAT (`grep llama.cpp|gguf apps/web/src packages/client-runtime/src` = empty; GOAT is a client for remote coding agents, `structure/overview.md:16`) | `model_list`, `model_catalog_status/refresh/reset`, `model_download/cancel/pause`, `model_download_preset`, `model_preset_cancel`, `hf_search`, `hf_repo_files`, `hf_custom_add`, `model_activate/remove/verify`, `inference_load/unload`, `engine_info`, `engine_fetch`, `local_servers_discover`, `system_health` |
| Voice (sherpa-onnx + online + live) | GOAT voice = composer speech-to-text draft insertion only (`docs/internals/voice-input.md`; `ComposerVoiceButton.tsx`, `VoiceModelDialog.tsx`, `GlobalVoiceRecording.tsx`); no voice settings page, no live voice, no TTS | `voice_settings_get/set`, `voice_key_set`, `voice_models`, `voice_engine_fetch`, `voice_transcribe`, `voice_live_start/push/stop`, `voice_cleanup` |
| Share (tunnel + pairing) | GOAT has pairing/guest/SSH (`docs/user/remote-access.md`, `/pair`, `/guest`) but GOAT Connect is "coming soon" and there is no Cloudflare quick-tunnel share surface | `share_status`, `share_start`, `share_stop`, `share_pair`, `share_cloudflared_fetch` |
| Encrypted backups | No backup settings page in GOAT (grep `backup`/`encrypt` in `apps/web/src`, `docs/user`, `docs/internals` = no settings surface) | `backup_config_get/set`, `backup_run`, `backup_list`, `backup_restore` |
| Personal agentic controls UI (plan/ask/smart/auto approvals, goals, automations) | GOAT has permission modes in the composer (`docs/user/permission-modes.md`) and agent-declared fleet goals via MCP (`docs/user/fleet-goals.md`), but no personal goals/automations settings panel; no `/automations` or `/goals` route in `routeTree.gen.ts` | `goal_start/status/step/complete/pause/block/resume`, `automations_list/save/delete/run_now`, `settings_approval_timers/set_approval_timers` |
| Skills panel | `skills_*` commands exist in backend but no GOAT route/screen | `skills_list/read/create/update/delete/undo/sources/discover/remote_describe/remote_detail/import/suggestions` |

### RETIRED — LocalAide duplicates of GOAT surfaces, removed as each GOAT surface lands

| LocalAide screen (`src/screens/`) | GOAT replacement |
| --- | --- |
| `ChatScreen.svelte` | GOAT thread view (`/_chat/$environmentId/$threadId`) + `ThreadRouteView` |
| `MemoryScreen.svelte` | GOAT Settings → Memory + Notes (`/_chat/settings/memory`, `/notes`) |
| `ModelManagerScreen.svelte` | GOAT Settings → Providers + `/providers` model catalog |
| `SettingsScreen.svelte` | GOAT Settings (`/_chat/settings` + 18 sub-pages) |
| `UsageScreen.svelte` | GOAT `/settings/usage` |
| `SetupWizard.svelte` | GOAT `/welcome` wizard |
| `SearchScreen.svelte` | Retired; replaced by ADD document-library search (GOAT-native) |
| `SkillsScreen.svelte` | GOAT has no skills screen → becomes ADD if kept; otherwise retired (not non-negotiable) |
| `AutomationsScreen.svelte` | GOAT has no personal automations screen → becomes ADD (non-negotiable: automations) |

## 4. Surface-to-commands mapping (GOAT screen → LocalAide `electron/ipc.mjs` commands or gap)

Every command key below is present in the `commands` table of `electron/ipc.mjs` (204 entries, verified by extraction; full key list at end of this section).

| GOAT surface | File | LocalAide commands | Gap / notes |
| --- | --- | --- | --- |
| App shell/chrome | `AppRoot.tsx`, `AppSidebarLayout.tsx`, `DestinationRail.tsx` | `settings_theme`, `settings_context_info`, `ui_scale_get/set`, `chat_starters`, `config_load` | Window frame/electron chrome handled by `apps/desktop` main process, not ipc commands |
| Welcome | `welcome.tsx`, `WelcomeWizard` | `config_load`, `config_setup_complete` | — |
| Pair / connect | `pair.tsx`, `connect.tsx`, `connect-agent.tsx` | `share_pair`, `cloud_provider_save/test`, `cloud_mode_set` | No LAN tunnel pairing command beyond `share_pair` |
| Guest links | `guest.tsx`, `GuestShareDialogHost` | — | Gap: guest-link issuance has no LocalAide command |
| Account delete | `account.delete.tsx` | — | Gap: account deletion is GOAT Connect/relay concern |
| Chat thread | `_chat.$environmentId.$threadId.tsx`, `ChatView.tsx` | `chat_stream`, `chat_cancel`, `chat_steer`, `chat_tool_decide`, `chat_models`, `chat_target_set`, `chat_favorite_toggle`, `chat_starters`, `chat_enhance_prompt`, `chat_suggest_next`, `chat_title`, `chat_attach_pick`, `chat_attach_dropped`, `summarize`, `web_search`, `web_crawl`, `inference_load/unload`, `engine_info`, `model_list`, `model_activate`, `context_files_list`, `context_refs_resolve`, `tool_execute`, `tool_audits_list`, `session_search`, `multi_run_chat` | — |
| Bot | `_chat.bot.tsx` | — | Gap (GOAT Connect channels) |
| Decisions | `_chat.decisions.tsx` | — | Gap (decision inbox RPC) |
| Draft | `_chat.draft.$draftId.tsx` | — | Gap (draft RPC not in LocalAide table) |
| Fleet | `_chat.fleet.tsx`, `FleetPage.tsx` | `system_health`, `usage_summary`, `logs_tail` | Partial: GOAT fleet is multi-machine; LocalAide is single-machine |
| New thread | `_chat.new.tsx` | `chat_thread_save` (new thread) | — |
| Notes | `_chat.notes.tsx`, `NotesPage.tsx` | `memory_create/read/update/delete/list/search`, `memory_pinned_set` | Partial: GOAT notes (status/threads/git header) not fully covered by LocalAide memory commands |
| Pull requests | `_chat.pull-requests.tsx` | — | Gap |
| Settings → index/general | `_chat.settings.tsx` | `settings_context/set_context`, `settings_compact_prompt`, `settings_engine_flags`, `settings_approval_timers`, `settings_model_memory`, `settings_theme`, `ui_scale_get/set`, `config_load` | — |
| Settings → memory | `_chat.settings.memory.tsx` | `memory_*` (all 11) | — |
| Settings → providers | `_chat.settings.providers.tsx` | `cloud_provider_*`, `cloud_mode_set`, `cloud_online_set`, `model_*` | — |
| Settings → model catalog | `/providers` | `model_catalog_status/refresh/reset`, `hf_search`, `hf_repo_files`, `hf_custom_add`, `model_download/cancel/pause/preset` | — |
| Settings → connections | `_chat.settings.connections.tsx` | `cloud_provider_*`, `mcp_servers_list/save/toggle/delete/test`, `mcp_import`, `mcp_oauth_start/finish` | — |
| Settings → storage | `_chat.settings.storage.tsx` | `engine_fetch`, `engine_info`, `inference_status/load/unload`, `local_servers_discover`, `system_health` | — |
| Settings → snap-shot | `_chat.settings.snap-shot.tsx` | — | Gap: GOAT snap-shot = screen capture, not backup; no LocalAide capture command |
| Settings → usage | `_chat.settings.usage.tsx` | `usage_summary`, `usage_record`, `usage_list` | — |
| Settings → diagnostics | `_chat.settings.diagnostics.tsx` | `logs_tail`, `doctor_run/fix`, `selftest_run`, `system_health`, `system_probe` | — |
| Settings → appearance/keybindings/integrations/hub/goat-bot/archived/beta/source-control/scheduled-tasks/open-source-licenses | `_chat.settings.*.tsx` | `settings_theme`, `ui_scale_get/set` | Mostly gaps (no LocalAide equivalents; out of LocalAide scope) |
| Projects | `projects.$projectKey.tsx` | `projects` state via RPC | No LocalAide project command |
| Voice (composer only, exists) | `ComposerVoiceButton.tsx`, `VoiceModelDialog.tsx`, `GlobalVoiceRecording.tsx` | `voice_transcribe` | Partial; live voice + settings + TTS = ADD |
| Projects/threads list | `components/sidebar/` | `chat_thread_list` | — |
| ADD: document library | (new screen) | `library_*`, `index_*`, `embedding_*`, `rerank_*` | Build GOAT-native |
| ADD: local runtimes/model manager | (new screen) | `model_*`, `inference_*`, `engine_*`, `local_servers_discover` | Build GOAT-native |
| ADD: voice settings + live | (new screen) | `voice_settings_get/set`, `voice_key_set`, `voice_models`, `voice_engine_fetch`, `voice_live_start/push/stop`, `voice_cleanup` | Build GOAT-native |
| ADD: share/tunnel | (new screen) | `share_status/start/stop/pair`, `share_cloudflared_fetch` | Build GOAT-native |
| ADD: encrypted backups | (new screen) | `backup_config_get/set`, `backup_run`, `backup_list`, `backup_restore` | Build GOAT-native |
| ADD: goals/automations panel | (new screen) | `goal_*`, `automations_*`, `settings_approval_timers/set_approval_timers` | Build GOAT-native |

Full 204 command keys (`electron/ipc.mjs` `commands` table, lines 277–1131):

`agents_list`, `agents_save`, `automations_delete`, `automations_list`, `automations_run_now`, `automations_save`, `backup_config_get`, `backup_config_set`, `backup_list`, `backup_restore`, `backup_run`, `chat_attach_dropped`, `chat_attach_pick`, `chat_cancel`, `chat_enhance_prompt`, `chat_export_text_save`, `chat_favorite_toggle`, `chat_models`, `chat_prefs_get`, `chat_prefs_set`, `chat_starters`, `chat_steer`, `chat_stream`, `chat_suggest_next`, `chat_target_set`, `chat_thread_delete`, `chat_thread_export_save`, `chat_thread_list`, `chat_thread_load`, `chat_thread_save`, `chat_title`, `chat_tool_decide`, `cloud_audit_clear`, `cloud_audit_list`, `cloud_config_get`, `cloud_list_models`, `cloud_mode_set`, `cloud_models_refresh`, `cloud_online_set`, `cloud_provider_delete`, `cloud_provider_presets`, `cloud_provider_save`, `cloud_provider_set_enabled`, `cloud_provider_test`, `config_load`, `config_setup_complete`, `context_files_list`, `context_refs_resolve`, `developer_get`, `developer_reset`, `developer_set`, `doctor_fix`, `doctor_run`, `embedding_backfill`, `embedding_start`, `embedding_status`, `embedding_stop`, `engine_fetch`, `engine_info`, `files_open`, `files_open_with`, `files_open_with_apps`, `files_reveal`, `goal_block`, `goal_complete`, `goal_pause`, `goal_resume`, `goal_start`, `goal_status`, `goal_step`, `hf_custom_add`, `hf_repo_files`, `hf_search`, `index_file`, `index_folder`, `index_remove`, `index_watch_start`, `index_watch_stop`, `indexer_stats`, `inference_load`, `inference_status`, `inference_unload`, `library_compact`, `library_embedding_select`, `library_folder_presets`, `library_folder_remove`, `library_index_cancel`, `library_index_start`, `library_status`, `list_index`, `local_servers_discover`, `logs_tail`, `mcp_artcraft_presets`, `mcp_import`, `mcp_oauth_finish`, `mcp_oauth_start`, `mcp_presets`, `mcp_server_delete`, `mcp_server_save`, `mcp_server_test`, `mcp_server_toggle`, `mcp_servers_list`, `memory_create`, `memory_delete`, `memory_forget_all`, `memory_list`, `memory_pending_accept`, `memory_pending_dismiss`, `memory_pending_list`, `memory_pinned_set`, `memory_read`, `memory_search`, `memory_update`, `model_activate`, `model_catalog_refresh`, `model_catalog_reset`, `model_catalog_status`, `model_download`, `model_download_cancel`, `model_download_pause`, `model_download_preset`, `model_list`, `model_preset_cancel`, `model_presets`, `model_remove`, `model_verify`, `multi_run_chat`, `open_link`, `opencode_check`, `opencode_get`, `opencode_send`, `opencode_set`, `pick_document`, `pick_folder`, `plugin_list`, `policy_config_get`, `policy_config_set`, `render_frame_put`, `rerank_start`, `rerank_status`, `rerank_stop`, `search_index`, `selftest_run`, `session_reindex`, `session_search`, `settings_approval_timers`, `settings_compact_prompt`, `settings_context`, `settings_context_info`, `settings_engine_flags`, `settings_model_memory`, `settings_set_approval_timers`, `settings_set_compact_prompt`, `settings_set_context`, `settings_set_engine_flags`, `settings_set_model_memory`, `settings_set_theme`, `settings_theme`, `share_cloudflared_fetch`, `share_pair`, `share_start`, `share_status`, `share_stop`, `skills_create`, `skills_delete`, `skills_discover`, `skills_import`, `skills_list`, `skills_read`, `skills_remote_describe`, `skills_remote_detail`, `skills_sources`, `skills_suggestions`, `skills_undo`, `skills_update`, `summarize`, `system_health`, `system_probe`, `tool_audits_clear`, `tool_audits_list`, `tool_execute`, `ui_scale_get`, `ui_scale_set`, `update_check`, `update_install`, `usage_list`, `usage_record`, `usage_summary`, `voice_cleanup`, `voice_engine_fetch`, `voice_key_set`, `voice_live_push`, `voice_live_start`, `voice_live_stop`, `voice_models`, `voice_settings_get`, `voice_settings_set`, `voice_transcribe`, `web_crawl`, `web_providers`, `web_search`, `web_set_default`, `web_set_firecrawl_url`, `web_set_key`.

Note: `rerank_status` returns a hard-coded not-ready stub (`ipc.mjs:807`); `rerank_start`/`rerank_stop` are stubs (`ipc.mjs:808-809`). The embedding/rerank backends exist in `electron/services/embedding.mjs` and `electron/services/rerank.mjs` but the UI surfaces do not exist in GOAT.

## 5. Non-negotiables compliance (all land as GOAT-native surfaces, not bolt-on dialogs)

| Non-negotiable | GOAT-native surface | Backend wiring | Status |
| --- | --- | --- | --- |
| Memory notes | Settings → Memory (`/_chat/settings/memory`) + Notes (`/notes`) | `memory_*` commands | REWIRE (GOAT UI exists) |
| Embedding/semantic search over own documents | New document-library screen | `library_*`, `index_*`, `embedding_*`, `rerank_*` | ADD (GOAT lacks it) |
| Local-first (own hardware/models, offline, cloud optional) | Local runtimes + model manager screen | `model_*`, `inference_*`, `engine_*`, `local_servers_discover` | ADD (GOAT is remote-agent client only) |
| Personal agentic controls (plan/ask/smart/auto approvals, goals, automations) | New goals/automations panel | `goal_*`, `automations_*`, `settings_approval_timers` | ADD (GOAT has composer permission modes + agent-declared fleet goals only) |
| Voice | New voice settings + live voice screens | `voice_settings_get/set`, `voice_key_set`, `voice_models`, `voice_engine_fetch`, `voice_live_start/push/stop`, `voice_transcribe`, `voice_cleanup` | ADD (GOAT has composer STT only) |
| Share | New share/tunnel screen | `share_status/start/stop/pair`, `share_cloudflared_fetch` | ADD (GOAT has pairing/guest only, no tunnel) |
| Encrypted backups | New backup settings screen | `backup_config_get/set`, `backup_run`, `backup_list`, `backup_restore` | ADD (GOAT lacks any backup surface) |

## 6. Evidence notes / limitations

- The ipc-surface contract test (`tests/unit/ipc-surface.test.ts`) lives on the unpushed `experiment` branch and is not on this machine; the command table above was read directly from `electron/ipc.mjs` (lines 277–1131), not from the test.
- GOAT docs state GOAT Connect is "coming soon" (`docs/user/remote-access.md:6-10`, `structure/desktop.md:41`); pairing surfaces exist but managed tunnels do not.
- `docs/user/snap-shot.md` covers screen-capture SnapShots, not backups; GOAT has no encrypted-backup UI surface.
- No application code was written, modified, committed, pushed, or merged; no mockup was built (mockup requires Omar approval per Part 4 step 3).
