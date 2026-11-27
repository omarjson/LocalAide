# ADR-0012: Chat tools, thinking, the model picker and MCP

> **2026-11-28:** The Rust/Tauri backend this ADR described was replaced by the Node ESM backend under `electron/`. The decision carries over — the contracts and invariants are the same, implemented in Node.


Date: 2026-11-19. Status: accepted (0.1.4).

## Context

In 0.1.3 the **Web** switch searched the web with the user's whole message before every answer, and **Notes** always pasted recall results into the prompt. A greeting or a general question still triggered a search. The model's thinking (`reasoning_content`) was dropped, so thinking models sat silent and then answered. Chat used one model at a time: the loaded local model, or the single "active" online provider. Omar asked for tools the model calls when it decides to, visible and collapsible thinking, a picker over every model of every enabled provider with its capabilities and thinking effort, file reading, and MCP servers.

## Decision

1. **The model decides.** Chat sends OpenAI-style `tools` to the model and runs a loop (`services/agent/`): stream a round, run the tool calls it asked for, send the results back, until it answers without a tool. Built-in tools: `web_search`, `read_web_page` (only while Web is on), `search_notes`, `save_note`, `list_skills`, `use_skill`, and the workspace tools `list_files`, `read_file`, `search_files`, `write_file`. A system-prompt line tells the model to answer directly when it already knows the answer. Rounds are capped (Standard 8, Strict 6, Autonomous 16); the last round forbids tools so an answer always comes.
2. **Ask before change.** `write_file`, `save_note` and MCP tools (unless the server is marked trusted) stop and ask in the chat, inline on the tool card: Allow once, Allow for this chat, Don't allow. Stop and a 15-minute wait count as no. Every tool call, allowed or refused, gets an activity-log row.
3. **One streaming client.** `services/inference/openai_stream.rs` streams local and online chat the same way: answer text, thinking (`reasoning_content`, `reasoning`, `<think>` tags) and streamed tool calls. When a provider refuses an optional field with 400/422 (or the local server with 500), the request is sent again without it: effort, usage report, temperature, images, then tools. A model that refuses tools is remembered, and it gets the 0.1.3 behaviour instead (web results and notes gathered first and put in the prompt).
4. **Thinking effort.** Sent as `reasoning_effort` (`"none"` = off; llama-server reads it too), as `reasoning: {effort}` for OpenRouter-style gateways, and with `max_completion_tokens` and no temperature for OpenAI's own API. The answer limit gets room for thinking on top of "Longest answer", so thinking cannot eat the whole answer. If it does anyway, the chat says so.
5. **Picker over all providers.** Each provider has an `enabled` switch and a saved model list (from "Load models"). The picker lists installed local models and every enabled provider's models, with icons for thinking, images and tools, plus the context window. Capabilities come from the provider where it says so (OpenRouter's `supported_parameters` and `input_modalities`, llama-server's `/props`), otherwise from the model name. Each message carries its target; the config keeps `active_cloud_provider` plus `active_cloud_model`, so older builds still read the file.
6. **MCP client** (`services/mcp/`): local programs over stdin/stdout, remote servers over Streamable HTTP with the session-based handshake (2025-11-25). Servers that only speak the old SSE transport are not supported. Tools are named `mcp_<server>_<tool>`. Remote addresses must be HTTPS unless on this computer; redirects are not followed. Header and environment values never go back to the window, and backups empty them unless keys are included. Configs from Claude Desktop, Cursor, VS Code, OpenCode and Crush can be pasted in.
7. **Attachments.** Picked or dropped files are read in the backend: text and code decoded, PDFs through the library's PDF reader, images as data URLs. A dropped path is read only if the window really received that drop in the last two minutes.

## Consequences

- Small local models sometimes call tools badly. Bad arguments come back to the model as an error it can fix; unknown tool names list the real ones.
- Not runtime-verified here: real llama-server tool calls, real providers, real MCP servers. The streaming client, retries, MCP transports (including a real stdio child) and the import formats have tests against loopback fakes. Settings and chat were checked in a browser with a fake backend.
- Privacy: messages go online only when the picked model is online. Web and MCP calls happen only when the model calls them, and each one shows as a tool card. PRIVACY.md lists them.
