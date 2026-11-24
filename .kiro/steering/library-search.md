---
inclusion: always
---

# Library search rule

When the user asks about their own files, notes, documents or earlier work, the assistant searches the library first (the `search_notes` tool) and answers from what it found, naming the file and citing it as `[n]`. It does not answer such questions from general knowledge or guess.

- The rule lives in the built-in tool guidance (`src-tauri/src/services/inference/modes.rs`, `tool_guidance`) and in the `search_notes` result text (`src-tauri/src/services/agent/tools/mod.rs`). Keep both when changing either.
- Search never degrades silently. When no meaning (embedding) model runs, `search_notes` tells the model the results matched words only, and Search → Library status says so to the user and offers the download.
- Indexing and search stay on the computer. Do not add any network call to `services/library/`.
