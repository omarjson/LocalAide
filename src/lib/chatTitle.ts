// Validating a model-generated chat title before it lands on the tab.
//
// A title is a nicety, never a blocker: a bad one is dropped and the next
// completed answer tries again, so the tab keeps its first-line fallback
// instead of wearing the raw first message (e.g. "hi") forever.

/** Strip wrapping quotes and cap the length; null when nothing is left. */
export function cleanTitle(raw: string | null | undefined): string | null {
  if (!raw) return null;
  let title = raw.trim();
  if (
    title.length >= 2 &&
    ((title.startsWith('"') && title.endsWith('"')) ||
      (title.startsWith("'") && title.endsWith("'")))
  ) {
    title = title.slice(1, -1).trim();
  }
  if (!title) return null;
  // A title is a tab label, not a sentence.
  if (title.length > 80) title = title.slice(0, 80).trimEnd();
  return title || null;
}

const normalizeForTitleCompare = (text: string): string =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, "")
    .replace(/\s+/g, " ")
    .trim();

/**
 * True when the title merely echoes the user's question: the same words,
 * the question's first line, or a short question repeated with a word or
 * two added. A real title rephrases; an echo adds nothing over the
 * first-line fallback the tab already shows.
 */
export function titleEchoesQuestion(title: string, question: string): boolean {
  const t = normalizeForTitleCompare(title);
  const q = normalizeForTitleCompare(question);
  if (!t || !q) return true;
  if (t === q) return true;
  const firstLine = normalizeForTitleCompare(question.split("\n")[0]);
  if (t === firstLine) return true;
  // Short questions ("hi", "plan my week"): a title that is just the
  // question, or just a prefix of it, is an echo.
  if (q.length <= 40 && (q.startsWith(t) || t.startsWith(q))) return true;
  return false;
}

/** A title worth putting on the tab. */
export function isGoodTitle(raw: string | null | undefined, question: string): boolean {
  const title = cleanTitle(raw);
  if (!title) return false;
  return !titleEchoesQuestion(title, question);
}
