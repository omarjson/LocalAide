# LocalAide 0.1.6: tasks

- [x] 1. Version 0.1.6, branch `release/0.1.6`, design skills in `.kiro/skills/`
- [x] 2. A: shell classifier + tests; `run_command` with jail, timeout, cap, audit; `execute_command` tool; chat terminal card
- [x] 3. H: approval mode (backend + header switch); `ask_user` tool + Question event; `parseQuestion`; OptionButtons for questions and approvals; number keys
- [x] 4. G: cloud audit table, redaction + tests, hooks, Settings log
- [x] 5. D: self-test command and results view
- [x] 6. B: folders table, progress/report, presets, status, embedding selection + autostart, mode reporting, agent guidance, steering note, UI
- [x] 7. C: provider test command + saved status, ProviderCard, one-flow form
- [x] 8. E: share server, tokens and roles, tunnel, cloudflared fetch, share page, Settings Share section and "Sharing is on" state
- [x] 9. F: stt catalog entries, sherpa engine fetch + run, recording + waveform, push-to-talk, online batch + live, local only, cleanup, read-aloud settings
- [x] 10. Verification pass, UI polish pass with the installed skills, CHANGELOG, screenshots, draft PR, tag `v0.1.6`

Not started: the stretch goal (Look at my screen).

## Verification (2026-11-21, in the build sandbox)

Run for real here:
- `cargo test`: 190 pass (the GUI libraries are stubbed only for linking), clippy clean, `npm run check`, `npm test` (40) and `npm run build` pass.
- Local transcription through the app's code with the real engine and models: Whisper tiny 0.9 s, Moonshine tiny 1.1 s, Parakeet 2.8 s on the sherpa-onnx test clip; Whisper base, small and Moonshine Arabic also transcribe their clips.
- The pinned sherpa-onnx and cloudflared downloads: checksum, unpack and start.
- The share server over HTTP: tokens, Viewer and Remote user rules, refusal of foreign approvals, dead link after Stop.
- The shell classifier and runner: timeouts, cancel, output cap, blocklist.
- UI with a fake backend in a browser: providers, library status and report, terminal cards, approval and question buttons with number keys, self-test, voice, share, both themes.

Could not run here (blocked network or no window/microphone); on the owner's computer:
- Self-test on the Tiny preset (Hugging Face download), a real terminal approval in the window, the microphone with Whisper tiny, one online transcription with a key, a public share link (trycloudflare.com refused from here), and a real cloud call appearing in the log.
