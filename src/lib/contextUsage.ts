import type { ChatMessage } from "./api/bindings";

/**
 * Context estimates for the chat's meter and auto-compact. Tokens are
 * estimated at four characters each, so the numbers are approximate; they
 * err high on purpose, so compaction happens a little early, not late.
 */

/** Fixed cost of the system prompt, mode and tool instructions. */
export const PROMPT_OVERHEAD_TOKENS = 700;

export const estimateTokens = (text: string): number => Math.ceil(text.length / 4);

/** Whether a message is sent back to the model as history. */
export const sentAsHistory = (m: ChatMessage): boolean =>
  !!m.content.trim() && !m.error && !m.content.startsWith("[Chat Error");

/** What one message costs as history: its text, its attached text, and role markup. */
export function messageTokens(m: ChatMessage): number {
  const attached = (m.attachments ?? [])
    .filter((a) => a.kind === "text" && a.text)
    .reduce((sum, a) => sum + estimateTokens(a.text ?? ""), 0);
  return estimateTokens(m.content) + attached + 4;
}

export interface ContextUsage {
  /** Earlier turns the model will see. */
  input: number;
  /** Of those, the tokens LocalAide's answers wrote. */
  output: number;
  /** Prompt, mode and tool instructions. */
  overhead: number;
  /** Room kept free for the next answer. */
  reserve: number;
  /** Input and overhead: what the chat takes up now. */
  used: number;
  /** Used plus the reserve. Over `limit`, the next answer does not fit. */
  total: number;
  limit: number;
  /** used / limit, 0 to 100. 0 when the limit is unknown. */
  percent: number;
  /** The next answer would not fit with its reserve. */
  overflow: boolean;
}

/**
 * Usage of the history from `from` on, plus the draft, overhead and the
 * answer reserve. `from` is where compaction cut the history.
 */
export function contextUsage(
  messages: ChatMessage[],
  limit: number | null | undefined,
  opts: { from?: number; draft?: string; systemPrompt?: string; reserve?: number } = {},
): ContextUsage {
  const sent = messages.slice(opts.from ?? 0).filter(sentAsHistory);
  const user = sent.filter((m) => m.role === "user").reduce((s, m) => s + messageTokens(m), 0);
  const output = sent.filter((m) => m.role !== "user").reduce((s, m) => s + messageTokens(m), 0);
  const input = user + output + estimateTokens(opts.draft ?? "");
  const overhead = PROMPT_OVERHEAD_TOKENS + estimateTokens(opts.systemPrompt ?? "");
  const reserve = opts.reserve ?? 0;
  const used = input + overhead;
  const total = used + reserve;
  const cap = limit ?? 0;
  return {
    input,
    output,
    overhead,
    reserve,
    used,
    total,
    limit: cap,
    percent: cap > 0 ? Math.min(100, (used / cap) * 100) : 0,
    overflow: cap > 0 && total > cap,
  };
}

/** One slice of what fills the context window, for the hover breakdown. */
export interface ContextSegment {
  label: string;
  tokens: number;
}

const TOOL_SPEC_TOKENS = 400;
const MCP_TOOL_TOKENS = 800;

/**
 * What composes the prompt, the way the model receives it: the message
 * history, the tool schemas, the meta context (pinned notes, folder line),
 * the skill list and the system instructions. Everything is the same
 * chars/4 estimate as the meter, so shares match its total.
 */
export function contextSegments(
  messages: ChatMessage[],
  opts: { from?: number; draft?: string; systemPrompt?: string; tools?: number; mcpServers?: number; pinnedNotes?: string[]; skills?: { name: string; description: string }[]; skillsOn?: boolean } = {},
): ContextSegment[] {
  const sent = messages.slice(opts.from ?? 0).filter(sentAsHistory);
  const messageTokens = sent.reduce((sum, m) => sum + estimateTokens(m.content) + 4, 0) + estimateTokens(opts.draft ?? "");
  const segments: ContextSegment[] = [{ label: "Messages", tokens: messageTokens }];
  const tools = (opts.tools ?? 0) * TOOL_SPEC_TOKENS + (opts.mcpServers ?? 0) * MCP_TOOL_TOKENS;
  if (tools > 0) segments.push({ label: "System tools", tokens: tools });
  const meta = (opts.pinnedNotes ?? []).reduce((sum, note) => sum + estimateTokens(note) + 16, 60);
  segments.push({ label: "Meta context", tokens: meta });
  if (opts.skillsOn) {
    const skillTokens = (opts.skills ?? []).reduce((sum, skill) => sum + estimateTokens(skill.name) + estimateTokens(skill.description) + 24, 0);
    if (skillTokens > 0) segments.push({ label: "Skills", tokens: skillTokens });
  }
  segments.push({ label: "System prompt", tokens: PROMPT_OVERHEAD_TOKENS + estimateTokens(opts.systemPrompt ?? "") });
  return segments;
}

/** True when the chat is under the threshold and the answer still fits. */
export const fits = (u: ContextUsage, threshold: number): boolean => u.percent < threshold && !u.overflow;

/**
 * Where to cut the history so the rest is under `threshold` percent of the
 * limit and the answer reserve still fits. Returns `from` unchanged when it
 * already fits. The newest two messages always stay, so the model keeps the
 * question it is answering.
 */
export function compactionStart(
  messages: ChatMessage[],
  limit: number,
  threshold: number,
  opts: { from?: number; draft?: string; systemPrompt?: string; reserve?: number } = {},
): number {
  const from = opts.from ?? 0;
  if (fits(contextUsage(messages, limit, opts), threshold)) return from;
  const lastKeep = Math.max(from, messages.length - 2);
  for (let start = from + 1; start <= lastKeep; start++) {
    if (fits(contextUsage(messages, limit, { ...opts, from: start }), threshold)) return start;
  }
  return lastKeep;
}
