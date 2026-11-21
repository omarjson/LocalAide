# LocalAide 0.1.6: design

## A. Terminal tool

- `services/tools/shell.rs`: `classify(command, workspace) -> Verdict { ReadOnly | Changes | Blocked(reason) }`. The command is split into segments on `;`, `&&`, `||`, `|`, newlines, backticks and `$(`. A segment is read-only when its program is on an allowlist (with sub-command checks for `git`, `find` without `-delete/-exec`, and similar), it has no output redirection, and no argument names a path outside the workspace (`/`, `~`, `..`, drive letters). Blocked patterns are checked first, on the whole command and each segment.
- `ToolService::run_command(command, cwd, timeout, cancel)` runs `sh -c` (Unix) or `cmd /C` (Windows) with stdin closed, the jail-resolved cwd, `LOCALAIDE_*` variables removed, stdout and stderr read on threads into a 1 MB buffer each, polled every 100 ms for exit, timeout or cancel (the process is killed). Audit row `started` before spawn, finished with `success`/`error`/`timeout`/`cancelled` and `exit N`.
- Output to the model is capped at the run's `max_chars` (at least 4,000). When cut, the full output goes to `workspace/.localaide/command-output/<time>.txt`.
- New `ToolKind::RunCommand`, group `shell`, permission `Permissions.shell` (chat Tools menu row "Terminal", on by default). `tools::run` gets the cancel flag.
- Approval depends on the arguments: `ToolSpec::needs_approval_for(args)`. Read-only commands skip it; blocked commands fail before approval.
- Chat card: terminal-style block with `$ command`, output, exit code and time.

## H. Questions and approvals

- `ask_user` tool (always offered when tools are on): `{question, options[], multiple?, allow_other?}`. The agent loop emits `ChatStreamEvent::Question` and ends the answer; the question is saved as a `question` message part.
- `src/lib/questions.ts`: `parseQuestion(text)` finds a trailing question with 2-6 short options, or a yes/no question. Unit tested.
- `OptionButtons.svelte`: shared buttons with number badges, single/multi select, Other. Used by the question card and the tool approval card.
- `ChatStreamRequest.approval_mode`: `ask | chat | auto`. In `run_call`, `chat` and `auto` skip the question. The header switch stores `auto`/`ask` globally and `chat` per conversation. Share requests always use `ask`.

## G. Cloud audit

- `services/cloud_audit.rs`: table `cloud_audit(id, timestamp, provider, endpoint_host, model, kind, preview, bytes, status)` in the library DB. `record(...)` redacts with `redact(text, secrets)`: removes every configured key, bearer tokens and common key shapes (`sk-…`, `gsk_…`, `AIza…`, long hex/base64 runs), then cuts to 160 characters.
- Hooks: `openai_stream::stream_once` for remote flavors, `RemoteEngine::generate`, web search and page reads, provider model lists, online transcription.
- Commands: `cloud_audit_list(limit)`, `cloud_audit_clear()`. Settings → Online providers shows it as a table.

## D. Self-test

- `selftest_run(channel)`: steps `model` (Tiny chat model installed, else download through the model download flow with checksum), `load` (load_and_activate), `reply` (one short local completion, 120 s limit). Events `{step, status, ms, detail}`. Returns a summary. Settings → General → "Check my setup" with a results list.

## B. Library

- DB migration: `folders(path PK, added_at, last_scan_at, indexed, skipped, limited)`; `embedding_model` config field.
- `index_folder_report` takes a progress callback and collects up to 200 skipped files with reasons.
- Commands: `library_folder_presets`, `library_index_start(folder, channel)` (progress events, cancellable job), `library_index_cancel`, `library_folders`, `library_folder_remove`, `library_status`, `library_embedding_select(model_id)`.
- On startup, the selected embedding model starts in the background if installed. Selecting a model clears vectors from another model and backfills.
- `search_notes` and `search_index` report the mode; the agent tool tells the model when it was keyword-only.
- Built-in guidance in `modes.rs`: questions about the user's own files go to `search_notes` first and cite the file. Repo steering note `.kiro/steering/library-search.md`.
- UI: Library status card and folder presets at the top of the Search screen, indexing progress and report; embedding models in the Models screen get "Use for search".

## C. Providers

- `ProviderCard.svelte` used for every provider. Status from `CloudProvider.check: {ok, at, model_count, error}` saved by `cloud_provider_test(provider_id)`.
- Form: presets (quick fill), Label, URL, Key, Load models → model list with search; empty list → text field.

## E. Share

- `services/share/`: `server.rs` (tiny_http on 127.0.0.1, random port), `tunnel.rs` (cloudflared quick tunnel, parses the trycloudflare URL, kills the child on stop), `auth.rs` (two random 32-byte tokens, compared by SHA-256 in constant time; failed attempts limited), `page.html` (the shared chat page, no external assets).
- Token travels in the URL fragment (`#k=…`), which browsers never send; the page sends it as `Authorization: Bearer`.
- API: `GET /api/me`, `GET /api/threads`, `GET /api/threads/:id`, `POST /api/chat` (NDJSON stream of chat events; remote user), `POST /api/decide` (remote user, only for approvals from link requests), `POST /api/threads/:id` (save, remote user), `POST /api/selftest` (remote user).
- Security headers on every response: strict CSP, no-referrer, no-store, X-Frame-Options DENY. Body limit 2 MB.
- cloudflared: found next to the app, in `~/.aiassistant/bin`, or on PATH; else "Download cloudflared" fetches the pinned release for the platform and checks its SHA-256.

## F. Voice

- Engine: sherpa-onnx `sherpa-onnx-offline` (v2.0.2, shared, no TTS) fetched once into `~/.aiassistant/engines/sherpa-onnx/` with a pinned SHA-256 per platform; run per recording with the model files and a 16 kHz mono WAV. Output JSON line, `text` field.
- Models: catalog entries with `kind: "stt"`, files from `csukuangfj/sherpa-onnx-*` on Hugging Face as companions with pinned checksums (CI's catalog check verifies them against Hugging Face).
- Audio: WebAudio in the window, resampled to 16 kHz mono 16-bit; analyser drives the waveform.
- Online: `services/voice/online.rs` (batch: Gemini generateContent, Deepgram /v1/listen, Groq and OpenAI /audio/transcriptions) and `live.rs` (WebSocket: Gemini Live, Deepgram, Soniox) via tokio-tungstenite. PCM chunks go from the window to the session by command; interim and final text come back on a channel.
- Config `voice: {enabled, model, local_only, cleanup, read_aloud, voice_name, rate, keys{provider: key}}`.
- Cleanup: one short completion on the loaded local model.
