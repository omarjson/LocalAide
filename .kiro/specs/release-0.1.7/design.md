# LocalAide 0.1.7: design

## Engine (ADR-0012)
- `services/externals.rs`: `llama_pin()` from `native_arch()` (`IsWow64Process2`, `sysctl.proc_translated`), pins for llama.cpp b12359 on Windows x64/ARM64 (zip), macOS arm64/x64 and Linux x64/arm64 (tar.gz). `Archive::Zip` is unpacked by bsdtar.
- `services/engine.rs`: `ensure()` downloads once (tokio mutex), emits `engine-download`. Called before every load: `load_and_activate`, embedding and rerank starts, startup auto-load, the self-test's new "Model engine" step.
- `sidecar::locate_binary`: env override, then the downloaded engine, then next to the app. `startup_exit_message` explains 0xC000001D / SIGILL.
- CI: the llama-server job, `llama-server.yml`, `prepare-sidecar.sh` and `externalBin` are gone (`tauri.release.conf.json` keeps the license file). The Windows matrix adds `aarch64-pc-windows-msvc` on `windows-11-arm`; `make-latest-json.mjs` maps it to `windows-aarch64`.

## Chat loop
- `ApprovalMode`: Plan (changes refused, read-only), Ask, Chat (0.1.6), Smart (asks when `ToolSpec::risky`: `shell::risky`, overwriting a file, MCP), Auto. `must_ask` holds the rule.
- `AgentRun.jail`: the chat's folder (`folder_jail` checks it) or the workspace. `ToolService::{execute_in, run_command_in}` take the jail.
- New tools: `update_todos` → `Todos` event; `run_subagent` (up to 4 per round, run in `thread::scope`, read-only, `SubSink` turns their activity into `SubagentStep`); `save_skill` → `SkillsService::learn` with `.history/` records and `skills_undo`. `ask_user` takes `questions[]` with option descriptions (old form still read).
- Guidance added to the system prompt: the folder, Plan mode, and the chart/mermaid/html blocks.

## Helpers (`commands/assist.rs`)
- `chat_enhance_prompt`, `chat_suggest_next` (one-shot, no tools, `services/agent/oneshot.rs`), `chat_starters` (from saved chats' first messages), `files_open` / `files_reveal` / `files_open_with(_apps)` (paths checked against the chat's jail; runnable types refused), `ui_scale_get/set` (webview zoom, saved in `ui-scale.json`, applied at startup).
- `services/frames.rs`: `mockup:` scheme serving model pages with `default-src 'none'` (no connect-src), loaded in an iframe with `sandbox="allow-scripts"`; `localaide.choose()` posts the pick, accepted only from that frame while it has focus.

## Frontend
- App keeps `ChatScreen` mounted (hidden when not shown, `active` prop gates its keys and drops); scroll positions per screen; `settingsView` remembers the Settings section and handles each request once.
- `chats.svelte.ts` + `chatGroups.ts` + `SidebarChats.svelte`; the header drawer is gone.
- `PermissionMenu`, new `ModelPicker` (provider view, effort slider), `QuestionDock`, `WorkStatus`, `TodoList`, `FileActions`, `ChartBlock` (SVG, no model code runs), `MermaidBlock` (lazy, shown as an image), `HtmlFrame`.
- `scale.svelte.ts` with Ctrl +/−/0.
