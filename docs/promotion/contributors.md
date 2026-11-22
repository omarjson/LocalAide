# Getting contributors

## Where to post

1. **Hacker News** — the Show HN draft in `show-hn.md`. Best single source of
   technically curious users and Rust/Svelte contributors.
2. **r/LocalLLaMA** — for the local inference and retrieval crowd. Lead with the
   sidecar and FTS5/embedding pipeline, not with "please contribute".
3. **r/opensource** and **r/programming** — use the contributor angle in
   `reddit.md`, and name the specific areas where help is useful.
4. **Your own profile** — pin the repo for a week; post the contributor version
   separately from the user launch.
5. **Local LLM communities** — Discord forums around llama.cpp / local models,
   if you are already a member. Do not join only to drop a link.

## Concrete good-first-issue ideas

These come from the repo's own open items and audit. They are small, real and
don't require deep familiarity with the codebase:

- **Installer smoke reports.** Run the v0.2.1 installers on a real macOS,
  Windows and Linux machine and report first-launch behaviour. The README says
  this hasn't happened yet. Good for users who can't write Rust.
- **Diagnostics log view.** Show `logs/localaide.log` alongside the llama-server
  log in the Diagnostics screen (`docs/evidence/audit-2026-11-14.md`, "open
  items for later").
- **Remove the dead `installed` map.** `ModelManager::is_installed` still reads
  it (same open-items list). A tidy-up with a clear before/after test.
- **A11y measurement.** The border token fix is in code but has not been
  re-measured on a real screen; `docs/evidence/a11y.md` has the checklist.
- **A tool-calling demo.** Exercise chat tools against a model that actually
  supports tool calls, then record the result in `docs/evidence/`. The README
  currently says this has not been done.
- **Screenshot tour.** The project has `preview.html` and a project-map page;
  a short screenshot set for the README would help users evaluate the app.

Before opening these as GitHub issues, add the `good first issue` and
`help wanted` labels, write a short "done when" for each, and link the exact
file or doc. Contributors trust specific scopes more than broad invitations.

## Outreach template (personal, not mass mail)

> Hi — I saw your work on [specific project/topic]. I've been building
> LocalAide, a local-first desktop AI assistant (Tauri 2, Rust, Svelte 5,
> SQLite), and there's a small task I think fits your experience:
> [one-sentence task and link]. It's MIT, CI runs the full gate on every PR,
> and I'm happy to pair on it. Interested?

Send to people whose work you actually know. Do not send the same message to a
list.

## Make it easy to say yes

- Keep `CONTRIBUTING.md` current: it currently says Rust 1.90+ and Node 20+,
  while the repo pins Rust 1.100.0 in `rust-toolchain.toml` and requires Node
  24+ in `package.json`. Fix that before inviting people.
- Put `make all` and `make lint` in the README's contributing section; those
  are the exact gates CI runs.
- Respond to every new issue and PR within a day for the first month. Early
  contributor retention depends more on response time than on the code.
- Label the area (`rust`, `svelte`, `docs`, `testing`) so people can filter.
