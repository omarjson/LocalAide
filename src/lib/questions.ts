// Find a question with answer options at the end of an answer, so the chat
// can show buttons instead of making the user type. Conservative: a list
// only counts when a question mark sits right next to it.

export interface ParsedQuestion {
  question: string;
  options: string[];
  multiple: boolean;
}

const LIST_ITEM = /^\s*(?:\d{1,2}[.)]|[-*•]|\(?[a-fA-F][.)])\s+(.+?)\s*$/;
const YES_NO_START =
  /^(do|does|did|should|shall|would|will|can|could|is|are|was|were|have|has|may|want|ok|okay)\b/i;
const MULTIPLE = /\b(which ones|select all|choose any|pick any|all that apply|one or more|any of these)\b/i;

/** Plain text of an option: no markdown marks, no trailing colon or full stop. */
export function cleanOption(raw: string): string {
  return raw
    .replace(/\*\*(.+?)\*\*/g, "$1")
    .replace(/__(.+?)__/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\s*[.:;]\s*$/, "")
    .trim();
}

function lastSentence(line: string): string {
  const parts = line.split(/(?<=[.!])\s+/);
  return parts[parts.length - 1].trim();
}

export function parseQuestion(text: string): ParsedQuestion | null {
  const lines = text
    .replace(/```[\s\S]*?```/g, "")
    .split("\n")
    .map((l) => l.trimEnd());
  while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
  if (lines.length === 0) return null;

  // A question line after the list, as in "1. A\n2. B\nWhich one?".
  let end = lines.length - 1;
  let trailingQuestion: string | null = null;
  if (!LIST_ITEM.test(lines[end]) && lines[end].trim().endsWith("?")) {
    trailingQuestion = lines[end].trim();
    end -= 1;
    while (end >= 0 && !lines[end].trim()) end -= 1;
  }

  const options: string[] = [];
  let i = end;
  while (i >= 0 && LIST_ITEM.test(lines[i])) {
    options.unshift(cleanOption(lines[i].match(LIST_ITEM)![1]));
    i -= 1;
  }
  if (options.length >= 2 && options.length <= 6 && options.every((o) => o.length > 0 && o.length <= 90)) {
    while (i >= 0 && !lines[i].trim()) i -= 1;
    const before = i >= 0 ? lines[i].trim() : "";
    const question = trailingQuestion ?? (before.endsWith("?") || /\?\s*\**:?$/.test(before) ? before : null);
    if (question) {
      const q = question.replace(/\*\*/g, "").replace(/:$/, "").trim();
      return { question: q, options, multiple: MULTIPLE.test(q) };
    }
    return null;
  }

  // A yes/no question as the last sentence.
  const last = lastSentence(lines[lines.length - 1].trim().replace(/\*\*/g, ""));
  if (last.endsWith("?") && last.length <= 200 && YES_NO_START.test(last) && !/\banything else\b/i.test(last) && !/\bor\b/i.test(last)) {
    return { question: last, options: ["Yes", "No"], multiple: false };
  }
  return null;
}
