# LocalAide for Kiroween 2026: plan

Status: proposal. Nothing below is built yet. Decisions marked **Decide** need your answer before work starts.

## 1. Eligibility (checked)

- Kiroween 2026 accepts only projects started in the submission window, October 30 to December 4, 2026. Source: the Kiroween 2026 announcement and rules, found by web search.
- This repo's first commit is dated 2026-10-30, so it qualifies. Keep the history as it is. Do not squash or rewrite it.
- Judges also look for real use of Kiro: specs, steering and hooks. This repo has `.kiro/steering/` and this spec. Keep adding real spec-driven work, and keep a short log of how Kiro shaped each change.

## 2. Category (Decide)

- **Costume Contest.** Judged on a polished, memorable UI. Fits the design work you asked for. **Recommended.**
- **Frankenstein.** Judged on stitching unlike parts into one product. LocalAide already fits this story: Tauri, Rust, Svelte, SQLite and a local model.

We can enter one category per project. The design can still carry the Frankenstein story.

## 3. What the audit found (must fix before a demo)

These come from `docs/evidence/audit-2026-11-13.md`:

1. Every Qwen download fails its checksum. Catalog hashes do not match Hugging Face.
2. Gemma, Llama 4 Scout and DeepSeek R2 presets return HTTP 401. Users cannot install them.
3. Folder indexing skips folders under a dot-folder. Fix: check ignore rules only inside the chosen folder.
4. Hybrid search claims vector search, which never runs. Fix either by wiring real embeddings or by calling it keyword search.
5. Speech and voice return placeholder text. Remove the controls until they are real.
6. README, scope and ARCHITECTURE contain the false claims listed in the audit.

## 4. Product scope for the demo

Keep the core that works, and make it excellent:

- Setup in three steps: pick a size, download, chat. Use plain English everywhere.
- Local chat with streaming, stop, and numbered citations from memory notes.
- Library: add a folder, see it indexed, search it.
- Memory notes: create, edit, delete, with recall.
- Workspace files: read and list, with an approval prompt before any write.

Cut or defer:

- Voice (speech and TTS): defer. Remove the stub controls.
- Vision: defer. Not built.
- Online mode: **Decide.** It adds a privacy-sensitive feature and needs a provider key from you. My recommendation is to skip it for the contest and keep the "local-first" promise clean.
- Dev Config screen: move it out of the main navigation, or remove it.

**Decide:** approve this scope, or name features to add back.

## 5. Model catalog

- Keep Qwen3.5 0.8B Q4_K_M as the Tiny default. The file exists and is public.
- Re-download and recompute the hashes for every entry from Hugging Face metadata. Record the source, file, size and date in `docs/evidence/models-2026-11.md`.
- Drop any model that is gated or missing. A model with no working download does not appear in the list.
- Show a RAM warning for large models.

## 6. Design direction (proposal, you choose)

Brand: LocalAide, a friendly stitched ghost that lives on your computer.

Logo concept: a small patched ghost with a single glowing dot for the "local" light. Keep it simple enough to read at 16 px in the app icon.

Palette (dark by default, WCAG AA contrast to be checked on screen):

| Role | Proposed |
|---|---|
| Background | ink `#120d1a` |
| Surface | plum `#1f1630` |
| Text | bone `#f3ead8` |
| Accent | ember `#ff7a1a` |
| Status ok | ghost green `#7cff6b` |
| Focus ring | `#d946ef` (keep, already in the code) |

Type: one display face for headings, one readable sans for body. Use the impeccable skill in `.kiro/skills/impeccable` for the design pass.

UX rules:

- Every screen has an empty state that says what to do next.
- Every button works, or it is removed.
- Errors say what happened and what to do next, in plain words.

## 7. Work plan (after you approve)

Tracks that can run in parallel, each as a sub-agent with its own files:

- **A. Backend fixes:** folder ignore logic, catalog hashes and presets, hybrid-search honesty, removing voice stubs.
- **B. Design system:** tokens, logo, screens, empty states, navigation cleanup.
- **C. Docs:** README, ARCHITECTURE, scope and SECURITY rewritten to match the code.
- **D. Kiro evidence:** spec tasks, a hook for `npm run check` on save, and a short write-up of how Kiro helped.

Each track ends with a written summary. Testing happens on your machine, not here. For each track I will list the exact commands for you to run.

## 8. Your test list (run on your machine)

- `npm ci`, `npm run check`, `npm run build`
- `cd src-tauri && cargo test --all-targets`
- `cd src-tauri && cargo clippy --all-targets -- -D warnings`
- Launch the app and do the demo path: setup, download the Tiny model, chat, add a folder, search, add a memory note, approve one file write.
