# LocalAide 0.1.4: tools, thinking, model picker, MCP

Status: implemented on branch `release/0.1.4`. Written 2026-11-19 from Omar's requests after 0.1.3. Decision record: `docs/adr/0012-chat-tools-and-mcp.md`.

## Requests and what was built

| Request | Built | Where |
|---|---|---|
| Web search, notes, memory, skills and data as tools the model calls when it wants, not an automatic search on every message | Tool loop with built-in tools; Web and Notes switches now grant permission; tool-use guidance in the system prompt; fallback to pre-gathered context for models without tools | `services/agent/`, `commands/chat.rs`, `services/inference/modes.rs` |
| Thinking you can collapse, open and read, with the tool runs | Reasoning streamed from `reasoning_content` / `reasoning` / `<think>`; Thinking blocks and tool cards in answer order, saved with the chat | `openai_stream.rs`, `lib/chatParts.ts`, `ThinkingBlock.svelte`, `ToolCallCard.svelte` |
| MCP servers, add some if there are none | stdio and Streamable HTTP client, Settings → Tools and MCP, presets, config import, per-chat switches | `services/mcp/`, `commands/mcp.rs`, `McpSettings.svelte` |
| Model selector with thinking effort per model, in the same interface; unlimited providers; capabilities, context, thinking and image icons | Picker in the message box over local models and every enabled provider; per-provider switch; effort picker | `commands/picker.rs`, `capabilities.rs`, `ModelPicker.svelte`, `EffortPicker.svelte` |
| Read files | Attach and drop text, code, PDF and images | `commands/attach.rs`, `ChatInput.svelte` |
| Better UI using the UI skills | Composer with + / Tools / Web / mode / model / thinking; inline approvals instead of a modal; craft-floor fixes (no thick coloured left borders, no gradient text) | `.kiro/skills/impeccable`, `.kiro/skills/ui-ux-pro-max` |
| Fix bugs | Thinking dropped; failed answers sent as history; 512-token default too small for thinking; OpenAI reasoning models rejected `max_tokens`/temperature; MCP tokens in backups | see CHANGELOG 0.1.4 |
| Release 0.1.4 and check Kiroween eligibility | Versions, changelog, privacy policy, ADR; eligibility notes below | |

## Ideas taken from other projects
- Claude and ChatGPT: collapsible "Thought for N s", tool cards in answer order, a tools menu with switches in the composer.
- OpenCode and Crush: model picker with search and capability hints; MCP config shapes (`mcp` with `local`/`remote`, `stdio`/`http`) accepted by the importer.
- 9Router and OmniRoute: many providers on at once, each model tagged free or not.
- Claude Desktop and Cursor: the `mcpServers` config block; ask before each tool unless trusted.

## Kiroween 2026 eligibility (checked 2026-11-19)
- Submission period: October 30, 2026, 9:00 PT to December 4, 2026, 14:00 PT. Today is inside it. Source: AWS Japan blog summary of the official rules ("Kiro のハロウィンハッカソン Kiroween 2026 が始まりました", aws.amazon.com/jp/blogs/news/kiroween-2026/); the Devpost rules page refused automated reads.
- Work must be newly created in the period. The first commit (`890c38c`) is 2026-10-30 06:58 PT, about two hours before the start. That remains the one risk; say so in the submission, and do not rewrite history.
- Requirements to keep: public GitHub repository with a visible open-source license, `/.kiro` at the root and not in `.gitignore`, a demo video under 3 minutes, and a write-up of how Kiro was used.

## Not verified here
- Real llama-server tool calls and `/props` fields at tag v1.0.0, real providers, real MCP servers, the Tauri window, and drag and drop in WebView2 and WKWebView.
- Verified: `cargo check` and `cargo clippy -D warnings` (Rust 1.92 locally with stub system libraries; CI uses 1.100), the new Rust unit tests in an isolated crate (streaming, retries, think tags, tool calls, MCP stdio and HTTP parsing, imports, approvals), `npm run check`, `npm test`, `npm run build`, and screenshots of chat, picker, tools menu, approval and Settings against a fake backend.
