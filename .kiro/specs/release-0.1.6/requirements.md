# LocalAide 0.1.6: requirements

Base: 0.1.5 (main 08ba735). Branch: `release/0.1.6`. Ships as a draft release from the `v0.1.6` tag; the owner publishes it.

Scope is the eight features below and nothing else. No new chat model presets, no new chat providers, no changes to the chat ModelPicker or to submission/demo files. Local-first stays: commands run locally, indexing and search never leave the machine, online mode stays opt-in and disclosed.

## A. Terminal tool (`execute_command`)

1. The model can call `execute_command` with a command, an optional working folder inside the workspace and an optional timeout.
2. It runs in the existing workspace jail (`~/.aiassistant/workspace`): the working folder must resolve inside it.
3. Read-only commands (ls, cat, pwd, git status/log/diff, and similar) run without asking, as long as they do not name a path outside the workspace and do not redirect output to a file.
4. Commands that write, delete, install or change the system ask first through the existing approval flow.
5. A blocklist refuses destructive commands outright (recursive deletes, disk formatting, raw disk writes, piping a download into a shell, fork bombs, shutdown, privilege escalation, and similar) and the model is told why.
6. Every run has a timeout (default 60 s, at most 300 s) and an output cap. Long output is cut, the cut is stated, and the full output is saved to a file in the workspace whose path is given.
7. Failures (non-zero exit, timeout, missing program) come back as text the model can act on.
8. The chat shows the command and its output, exit code and duration. Every run is written to the existing audit log before it starts and finished after it ends.

## B. Library search inside the system

1. One-click folder presets: Documents, Desktop, Downloads, and a custom folder picker.
2. Indexing shows live progress (files seen, indexed, skipped, current file) and ends with a report of what was indexed and what was skipped, with reasons. It can be cancelled.
3. Indexed folders are remembered with file count and last scan time.
4. A Library status area always shows folders indexed, file count, last scan, and whether search is semantic or keyword-only right now. Without an embedding model it says so and offers the download.
5. The embedding model is pickable like chat models (list, download, select). Selecting one starts it, re-embeds the library, and is remembered across restarts.
6. Search never silently degrades: results and the agent tool say when they are keyword-only.
7. The agent searches the library first when asked about the user's own files, notes or documents, and cites the file. This rule is part of the built-in instructions and a repo steering note.
8. Search results in chat show the file name and a short matching excerpt. Indexing and search stay on the computer.

## C. Settings providers redesign

1. One consistent card per provider: name, status (Connected, Not checked, Needs a key, Not reachable), Test connection, and the models it offers.
2. The add/edit form is one flow: label, URL, key, then Load models fills the model list. An empty list says so and lets the user type the model name.
3. Key storage is unchanged (config.json, 0600, atomic write). The chat ModelPicker is unchanged.

## D. First-run self-test

1. "Check my setup" in Settings runs: Tiny model present (download if missing), load it, get one reply. Each step reports pass or fail, time, and detail.
2. One click, repeatable, no new screens beyond a results view.

## H. Questions and approvals in chat

1. The model can ask a question with options (`ask_user`). Options render as buttons: single choice, multiple choice, Yes/No, and Other (focuses the input). Tapping sends the answer. Keys 1-9 pick options while the chat box is empty.
2. Plain-text questions with a short numbered or bulleted option list at the end of an answer, or a yes/no question, render the same buttons.
3. Tool confirmations use the same option UI ("Run this command?" with Run / Allow for this chat / Don't run) over the existing approval backend.
4. A global approval mode switch in the chat header: Ask every time / Allow for this chat / Auto-approve all. Always visible, changeable any time. Auto-approve never runs blocklisted commands and every run is still audited. Per-MCP-server auto-approve keeps working.

## E. Share the assistant (Cloudflare Tunnel)

1. Settings → Share: off by default. Start opens a local server on 127.0.0.1 and a Cloudflare quick tunnel, and shows the link.
2. Two permissions: Viewer (read conversations) and Remote user (chat, answer approvals for its own requests, run the self-test). Each has its own token, generated on the device, shown in the link once and copyable. Requests without a valid token are refused.
3. Stopping sharing ends the tunnel and the server; old links stop working. A visible "Sharing is on" state shows in the app while it runs.
4. Requests through the link use the same approval flow and audit log; audit entries are marked as coming from the link. The link never uses the desktop's auto-approve mode.
5. If cloudflared is missing, the app offers to download a pinned, checksum-verified copy.

## F. Voice

1. Settings → Voice: off by default. One picker with local and online transcription models; online ones are labelled Live or Batch.
2. Push-to-talk microphone button in chat: hold to record, release to transcribe and send. A waveform and, for live models, interim text show while recording.
3. Local models (smallest first, Whisper tiny default): Whisper tiny, base, small; Parakeet TDT 0.6B v2; Moonshine tiny; Moonshine base; plus Moonshine base Arabic. Downloaded from Hugging Face through the model download flow with pinned checksums. Run by a local sherpa-onnx engine fetched once with a pinned checksum. Local transcription never uses the network.
4. Online models with the user's own key, stored like provider keys: Gemini 3.5 Transcribe Live (live), Gemini 3.5 Transcribe (batch), Deepgram Nova-3 (live and batch), Groq Whisper large-v3-turbo (batch), OpenAI GPT-Transcribe, GPT-4o Transcribe and Whisper (batch), Soniox stt-rt-v6 (live).
5. "Local only" blocks all network use for voice.
6. AI cleanup (off by default): the loaded local model removes filler words and fixes punctuation. No audio leaves the computer for this.
7. Read aloud becomes a user setting (on/off, voice, speed), using the operating system's speech.

## G. Cloud audit log

1. Every outbound call made for chat, transcription, web search, page reads and model lists is logged: when, provider, model, kind, and a short preview (at most 160 characters, keys and tokens removed). The log never contains an API key.
2. The log is visible in Settings next to the online providers, can be cleared, and is stored locally.
3. The chat keeps saying, in plain words, when a reply came from an online model.

## Release

Version 0.1.6 everywhere. `npm run check`, `npm test`, `cargo test`, `cargo build` pass in CI. Draft PR with screenshots of the providers section, the Library status and the share page. Tag `v0.1.6` produces the draft release.
