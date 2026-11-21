import type { MessagePart, QuestionItem, TodoItem } from "./api/bindings";

/**
 * Builds an answer from the chat stream, in the order things happened:
 * thinking, tool calls, text. The functions change the list in place, so
 * they work on Svelte state directly.
 */

type ReasoningPart = Extract<MessagePart, { type: "reasoning" }> & { started_at?: number };
type ToolPart = Extract<MessagePart, { type: "tool" }>;

/** Close an open thinking block and note how long it took. */
export function closeReasoning(parts: MessagePart[], now: number): void {
  const last = parts.at(-1) as ReasoningPart | undefined;
  if (last?.type === "reasoning" && (last.ms === null || last.ms === undefined)) {
    last.ms = last.started_at !== undefined ? Math.max(0, now - last.started_at) : 0;
  }
}

export function appendText(parts: MessagePart[], text: string, now: number): void {
  if (!text) return;
  closeReasoning(parts, now);
  const last = parts.at(-1);
  if (last?.type === "text") last.text += text;
  else parts.push({ type: "text", text });
}

export function appendReasoning(parts: MessagePart[], text: string, now: number): void {
  if (!text) return;
  const last = parts.at(-1) as ReasoningPart | undefined;
  if (last?.type === "reasoning" && (last.ms === null || last.ms === undefined)) {
    last.text += text;
  } else {
    const part: ReasoningPart = { type: "reasoning", text, ms: null, started_at: now };
    parts.push(part);
  }
}

export function startTool(
  parts: MessagePart[],
  call: { id: string; name: string; title: string; detail: string; group: string; arguments: string },
  now: number,
): void {
  closeReasoning(parts, now);
  if (parts.some((p) => p.type === "tool" && p.id === call.id)) return;
  parts.push({ type: "tool", ...call, status: "running", summary: "", output: "" });
}

/** The model asked the user something (`ask_user`). */
export function addQuestion(
  parts: MessagePart[],
  q: { id: string; question: string; options: string[]; multiple: boolean; items?: QuestionItem[] },
  now: number,
): void {
  closeReasoning(parts, now);
  if (parts.some((p) => p.type === "question" && p.id === q.id)) return;
  parts.push({ type: "question", ...q, answer: null });
}

/** The questions of a question part, in the 0.1.7 shape. Older saved
 * chats have one question with plain options. */
export function questionItems(part: Extract<MessagePart, { type: "question" }>): QuestionItem[] {
  if (part.items && part.items.length > 0) return part.items;
  return [{ header: "", question: part.question, options: part.options.map((label) => ({ label, description: "" })), multiple: part.multiple }];
}

/** The model's newest to-do list replaces the old one. */
export function setTodos(parts: MessagePart[], items: TodoItem[], now = performance.now()): void {
  closeReasoning(parts, now);
  const existing = parts.find((p): p is Extract<MessagePart, { type: "todos" }> => p.type === "todos");
  if (existing) existing.items = items;
  else parts.push({ type: "todos", items });
}

/** A subagent's newest step. */
export function addSubagentStep(parts: MessagePart[], id: string, step: string): void {
  const tool = findTool(parts, id);
  if (!tool) return;
  const steps = tool.steps ?? [];
  if (steps.at(-1) !== step) tool.steps = [...steps, step].slice(-40);
}

/** A skill the model saved or improved, for the note with Undo. */
export function setSkillChange(parts: MessagePart[], change: { id: string; change_id: string; name: string; action: string }): void {
  const tool = findTool(parts, change.id);
  if (tool) tool.skill = { change_id: change.change_id, name: change.name, action: change.action };
}

function findTool(parts: MessagePart[], id: string): ToolPart | undefined {
  return parts.find((p): p is ToolPart => p.type === "tool" && p.id === id);
}

export function setToolStatus(parts: MessagePart[], id: string, status: ToolPart["status"]): void {
  const tool = findTool(parts, id);
  if (tool) tool.status = status;
}

export function finishTool(
  parts: MessagePart[],
  result: { id: string; ok: boolean; summary: string; output: string },
): void {
  const tool = findTool(parts, result.id);
  if (!tool) return;
  tool.summary = result.summary;
  tool.output = result.output;
  if (tool.status === "denied") return;
  tool.status = result.ok ? "done" : "failed";
}

/** Tools still running when an answer stops count as stopped. */
export function settleParts(parts: MessagePart[], now: number): void {
  closeReasoning(parts, now);
  // A thinking block that something else followed is closed too.
  for (const p of parts as ReasoningPart[]) {
    if (p.type === "reasoning" && (p.ms === null || p.ms === undefined)) p.ms = p.started_at !== undefined ? Math.max(0, now - p.started_at) : 0;
  }
  for (const p of parts) {
    if (p.type === "tool" && (p.status === "running" || p.status === "waiting")) {
      p.status = "failed";
      if (!p.summary) p.summary = "Stopped";
    }
  }
}

/** The answer's text, without thinking or tools. */
export function textOf(parts: MessagePart[]): string {
  return parts
    .filter((p): p is Extract<MessagePart, { type: "text" }> => p.type === "text")
    .map((p) => p.text)
    .join("");
}

/** The parts as they are saved: no timers, tool output kept short. */
export function partsForSaving(parts: MessagePart[]): MessagePart[] {
  return parts.map((p) => {
    if (p.type === "reasoning") return { type: "reasoning", text: p.text, ms: p.ms == null ? null : Math.max(0, Math.round(p.ms)) };
    if (p.type === "tool") return { ...p, output: p.output.slice(0, 2000), arguments: p.arguments.slice(0, 2000), steps: p.steps?.slice(-20) };
    return { ...p };
  });
}

/** "Thought for 12 s", "Thought for 1 min 5 s", or "Thinking…" while open. */
export function thinkingLabel(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "Thinking…";
  const s = Math.max(1, Math.round(ms / 1000));
  if (s < 60) return `Thought for ${s} s`;
  return `Thought for ${Math.floor(s / 60)} min ${s % 60} s`;
}

/** Short context sizes for the model picker: 8K, 128K, 1M. */
export function contextLabel(tokens: number | null | undefined): string {
  if (!tokens) return "";
  if (tokens >= 1_000_000) return `${+(tokens / 1_048_576).toFixed(tokens % 1_048_576 === 0 ? 0 : 1)}M`;
  return `${Math.round(tokens / 1024)}K`;
}

/** Plain words for an effort level. */
export function effortLabel(effort: string): string {
  const labels: Record<string, string> = {
    auto: "Auto",
    off: "Off",
    on: "On",
    minimal: "Minimal",
    low: "Low",
    medium: "Medium",
    high: "High",
    xhigh: "Xhigh",
    max: "Max",
    ultra: "Ultra",
  };
  return labels[effort] ?? effort.charAt(0).toUpperCase() + effort.slice(1);
}
