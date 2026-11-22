import type { ChatMessage } from "./api/bindings";

/**
 * Context estimates for the chat's meter and auto-compact.
 *
 * The best number is the engine's own: after each answer the backend
 * reports how many tokens the first request of that answer really took
 * (instructions, tool schemas, history and the question). The meter starts
 * from that and adds estimates only for what came after it, the answer and
 * the draft. Without a report (a new chat, or a provider that does not
 * count) it falls back to a text estimate, corrected by how far off the
 * last estimate was for this model.
 */

/** Cost of the system prompt, mode and tool instructions before the backend has reported one. */
export const PROMPT_OVERHEAD_TOKENS = 700;

/** Tokens an image takes, roughly. The backend budgets the same. */
export const IMAGE_TOKENS = 750;

const isWide = (code: number): boolean =>
  (code >= 0x1100 && code <= 0x11ff) ||
  (code >= 0x2e80 && code <= 0x9fff) ||
  (code >= 0xac00 && code <= 0xd7af) ||
  (code >= 0xf900 && code <= 0xfaff) ||
  (code >= 0xff00 && code <= 0xffef) ||
  (code >= 0x20000 && code <= 0x2ffff);

/**
 * Approximate tokens of a text: about four characters a token for English
 * and code, two for other alphabets, one per character for Chinese,
 * Japanese and Korean. Mirrors `estimate_text_tokens` in the backend
 * (`services/inference/context.rs`).
 */
export function estimateTokens(text: string): number {
  let ascii = 0;
  let other = 0;
  let wide = 0;
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    if (code < 0x80) ascii++;
    else if (isWide(code)) wide++;
    else other++;
  }
  return Math.ceil(ascii / 4) + Math.ceil(other / 2) + wide;
}

/** Whether a message is sent back to the model as history. */
export const sentAsHistory = (m: ChatMessage): boolean =>
  !!m.content.trim() && !m.error && !m.content.startsWith("[Chat Error");

/** What one message costs as history: its text, attached text and images, and role markup. */
export function messageTokens(m: ChatMessage): number {
  const attached = (m.attachments ?? [])
    .filter((a) => a.kind === "text" && a.text)
    .reduce((sum, a) => sum + estimateTokens(a.text ?? "") + 12, 0);
  const images = (m.images?.length ?? 0) * IMAGE_TOKENS;
  return estimateTokens(m.content) + attached + images + 4;
}

/**
 * Extra room an answer keeps for thinking, per effort. Mirrors
 * `Effort::thinking_allowance` in the backend.
 */
export function thinkingReserve(effort: string | null | undefined): number {
  switch ((effort ?? "auto").trim().toLowerCase()) {
    case "off":
    case "none":
      return 0;
    case "":
    case "auto":
    case "default":
      return 4096;
    case "minimal":
      return 1024;
    case "low":
      return 2048;
    case "medium":
      return 4096;
    case "high":
      return 8192;
    default:
      return 12_288;
  }
}

/**
 * The engine's own count for a request, and where in the chat it was taken:
 * `upTo` messages were in it (the question included), counted from `from`.
 * `anchor` is the timestamp of its last message, so an edit that rewrote
 * the chat before that point is noticed.
 */
export interface MeasuredPrompt {
  tokens: number;
  upTo: number;
  from: number;
  anchor: number | undefined;
}

/** Whether a measurement still describes the start of this chat. */
export function measurementHolds(messages: ChatMessage[], from: number, m: MeasuredPrompt | null | undefined): m is MeasuredPrompt {
  return !!m && m.tokens > 0 && m.from === from && m.upTo > 0 && m.upTo <= messages.length && messages[m.upTo - 1]?.ts === m.anchor;
}

/** How far off the text estimate was, as a factor, kept within sane bounds. */
export function calibration(measured: number, estimated: number): number {
  if (!(measured > 0) || !(estimated > 0)) return 1;
  return Math.min(1.8, Math.max(0.6, measured / estimated));
}

export interface ContextUsage {
  /** Earlier turns the model will see, with the draft. */
  input: number;
  /** Of those, the tokens LocalAide's answers wrote. */
  output: number;
  /** Prompt, mode, pinned notes and tool schemas. */
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
  /** `used` starts from the engine's own count, not only an estimate. */
  measured: boolean;
}

export interface UsageOptions {
  /** Where compaction cut the history. */
  from?: number;
  draft?: string;
  systemPrompt?: string;
  reserve?: number;
  /** Instructions and tool schemas as the backend counted them last time. */
  overhead?: number | null;
  /** The engine's count for the last request of this chat. */
  measured?: MeasuredPrompt | null;
  /** Correction for the text estimate (see `calibration`). */
  ratio?: number;
}

/**
 * Usage of the history from `from` on, plus the draft, instructions and
 * the answer reserve.
 */
