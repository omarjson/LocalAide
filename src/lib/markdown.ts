// A small markdown reader for documents the app ships (privacy policy,
// terms, release notes). It turns text into plain data that Svelte renders
// with {#each}; nothing goes through {@html}, so no markup can be injected.
//
// Supported: # headings, paragraphs, - and 1. lists, | tables |, and inline
// **bold**, `code` and [links](https://...) or bare https:// links.

export type Inline =
  | { kind: "text"; text: string }
  | { kind: "bold"; text: string }
  | { kind: "code"; text: string }
  | { kind: "link"; text: string; href: string };

export type Block =
  | { kind: "heading"; level: 1 | 2 | 3; text: Inline[] }
  | { kind: "paragraph"; text: Inline[] }
  | { kind: "list"; ordered: boolean; items: Inline[][] }
  | { kind: "table"; header: Inline[][]; rows: Inline[][][] };

const INLINE = /(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\((https?:\/\/[^)\s]+)\)|https?:\/\/[^\s)|]+)/g;

export function parseInline(text: string): Inline[] {
  const out: Inline[] = [];
  let last = 0;
  for (const match of text.matchAll(INLINE)) {
    const token = match[0];
    const at = match.index ?? 0;
    if (at > last) out.push({ kind: "text", text: text.slice(last, at) });
    if (token.startsWith("**")) {
      out.push({ kind: "bold", text: token.slice(2, -2) });
    } else if (token.startsWith("`")) {
      out.push({ kind: "code", text: token.slice(1, -1) });
    } else if (token.startsWith("[")) {
      const label = token.slice(1, token.indexOf("]"));
      out.push({ kind: "link", text: label, href: match[2] ?? "" });
    } else {
      // A bare link: keep trailing sentence punctuation out of the address.
      const href = token.replace(/[.,;:]+$/, "");
      out.push({ kind: "link", text: href, href });
      if (href.length < token.length) out.push({ kind: "text", text: token.slice(href.length) });
    }
    last = at + token.length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out;
}

const cells = (line: string): string[] =>
  line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => c.trim());

const isDivider = (line: string): boolean => /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$/.test(line);

export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let paragraph: string[] = [];

  const flush = () => {
    if (paragraph.length) {
      blocks.push({ kind: "paragraph", text: parseInline(paragraph.join(" ")) });
      paragraph = [];
    }
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    if (!trimmed) {
      flush();
      continue;
    }

    const heading = /^(#{1,3})\s+(.*)$/.exec(trimmed);
    if (heading) {
      flush();
      blocks.push({ kind: "heading", level: heading[1].length as 1 | 2 | 3, text: parseInline(heading[2]) });
      continue;
    }

    if (trimmed.startsWith("|") && i + 1 < lines.length && isDivider(lines[i + 1])) {
      flush();
      const header = cells(trimmed).map(parseInline);
      const rows: Inline[][][] = [];
      i += 2;
      while (i < lines.length && lines[i].trim().startsWith("|")) {
        rows.push(cells(lines[i]).map(parseInline));
        i++;
      }
      i--;
      blocks.push({ kind: "table", header, rows });
      continue;
    }

    const bullet = /^([-*]|\d+\.)\s+(.*)$/.exec(trimmed);
    if (bullet) {
      flush();
      const ordered = /\d/.test(bullet[1]);
      const items: Inline[][] = [];
      while (i < lines.length) {
        const m = /^\s*([-*]|\d+\.)\s+(.*)$/.exec(lines[i]);
        if (!m) break;
        items.push(parseInline(m[2]));
        i++;
      }
      i--;
      blocks.push({ kind: "list", ordered, items });
      continue;
    }

    paragraph.push(trimmed);
  }
  flush();
  return blocks;
}
