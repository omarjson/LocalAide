# Eligibility disclosure

Kiroween 2026 accepts only projects built during the submission period, October 30 to December 4, 2026. Projects started before October 30 are not eligible ([rules](https://kiroween.devpost.com/)). This file sets out what the git history shows so judges can decide for themselves. Every number below can be reproduced with the commands in the last section.

## Summary

- The first commit is a working app, not an empty setup. It landed on October 30, before the window's opening time cited in `plan.md`.
- Almost none of that code survives. **700 of the 23,047 lines** of app source in the current tree (3.0%) trace back to the first commit.
- Every feature a judge will see in the demo was built between October 30 and November 15.

## Timeline

| When (Pacific) | What | Evidence |
|---|---|---|
| Oct 30, 06:58 PDT | First commit `890c38c`: 326 files, 155,834 lines | `git show --stat 890c38c` |
| Oct 30, 07:42 PDT | GitHub repository created | `gh api repos/omarjson/LocalAide` → `created_at` 14:42 UTC |
| Oct 30, 09:00 PDT | Window opens. `plan.md` gives this hour; the rules page could not be fetched to confirm it. If the window opened at midnight, the first commit falls inside it | — |
| Oct 30, 06:58 to 14:18 PDT | 17 commits in total. The largest, `36f8481` at 13:26 PDT (10,205 lines added), came after the window opened | `git log --since=2026-10-29 --until=2026-10-31` |
| Nov 2 | Impeccable Kiro skill installed, UI fixes (`eef0dae`) | |
| Nov 13 to 15 | 129 of the 147 commits: engine rewrite, online mode, security fixes, design, release | `git log --format=%ad --date=short \| sort \| uniq -c` |

## What the first commit contained

**Own code: 23 files, 4,137 lines.**

- **Rust (2,716 lines):** `lib.rs`, `model_manager.rs`, `indexer.rs` (Tantivy), `inference.rs` (in-process llama.cpp bindings), `embeddings.rs` (Candle), `voice.rs`, `tools.rs`, `watcher.rs`, `config.rs`.
- **Svelte/TypeScript (1,421 lines):** `App.svelte` plus seven screens (Chat, DevConfig, Memory, ModelManager, Search, Settings, SetupWizard), mostly 100 to 235 lines each.

**Vendored code: `vendor/llama_cpp_rs`, 140,369 lines.** These are third-party Rust bindings to llama.cpp, not our code.

**Dependencies that are all gone today:** `llama_cpp_rs`, `tantivy`, `tantivy-stemmers`, `sled`, `lmdb`, `candle-core`, `candle-transformers`, `candle-nn`, `rodio`.

**Other files:** lockfiles, README, CONTRIBUTING, CODE_OF_CONDUCT, a CI workflow, `verify-packaging.sh`.

The first commits gave the year as 2025 in `LICENSE` and `CHANGELOG.md`. That was a typo, since corrected to 2026.

## What survives today

Lines in the current app source (`src/`, `src-tauri/src/`, `src-tauri/tests/`) whose last change is the first commit. Measured with `git blame -w -M -C`, which also follows lines that moved between files:

| File | Lines from `890c38c` / total |
|---|---|
| `src-tauri/src/model_manager.rs` | 109 / 262 |
| `src/screens/SetupWizard.svelte` | 87 / 417 |
| `src/App.svelte` | 83 / 310 |
| `src/screens/MemoryScreen.svelte` | 80 / 308 |
| `src/screens/ModelManagerScreen.svelte` | 62 / 749 |
| `src/screens/ChatScreen.svelte` | 61 / 857 |
| `src/screens/SearchScreen.svelte` | 55 / 693 |
| `src-tauri/src/config.rs` | 44 / 307 |
| `src-tauri/src/lib.rs` | 21 / 316 |
| **All app source** | **700 / 23,047 (3.0%)** |

These leftovers are screen skeletons, model-manager plumbing and config structs. None of them is a feature on its own.

On Nov 13, chat moved to a llama-server sidecar (`ea9ed68`) and the vendored engine was deleted (`d8c4427`). `indexer.rs`, `inference.rs`, `embeddings.rs`, `voice.rs`, `tools.rs` and the Dev Config screen are all deleted or rewritten.

## What was built during the window

| Feature | First commit | Date |
|---|---|---|
| llama-server sidecar engine (ADR-0007), replacing the vendored engine | `ea9ed68` | Nov 13 |
| Opt-in online providers: `RouterEngine` + `RemoteEngine` (spec: `.kiro/specs/online-providers/`) | `3c6b034` | Nov 13 |
| SQLite FTS5 library replacing Tantivy/sled (ADR-0002) | `36f8481`, `68c9303` | Oct 30, 13:26 PDT onward |
| Real vector and hybrid search with a local embedding engine | `289c675` | Nov 14 |
| Reranker second stage | `05d5775` | Nov 14 |
| Encrypted backups (ADR-0009) | `4306de8` | Nov 14 |
| Web search and page reading (ADR-0011) | `59794c4` | Nov 14 |
| Community models from Hugging Face (ADR-0010) | `11eb820` | Nov 14 |
| Stitched-ghost logo and design system, then the light theme | `04edb8c`, `aa5daf6`, `6282dd7` | Nov 13 to 14 |
| Security fixes SEC-01 to SEC-25 (`docs/evidence/audit-2026-11-14.md`) | `46ec305` and later | Nov 14 |
| Vendored engine snapshot removed | `d8c4427` | Nov 13 |
| Skills, workspace tools with approval, image input, release pipeline | see `CHANGELOG.md` | Nov 13 to 15 |

## Owner's statement

> **Omar: fill in the bracketed parts, then delete this line.**
>
> The code in the first commit was written by me on [date or dates], on [my own machine / another private repository: name it]. It was not part of a previous hackathon entry or a released product. I pushed it on the morning of October 30 as a starting point, before I had checked the window's opening hour. Everything shown in the demo was built after that, as the table above shows.

If the first-commit code was written before October 30, send the note below to the organizers before submitting, and follow their answer.

## Note to the organizers

Send this through the Devpost "Contact the manager" link or the Kiro Discord.

> Subject: Kiroween 2026 eligibility question, LocalAide
>
> Hi, I'd like to check eligibility before I submit. My repo (https://github.com/omarjson/LocalAide) has a first commit on October 30 at 06:58 PDT. It contained a small prototype (about 4,100 lines of my own Rust and Svelte, plus 140,000 lines of vendored third-party llama.cpp bindings). The prototype was written [when]. Since then I deleted or rewrote almost all of it: 3% of the current 23,000 lines trace to that commit. Every feature in my demo was built between October 30 and November 15. A full breakdown is in `.kiro/specs/kiroween-2026/eligibility.md`. Is the project eligible, and if so, is there anything you'd like me to state in the submission?
>
> Thanks, Omar

## Reproduce the numbers

```bash
git show --stat 890c38c | tail -1
git show --numstat --format= 890c38c | grep -v vendor/ | grep -E '\.(rs|svelte|ts|css)$' | awk '{n++; s+=$1} END {print n, s}'
gh api repos/omarjson/LocalAide --jq .created_at
# Surviving lines: blame every app source file, count lines from 890c38c
for f in $(git ls-files 'src/*' 'src-tauri/src/*' 'src-tauri/tests/*' | grep -E '\.(rs|svelte|ts|css)$'); do
  git blame --line-porcelain -w -M -C HEAD -- "$f" | grep -E '^[0-9a-f]{40} '
done | awk '{t++} /^890c38c/ {o++} END {print o " of " t}'
```

The history is kept exactly as it was. Nothing has been squashed or rewritten.
