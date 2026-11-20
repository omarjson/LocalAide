# LocalAide 0.1.5 follow-up: UI clarity, context usage, slash commands, resumable downloads

Status: released as 0.1.5 (draft). Review pass on 2026-11-20 fixed the issues listed under "Review fixes".

## Delivered

- Local and online chat badges use ember orange in light and dark themes. The online-provider notice uses the same semantic accent instead of amber. Chat's flex parents now use `min-h-0`, so the window stays fixed and only the conversation scrolls.
- Model-picker filters persist in local storage, show their active count, and have a Clear action. Cursor navigation clamps when filtering removes rows.
- Chat More actions include Markdown export, JSON export, Copy full conversation and the existing answer summary. Export waits for the current thread save first; clipboard copy serializes the whole conversation with citations and attachment names.
- The message composer includes a circular context meter. It shows estimated input and output tokens, the context limit, and clearly says when the current engine does not report cache counters instead of inventing cache numbers. Chat settings includes an auto-compact threshold; older turns are reduced before the next send when it reaches the threshold.
- Slash commands are separate from saved skills and include `/clear`, `/compact`, `/model`, `/settings`, `/export`, `/copy`, `/goal`, `/scale`, `/by-the-way`, `/remember`, `/create-skill`, `/web`, `/notes` and `/tools`. Skills still match by name, description and tags.
- Model downloads can pause and resume. Pause marks the job paused, keeps the partial file, and a later Resume starts the existing Range-resume path. The model screen shows downloaded/total bytes, throughput and ETA.

## Verification

- `npm run check`, `npm run build`, `npm test` pass.
- Rust `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` pass with the sandbox's available toolchain/system-library setup.
- Real Tauri-window light-theme rendering, real provider cache counters, model download pause against a live slow server, and real model/MCP execution still need machine verification.

## Review fixes (2026-11-20)

- Slash commands that write a prompt (`/goal`, `/scale`, `/by-the-way`, `/remember`, `/create-skill`) and saved skills were erased right after insertion. The "/word" is now cleared before the command runs. `/tools` opens the Tools menu.
- `/compact` and auto-compact deleted earlier messages from the saved chat. Compaction now moves a cut point: older messages stay on screen and on disk, are not sent, and a divider offers "Send them again". Usage and the trigger share `src/lib/contextUsage.ts` and count attachments, the draft, instructions and the answer reserve; auto-compact also fires when the next answer would not fit.
- Copy and Markdown export now share `src/lib/chatExport.ts` (thinking, tools, attachments, ordered references), written through `chat_export_text_save`.
- Chat saves failed on fractional thinking times. The window rounds them, and the backend also accepts fractions for `ts` and `ms`.
- Downloads: one per model (`DOWNLOAD_BUSY`), Pause and Cancel act within 200 ms while the network is silent, Resume waits for Pause to finish, partial files survive restarts and show as Resume, Cancel discards them, and vision-model progress counts all files.
- Starred models no longer appear twice. Light-theme amber is remapped to the ember family.

Tests added: `tests/unit/context-usage.test.ts`, `tests/unit/chat-export.test.ts`, downloader HTTP tests (Range 206, ignored Range 200, 416, pause during a silent stream), pause state in `jobs.rs`, lenient times in `domain/chat.rs`.
