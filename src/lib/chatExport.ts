import type { ChatMessage, MessagePart } from "./api/bindings";

/**
 * One Markdown form of a conversation, used both for "Copy full
 * conversation" and "Export as Markdown", so the two always match.
 */

const TOOL_STATUS: Record<string, string> = {
  done: "done",
  failed: "failed",
  denied: "not allowed",
  running: "stopped",
  waiting: "stopped",
};

/** Quote text as a Markdown blockquote, line by line. */
const quote = (text: string) =>
  text
    .trim()
    .split("\n")
    .map((line) => (line ? `> ${line}` : ">"))
    .join("\n");

/** A fence longer than any backtick run inside, so code cannot break out. */
const fence = (text: string) => {
  const longest = Math.max(2, ...(text.match(/`+/g) ?? []).map((run) => run.length));
  return "`".repeat(longest + 1);
};

function partLines(part: MessagePart, withThinking: boolean): string[] {
  if (part.type === "text") return part.text.trim() ? [part.text.trim(), ""] : [];
  if (part.type === "reasoning") {
    if (!withThinking || !part.text.trim()) return [];
    const secs = part.ms ? ` (${Math.max(1, Math.round(part.ms / 1000))} s)` : "";
    return [`<details><summary>Thinking${secs}</summary>`, "", quote(part.text), "", "</details>", ""];
  }
  const status = TOOL_STATUS[part.status] ?? part.status;
  const head = `**Tool: ${part.title}**${part.detail ? ` · ${part.detail}` : ""} · ${status}${part.summary ? ` · ${part.summary}` : ""}`;
  const out: string[] = [head, ""];
  const args = part.arguments.trim();
  if (args && args !== "{}") {
    const f = fence(args);
    out.push(`${f}json`, args, f, "");
  }
  return out;
}

export interface ExportOptions {
  title: string;
  systemPrompt?: string | null;
  /** Include the model's thinking (collapsed). Default true. */
  thinking?: boolean;
}

export function conversationMarkdown(messages: ChatMessage[], opts: ExportOptions): string {
  const withThinking = opts.thinking !== false;
  const lines: string[] = [`# ${opts.title.trim() || "Chat"}`, ""];
  if (opts.systemPrompt?.trim()) lines.push(`**System prompt:** ${opts.systemPrompt.trim()}`, "", "---", "");

  for (const m of messages) {
    const who = m.role === "user" ? "You" : `LocalAide${m.model ? ` · ${m.model}` : ""}`;
    lines.push(`## ${who}${m.error ? " (failed)" : ""}`, "");
    for (const a of m.attachments ?? []) {
      lines.push(`> Attached: ${a.name}${a.error ? ` (${a.error})` : a.truncated ? " (shortened)" : ""}`);
    }
    if (m.attachments?.length) lines.push("");
    if (m.images?.length) lines.push(`> ${m.images.length} image${m.images.length === 1 ? "" : "s"} attached`, "");

    if (m.parts && m.parts.length > 0) {
      for (const p of m.parts) lines.push(...partLines(p, withThinking));
    } else if (m.content.trim()) {
      lines.push(m.content.trim(), "");
    }

    if (m.citations?.length) {
      lines.push("**References**", "");
      for (const c of [...m.citations].sort((a, b) => a.index - b.index)) {
        const link = /^https?:\/\//.test(c.source) ? `[${c.title}](${c.source})` : `${c.title} (${c.source})`;
        lines.push(`${c.index}. ${link}`);
      }
      lines.push("");
    }
  }
  return `${lines.join("\n").trim()}\n`;
}

/** A file name every system accepts. */
export function exportFileName(title: string, extension: "md" | "json"): string {
  const stem = title
    .replace(/[^\p{L}\p{N}_-]+/gu, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 60);
  return `${stem || "chat"}.${extension}`;
}
