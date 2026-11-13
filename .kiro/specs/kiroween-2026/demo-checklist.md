# Kiroween 2026 — live demo checklist

A ten-minute walkthrough that exercises every shipped feature in the order a
judge would meet them. Run against a clean install (delete
`%USERPROFILE%\.aiassistant` first if the machine has seen the app before).
Every step lists what success looks like and what to do if it does not happen.

## 0. Before you start

- [ ] Latest build installed from CI artifacts (Quality Gate → sidecar →
      platform build on `kiroween/engine-and-design`), or `make release` with
      the sidecar staged per PACKAGING.md.
- [ ] Internet available for the model download; nothing else needs the
      network for the rest of the demo.
- [ ] Budget check: the machine should have ≥ 8 GB RAM so the Tiny preset fits.

## 1. First run and system detection

- [ ] Launch the app. The setup wizard probes real system memory and shows
      **Total / Usable / 70% budget** figures that match the machine.
- [ ] A preset is marked **Recommended** and presets larger than the budget
      are greyed out with "Too large".

*If detection fails:* the wizard says so instead of guessing; check the
`system_probe` command in the dev tools console and continue — steps below do
not depend on the recommendation.

## 2. Download the Tiny preset

- [ ] Pick Tiny → Install. The progress bar **moves** (aggregate percent and
      the current model id are shown).
- [ ] When it finishes, "Open app" lands you **in Chat** — no reload loop.

*If the download fails:* the wizard returns to the preset step with the error
text; re-running resumes via the `.part` file rather than restarting.

## 3. Chat, fully local

- [ ] Send a starter prompt. The answer streams in token by token.
- [ ] The header shows the local model with the emerald dot — **no cloud
      badge**. Nothing has left the machine.
- [ ] Stop a long answer mid-stream with the stop control; the partial text
      stays and is marked, not lost.

## 4. Folder indexing and search

- [ ] In Search, index a small folder of text/markdown notes.
- [ ] Search a phrase that exists in one file. Results return with the file
      path and snippet. The subtitle says **BM25 keyword retrieval** — that is
      what runs; no vector claims.

## 5. Memory with approval

- [ ] In Memory, create a note ("Prefers concise answers", for example).
- [ ] Ask the assistant to write a file from Chat's tools panel. The write
      **requires explicit approval**; denying it produces a logged "denied"
      entry, approving it writes only inside the workspace jail.
- [ ] Settings → Audit Log shows the tool call rows (time, action, path,
      approved, status). "Clear Log" empties it.

## 6. Opt-in online mode (the honesty moment)

- [ ] Settings → Online Providers. Add a provider (HTTPS base URL — the
      backend rejects plain HTTP to anything that is not this machine).
- [ ] The key hint shows masked dots + last four. Toggle cloud mode on.
- [ ] Chat header now shows the amber **Cloud: model · label** badge and the
      empty state says messages are sent to that provider. Send one message;
      it streams from the provider.
- [ ] Toggle cloud mode off. The local model answers again; the badge returns
      to the emerald local dot.
- [ ] Optional: open the provider's dashboard and show the request landed
      there — and nowhere else in the app does the API key appear.

*If you have no provider key:* skip this section and say so; the demo stands
on local mode.

## 7. Honesty spot-checks (fast)

- [ ] README claims match what was just demonstrated (offline default, opt-in
      online mode, keyword search, sidecar engine).
- [ ] `docs/evidence/audit-2026-11-13.md` is marked superseded and points at
      `.kiro/specs/kiroween-2026/kiro-log.md` for the living log.
- [ ] Settings → Security tab says exactly what the jail enforces; no
      "zero telemetry" line appears anywhere cloud mode could contradict it.

## Wrap-up line for the judge

"Everything you just saw ran on this machine. The one moment traffic left it
was when I explicitly flipped cloud mode on with my own key — and the app told
you, in its own header, while it was happening."
