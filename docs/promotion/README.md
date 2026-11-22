# LocalAide promotion kit

Ready-to-use copy for announcing LocalAide and recruiting contributors. Everything
here is written from claims the repository can actually back — see the honesty
rules below.

## Start here

1. [Launch checklist](launch-checklist.md) — do these before posting anywhere.
2. [Pitch](pitch.md) — the one-liner, 30-second pitch, audiences and honest limits.
3. [Show HN](show-hn.md) — draft post for news.ycombinator.com.
4. [Reddit](reddit.md) — drafts for r/LocalLLaMA and r/selfhosted, plus a
   [final ready-to-paste r/LocalLLaMA post](reddit-localllama-final.md) with
   reply snippets.
5. [Social](social.md) — short thread for X / Mastodon / Bluesky.
6. [Contributors](contributors.md) — where to find them, good-first-issue ideas,
   and an outreach template.

## Honesty rules

These are not optional. LocalAide's README and audit deliberately avoid overclaiming;
promotion must match that standard.

- Say **local-first**, offline by default, zero telemetry — true in the repo.
- Say installers exist for macOS (x64/arm64), Windows (x64/arm64) and Linux
  (x64 .deb/AppImage) — true in v0.2.1 assets.
- Do **not** say the installers are code-signed or were tested on real machines;
  they are not signed yet, and the demo run is still open.
- Do **not** say the assistant has been driven against a real tool-calling model
  or MCP server; the README explicitly says it has not.
- Do **not** say the scorecard numbers are achieved; they are targets.
- Do **not** imply Ed25519 catalog signing exists; the catalog has an integrity
  digest and structural anti-rollback checks instead.
- Mention that the model engine (about 13–25 MB) downloads on first use.
