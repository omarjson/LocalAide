# LocalAide 0.1.3: chat, modes, developer mode, keyless search

Status: implemented on branch `release/0.1.3`. Written 2026-11-19 from Omar's requests after 0.1.2.

## Requests and what was built

| Request | Built | Where |
|---|---|---|
| Chat UI that matches current AI chat apps | Full-height layout, centred column, unbubbled answers, growing composer with mode/Web/Notes chips, typing dots, research steps, source cards, history drawer, settings sheet | `ChatScreen.svelte`, `ChatInput.svelte`, `MessageBubble.svelte` |
| Free keyless Kilo Code | Preset `https://api.kilo.ai/api/openrouter`, model `kilo-auto/free`, no key | `commands/cloud.rs` |
| Link to OpenCode | OpenCode Zen preset (key "public"), and Settings → Integrations to link `opencode serve`; "Send to OpenCode" creates a session and posts the task (`POST /session`, `POST /session/:id/prompt_async`) | `services/opencode.rs`, `commands/integrations.rs` |
| Exa and Firecrawl keyless; Firecrawl local | Exa via `https://mcp.exa.ai/mcp` (MCP over HTTP), Firecrawl REST without `Authorization`, optional Firecrawl base URL | `services/websearch/mod.rs` |
| Search not working | DuckDuckGo lite parser read attributes in one order and quote style only; rewritten and tested against the real layout; Automatic provider falls back to Exa and Wikipedia | same |
| Collapse the side menu | Icon rail, Ctrl/⌘+B, remembered | `App.svelte` |
| Skill descriptions from the web | `skills_remote_describe` reads SKILL.md frontmatter, 8 at a time, per visible page | `services/skills/discover.rs` |
| Strict, Unrestricted, Autonomous modes | Mode table and prompt composition; Strict caps temperature; Autonomous reads the best web result | `services/inference/modes.rs`, `commands/chat.rs` |
| Edit system prompt in developer mode; developer mode with flags; modules and extensions | Flags and modules registry with real wiring, request log, mode instruction viewer | `commands/developer.rs`, `lib/dev.svelte.ts`, Settings → Developer |
| Better visuals using web UI skills | Vendored UI UX Pro Max (MIT); its "AI-Native UI" guidance (minimal chrome, sticky input, typing indicator, context cards, regenerate) and pre-delivery checklist (IME-safe Enter, visible focus, reduced motion) | `.kiro/skills/ui-ux-pro-max/` |
| Release 0.1.3 with changelog | Version 0.1.3, `CHANGELOG.md` section, notes generated from it | release job |

## Ideas taken from other projects
- OpenChamber (a UI for OpenCode): session hand-off rather than re-implementing an agent; autonomous runs that report steps.
- Exa and Firecrawl's own keyless docs for the request shapes.

## Not verified here
- Live calls to Kilo, OpenCode Zen, Exa MCP, Firecrawl keyless and DuckDuckGo: the sandbox has no access to them. HTTP behaviour is tested against loopback servers that mimic each protocol.
- OpenCode Zen's free tier may require extra client headers in future; if chat returns 401/403 there, use a Zen key.