export function contextUsage(messages: ChatMessage[], limit: number | null | undefined, opts: UsageOptions = {}): ContextUsage {
  const from = opts.from ?? 0;
  const ratio = opts.ratio && opts.ratio > 0 ? opts.ratio : 1;
  const sent = messages.slice(from).filter(sentAsHistory);
  const user = sent.filter((m) => m.role === "user").reduce((s, m) => s + messageTokens(m), 0);
  const output = sent.filter((m) => m.role !== "user").reduce((s, m) => s + messageTokens(m), 0);
  const draft = estimateTokens(opts.draft ?? "");
  const overhead = opts.overhead && opts.overhead > 0 ? opts.overhead : PROMPT_OVERHEAD_TOKENS + estimateTokens(opts.systemPrompt ?? "");
  const reserve = opts.reserve ?? 0;

  let used: number;
  let measured = false;
  if (measurementHolds(messages, from, opts.measured)) {
    // The engine's count covers everything up to the question; estimate
    // only the answer and anything said since.
    const after = messages.slice(opts.measured.upTo).filter(sentAsHistory).reduce((s, m) => s + messageTokens(m), 0);
    used = opts.measured.tokens + Math.round((after + draft) * ratio);
    measured = true;
  } else {
    used = Math.round((user + output + draft + overhead) * ratio);
  }
  const input = Math.max(0, used - overhead);
  const total = used + reserve;
  const cap = limit ?? 0;
  return {
    input,
    output: Math.round(output * ratio),
    overhead,
    reserve,
    used,
    total,
    limit: cap,
    percent: cap > 0 ? Math.min(100, (used / cap) * 100) : 0,
    overflow: cap > 0 && total > cap,
    measured,
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
 * history, the tool schemas, the meta context (pinned notes), the skill
 * list and the system instructions. Uses the backend's counts for the
 * instructions and tools when it has reported them, and, given `total`,
 * scales every slice so they add up to the meter's number.
 */
export function contextSegments(
  messages: ChatMessage[],
  opts: {
    from?: number;
    draft?: string;
    systemPrompt?: string;
    tools?: number;
    mcpServers?: number;
    pinnedNotes?: string[];
    skills?: { name: string; description: string }[];
    skillsOn?: boolean;
    /** The backend's count of the system turns (prompt and pinned notes). */
    systemTokens?: number | null;
    /** The backend's count of the tool schemas. */
    toolTokens?: number | null;
    /** The meter's `used`; the slices are scaled to add up to it. */
    total?: number;
  } = {},
): ContextSegment[] {
  const sent = messages.slice(opts.from ?? 0).filter(sentAsHistory);
  const history = sent.reduce((sum, m) => sum + messageTokens(m), 0) + estimateTokens(opts.draft ?? "");
  const segments: ContextSegment[] = [{ label: "Messages", tokens: history }];
  const toolGuess = (opts.tools ?? 0) * TOOL_SPEC_TOKENS + (opts.mcpServers ?? 0) * MCP_TOOL_TOKENS;
  const tools = opts.toolTokens != null && opts.toolTokens >= 0 ? opts.toolTokens : toolGuess;
  if (tools > 0) segments.push({ label: "System tools", tokens: tools });
  const meta = (opts.pinnedNotes ?? []).reduce((sum, note) => sum + estimateTokens(note) + 16, 60);
  segments.push({ label: "Meta context", tokens: meta });
  let skillTokens = 0;
  if (opts.skillsOn) {
    skillTokens = (opts.skills ?? []).reduce((sum, skill) => sum + estimateTokens(skill.name) + estimateTokens(skill.description) + 24, 0);
    if (skillTokens > 0) segments.push({ label: "Skills", tokens: skillTokens });
  }
  const system =
    opts.systemTokens != null && opts.systemTokens > 0
      ? Math.max(0, opts.systemTokens - meta)
      : PROMPT_OVERHEAD_TOKENS + estimateTokens(opts.systemPrompt ?? "");
  segments.push({ label: "System prompt", tokens: system });

  const sum = segments.reduce((s, x) => s + x.tokens, 0);
  if (opts.total && opts.total > 0 && sum > 0) {
    const scale = opts.total / sum;
    for (const segment of segments) segment.tokens = Math.round(segment.tokens * scale);
  }
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
export function compactionStart(messages: ChatMessage[], limit: number, threshold: number, opts: UsageOptions = {}): number {
  const from = opts.from ?? 0;
  if (fits(contextUsage(messages, limit, opts), threshold)) return from;
  const lastKeep = Math.max(from, messages.length - 2);
  for (let start = from + 1; start <= lastKeep; start++) {
    // A cut changes what is sent, so the engine's old count no longer applies.
    if (fits(contextUsage(messages, limit, { ...opts, from: start, measured: null }), threshold)) return start;
  }
  return lastKeep;
}
