/**
 * A2UI v1.0 (https://a2ui.org, Apache-2.0 specification) for the chat.
 *
 * The model writes A2UI messages in a ```a2ui block, one JSON envelope per
 * line (a JSON array works too). LocalAide is the renderer: it builds each
 * surface from the Basic Catalog (v1) with its own components, keeps the
 * surface's data model, evaluates bindings and checks, and turns a pressed
 * button into a renderer-to-agent `action` envelope that goes back to the
 * model as the user's next message.
 *
 * This file is the framework-free core: parsing, validation, the surface
 * model, JSON Pointer data binding, the Basic Catalog functions and action
 * building. `A2uiNode.svelte` draws components; `a2ui.svelte.ts` keeps the
 * surfaces of one chat.
 *
 * Not supported, on purpose: `callRendererFunction` and `callAgentFunction`
 * (the chat has no function channel; such envelopes are reported and
 * skipped), and remote media. Images, video and audio load only from
 * `data:` URLs; an http(s) address shows a link the user can open in the
 * browser, because the chat makes no network call the user did not ask for.
 */

export const A2UI_VERSION = "1.0";
export const BASIC_CATALOG_ID = "https://a2ui.org/specification/v1_0/catalogs/basic/catalog.json";
/** Fence language of an action the user sent back from a surface. */
export const EVENT_FENCE = "a2ui-event";

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

export interface A2uiComponent {
  id: string;
  component: string;
  [prop: string]: unknown;
}

export interface Surface {
  id: string;
  catalogId: string | null;
  sendDataModel: boolean;
  components: Record<string, A2uiComponent>;
  data: JsonObject;
  /** Problems found while building the surface, each pointing at its field. */
  problems: A2uiProblem[];
  deleted: boolean;
}

/** A problem with the field it points at, as a JSON Pointer. */
export interface A2uiProblem {
  message: string;
  /** JSON Pointer to the failing field; "/" when the whole block is at fault. */
  path: string;
}

/** Builds a problem; the path defaults to the whole block. */
export const a2uiProblem = (message: string, path = "/"): A2uiProblem => ({ message, path });

export const COMPONENTS = new Set([
  "Text",
  "Image",
  "Icon",
  "Video",
  "AudioPlayer",
  "Row",
  "Column",
  "List",
  "Card",
  "Tabs",
  "Modal",
  "Divider",
  "Button",
  "TextField",
  "CheckBox",
  "ChoicePicker",
  "Slider",
  "DateTimeInput",
]);

const MESSAGE_KEYS = ["createSurface", "updateComponents", "updateDataModel", "deleteSurface", "callRendererFunction", "agentFunctionResponse"] as const;

// UAX #31 identifiers (component ids, names).
const IDENT = /^[\p{XID_Start}_][\p{XID_Continue}]*$/u;
export const isIdentifier = (value: unknown): value is string => typeof value === "string" && IDENT.test(value);

/** A plain copy. JSON, not structuredClone, so Svelte state proxies copy too. */
const clone = <T>(value: T): T => (value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T));

const isObject = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);

// ---------------------------------------------------------------------------
// Parsing

export interface ParsedStream {
  envelopes: Record<string, unknown>[];
  problems: A2uiProblem[];
}

/**
 * Reads a ```a2ui block: JSONL (one envelope per line), a JSON array of
 * envelopes, or a single envelope. Lines that do not parse are reported.
 */
export function parseStream(source: string): ParsedStream {
  const text = source.trim();
  const problems: A2uiProblem[] = [];
  if (!text) return { envelopes: [], problems: [a2uiProblem("The interface block is empty.")] };
  try {
    const whole: unknown = JSON.parse(text);
    if (Array.isArray(whole)) return { envelopes: whole.filter(isObject), problems };
    if (isObject(whole)) return { envelopes: [whole], problems };
  } catch {
    // Not one JSON value: read it line by line.
  }
  const envelopes: Record<string, unknown>[] = [];
  text.split(/\r?\n/).forEach((line, i) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    try {
      const value: unknown = JSON.parse(trimmed);
      if (isObject(value)) envelopes.push(value);
      else problems.push(a2uiProblem(`Line ${i + 1} is not a JSON object.`));
    } catch {
      problems.push(a2uiProblem(`Line ${i + 1} is not valid JSON.`));
    }
  });
  return { envelopes, problems };
}

// ---------------------------------------------------------------------------
// Applying messages

/** Applies one envelope to the surfaces. Returns the surface it created, if any, and any problem. */
export function applyEnvelope(surfaces: Record<string, Surface>, envelope: Record<string, unknown>): { created?: string; problem?: A2uiProblem } {
  if (envelope.version !== A2UI_VERSION) return { problem: a2uiProblem(`Only A2UI version "${A2UI_VERSION}" is supported; a message said ${JSON.stringify(envelope.version ?? null)}.`) };
  const keys = MESSAGE_KEYS.filter((k) => k in envelope);
  if (keys.length !== 1) return { problem: a2uiProblem("Each message needs exactly one of createSurface, updateComponents, updateDataModel or deleteSurface.") };
  const kind = keys[0];
  const body = envelope[kind];
  if (!isObject(body)) return { problem: a2uiProblem(`${kind} must be an object.`, `/${kind}`) };
  if (kind === "callRendererFunction" || kind === "agentFunctionResponse") {
    return { problem: a2uiProblem(`${kind} is not supported in the chat, so it was skipped.`, `/${kind}`) };
  }
  const surfaceId = body.surfaceId;
  if (typeof surfaceId !== "string" || !surfaceId) return { problem: a2uiProblem(`${kind} needs a surfaceId.`, `/${kind}/surfaceId`) };

  if (kind === "createSurface") {
    const existing = surfaces[surfaceId];
    if (existing && !existing.deleted) return { problem: a2uiProblem(`A surface called "${surfaceId}" already exists (INTEGRITY_ERROR). Delete it first or use another surfaceId.`, "/createSurface/surfaceId") };
    if ("theme" in body) return { problem: a2uiProblem("createSurface may not carry a theme in A2UI 1.0; the app's own theme is used.", "/createSurface/theme") };
    const catalogId = typeof body.catalogId === "string" ? body.catalogId : null;
    const surface: Surface = {
      id: surfaceId,
      catalogId,
      sendDataModel: body.sendDataModel === true,
      components: {},
      data: isObject(body.dataModel) ? (clone(body.dataModel) as JsonObject) : {},
      problems: [],
      deleted: false,
    };
    if (catalogId !== BASIC_CATALOG_ID) {
      surface.problems.push(a2uiProblem(catalogId ? `Only the A2UI Basic Catalog is available here, not "${catalogId}".` : "createSurface needs a catalogId (the Basic Catalog).", "/createSurface/catalogId"));
    }
    if (Array.isArray(body.components)) addComponents(surface, body.components, "/createSurface");
    surfaces[surfaceId] = surface;
    return { created: surfaceId };
  }

  const surface = surfaces[surfaceId];
  if (!surface || surface.deleted) return { problem: a2uiProblem(`There is no surface called "${surfaceId}" to ${kind === "deleteSurface" ? "delete" : "update"}.`, `/${kind}/surfaceId`) };
  if (kind === "deleteSurface") {
    surface.deleted = true;
    return {};
  }
  if (kind === "updateComponents") {
    if (!Array.isArray(body.components)) return { problem: a2uiProblem("updateComponents needs a components list.", "/updateComponents/components") };
    addComponents(surface, body.components, "/updateComponents");
    return {};
  }
  // updateDataModel
  if (!("value" in body)) return { problem: a2uiProblem("updateDataModel needs a value (null removes the key).", "/updateDataModel/value") };
  const path = typeof body.path === "string" && body.path ? body.path : "/";
  if (path === "/") {
    surface.data = isObject(body.value) ? (clone(body.value) as JsonObject) : {};
  } else {
    setPointer(surface.data, path, body.value === null ? undefined : (clone(body.value) as JsonValue));
  }
  return {};
}

function addComponents(surface: Surface, list: unknown[], base: string): void {
  list.forEach((raw, i) => {
    if (!isObject(raw)) {
      surface.problems.push(a2uiProblem(`Component ${i + 1} is not an object.`, `${base}/components/${i}`));
      return;
    }
    const { id, component } = raw;
    if (!isIdentifier(id)) {
      surface.problems.push(a2uiProblem(`Component ${i + 1} has an id that is not a plain name (letters, digits and underscores, not starting with a digit): ${JSON.stringify(id ?? null)}.`, `${base}/components/${i}/id`));
      return;
    }
    if (typeof component !== "string" || !COMPONENTS.has(component)) {
      surface.problems.push(a2uiProblem(`"${id}" uses ${JSON.stringify(component ?? null)}, which is not in the Basic Catalog.`, `${base}/components/${i}/component`));
      return;
    }
    if (raw["@catalogId"] !== undefined && raw["@catalogId"] !== BASIC_CATALOG_ID) {
      surface.problems.push(a2uiProblem(`"${id}" asks for another catalog; only the Basic Catalog is available.`, `${base}/components/${i}/@catalogId`));
      return;
    }
    surface.components[id] = raw as A2uiComponent;
  });
}

/** Builds surfaces from one block. */
export function buildSurfaces(source: string, surfaces: Record<string, Surface> = {}): { surfaces: Record<string, Surface>; created: string[]; problems: A2uiProblem[] } {
  const parsed = parseStream(source);
  const created: string[] = [];
  const problems = [...parsed.problems];
  for (const envelope of parsed.envelopes) {
    const result = applyEnvelope(surfaces, envelope);
    if (result.created) created.push(result.created);
    if (result.problem) problems.push(result.problem);
  }
  return { surfaces, created, problems };
}

// ---------------------------------------------------------------------------
// JSON Pointer (RFC 6901)

const unescape = (token: string) => token.replace(/~1/g, "/").replace(/~0/g, "~");

export function pointerTokens(path: string): string[] {
  if (!path || path === "/") return [];
  return path.replace(/^\//, "").split("/").map(unescape);
}

/** An absolute path for `path`, resolved against the collection scope `base`. */
export function absolutePath(path: string, base: string): string {
  if (path.startsWith("/")) return path;
  const root = base === "/" ? "" : base.replace(/\/$/, "");
  return `${root}/${path}`;
}

export function getPointer(data: unknown, path: string): unknown {
  let node: unknown = data;
  for (const token of pointerTokens(path)) {
    if (Array.isArray(node)) node = node[Number(token)];
    else if (isObject(node)) node = node[token];
    else return undefined;
  }
  return node;
}

/** Sets (or, with `undefined`, removes) the value at `path`, creating containers on the way. */
export function setPointer(data: JsonObject, path: string, value: JsonValue | undefined): void {
  const tokens = pointerTokens(path);
  if (tokens.length === 0) return;
  let node: unknown = data;
  for (let i = 0; i < tokens.length - 1; i++) {
    const token = tokens[i];
    const next = tokens[i + 1];
    const container = node as Record<string, unknown> | unknown[];
    const key = Array.isArray(container) ? Number(token) : token;
    let child = (container as Record<string | number, unknown>)[key];
    if (!child || typeof child !== "object") {
      child = /^\d+$/.test(next) ? [] : {};
      (container as Record<string | number, unknown>)[key] = child;
    }
    node = child;
  }
  const last = tokens[tokens.length - 1];
  if (Array.isArray(node)) {
    const index = last === "-" ? node.length : Number(last);
    if (value === undefined) node.splice(index, 1);
    else node[index] = value;
  } else if (isObject(node)) {
    if (value === undefined) delete node[last];
    else node[last] = value;
  }
}

// ---------------------------------------------------------------------------
// Dynamic values and functions

/** Where a value is evaluated: the surface data, the collection scope and its index. */
export interface Scope {
  data: JsonObject;
  base: string;
  index: number | null;
}

export const rootScope = (data: JsonObject): Scope => ({ data, base: "/", index: null });

const isBinding = (value: unknown): value is { "@path": string } => isObject(value) && typeof value["@path"] === "string" && Object.keys(value).length === 1;
const isCall = (value: unknown): value is { "@call": string; args?: Record<string, unknown> } => isObject(value) && typeof value["@call"] === "string";

/** Resolves a literal, a `{"@path"}` binding or a `{"@call"}` function call. */
export function resolve(value: unknown, scope: Scope, activation = false): unknown {
  if (isBinding(value)) return getPointer(scope.data, absolutePath(value["@path"], scope.base));
  if (isCall(value)) return callFunction(value["@call"], value.args ?? {}, scope, activation);
  if (Array.isArray(value)) return value;
  return value;
}

/** The path a two-way input writes to, or null when the value is a literal. */
export function boundPath(value: unknown, scope: Scope): string | null {
  return isBinding(value) ? absolutePath(value["@path"], scope.base) : null;
}

export const toText = (value: unknown): string => {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
};

/** DynamicBoolean coercion (spec: "Any value → boolean"). */
export function toBoolean(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  if (isObject(value) && typeof value.valid === "boolean") return value.valid;
  if (typeof value === "string") return value.toLowerCase() === "true";
  if (typeof value === "number") return value !== 0;
  return false;
}

export interface ValidationResult {
  valid: boolean;
  message?: string;
  code?: string;
  severity?: "error" | "warning" | "info";
}

const valid = (ok: boolean): ValidationResult => ({ valid: ok });

/** The spec's static safety rejection tokens (a2ui-core names). */
export type PatternRejection = "PATTERN_TOO_LONG" | "NESTED_QUANTIFIER" | "OVERLAPPING_ALTERNATION" | "ADJACENT_OVERLAP" | "INVALID_PATTERN";

/**
 * Checks a pattern against the A2UI regex dialect and its four static
 * safety rules (Basic Catalog Guide, `regex`): 1) at most 256 code points;
 * 2) no quantifier with upper bound above 1 over a subexpression holding
 * an unbounded quantifier; 3) no unbounded quantifier over an alternation
 * whose branches can start with the same character; 4) no two adjacent
 * unbounded-quantified atoms with overlapping character sets. A pattern
 * outside the dialect or failing a rule is rejected with `EXPRESSION_ERROR`
 * before matching; only a value over 4,096 code points is a silent
 * non-match (handled by the `regex` function, not here).
 */
export function checkPattern(pattern: string): { regex: RegExp | null; rejection?: PatternRejection } {
  if ([...pattern].length > 256) return { regex: null, rejection: "PATTERN_TOO_LONG" };
  const ast = new RegexParser(pattern).parse();
  if (!ast) return { regex: null, rejection: "INVALID_PATTERN" };
  const rejection = checkRegexSafety(ast);
  if (rejection) return { regex: null, rejection };
  try {
    return { regex: new RegExp(pattern, "u") };
  } catch {
    return { regex: null, rejection: "INVALID_PATTERN" };
  }
}

/** The compiled pattern, or null when the dialect or safety rules refuse it. */
export function safePattern(pattern: string): RegExp | null {
  return checkPattern(pattern).regex;
}

// --- Regex parser: a small AST over the ECMAScript dialect, enough to
// run the static safety rules. The dialect itself (no backreferences,
// lookbehind, named groups, \p/\P, \cX, octal escapes, inline flags) is
// enforced while parsing; `new RegExp` at the end is the final arbiter.

/** A character set as inclusive code-point ranges, or its complement. */
type CharSet = { kind: "any" } | { kind: "ranges"; ranges: [number, number][] } | { kind: "neg"; ranges: [number, number][] };
/** A first-set is a disjunction of character sets. */
type FirstSet = CharSet[];

const ANY_SET: CharSet = { kind: "any" };
const EMPTY_SET: CharSet = { kind: "ranges", ranges: [] };
const DIGITS: [number, number][] = [[48, 57]];
const WORD: [number, number][] = [
  [48, 57],
  [65, 90],
  [95, 95],
  [97, 122],
];
const SPACE: [number, number][] = [
  [9, 13],
  [32, 32],
  [160, 160],
  [5760, 5760],
  [8192, 8202],
  [8232, 8233],
  [8239, 8239],
  [8287, 8287],
  [12288, 12288],
  [65279, 65279],
];

const isEmptySet = (s: CharSet): boolean => s.kind === "ranges" && s.ranges.length === 0;

function rangesOverlap(a: [number, number][], b: [number, number][]): boolean {
  for (const [a0, a1] of a) for (const [b0, b1] of b) if (a0 <= b1 && b0 <= a1) return true;
  return false;
}

/** Is every interval of `inner` fully covered by `outer`? */
function rangesCovered(inner: [number, number][], outer: [number, number][]): boolean {
  const sorted = [...outer].sort((x, y) => x[0] - y[0] || x[1] - y[1]);
  return inner.every(([lo, hi]) => {
    let at = lo;
    for (const [o0, o1] of sorted) {
      if (o0 > at) break;
      if (o1 >= at) at = Math.max(at, o1 + 1);
      if (at > hi) return true;
    }
    return at > hi;
  });
}

function setsOverlap(a: CharSet, b: CharSet): boolean {
  if (isEmptySet(a) || isEmptySet(b)) return false;
  if (a.kind === "any" || b.kind === "any") return true;
  if (a.kind === "ranges" && b.kind === "ranges") return rangesOverlap(a.ranges, b.ranges);
  if (a.kind === "neg" && b.kind === "neg") return true;
  const [ranges, neg] = a.kind === "ranges" ? [a, b] : [b, a];
  return !rangesCovered(ranges.ranges, (neg as { kind: "neg"; ranges: [number, number][] }).ranges);
}

const firstSetsOverlap = (a: FirstSet, b: FirstSet): boolean => a.some((x) => b.some((y) => setsOverlap(x, y)));

type RegexNode =
  | { type: "seq"; items: RegexNode[] }
  | { type: "alt"; branches: RegexNode[] }
  | { type: "group"; child: RegexNode }
  | { type: "assert"; child: RegexNode }
  | { type: "char"; cp: number }
  | { type: "dot" }
  | { type: "set"; first: FirstSet }
  | { type: "anchor" }
  | { type: "quant"; child: RegexNode; min: number; max: number | null };

class RegexParser {
  private pos = 0;
  private readonly src: string;
  constructor(src: string) {
    this.src = src;
  }

  parse(): RegexNode | null {
    const node = this.parseAlt();
    return node && this.pos === this.src.length ? node : null;
  }

  private peek(): string | undefined {
    return this.src[this.pos];
  }

  private parseAlt(): RegexNode | null {
    const first = this.parseSeq();
    if (!first) return null;
    const branches: RegexNode[] = [first];
    while (this.peek() === "|") {
      this.pos++;
      const next = this.parseSeq();
      if (!next) return null;
      branches.push(next);
    }
    return branches.length === 1 ? branches[0] : { type: "alt", branches };
  }

  private parseSeq(): RegexNode | null {
    const items: RegexNode[] = [];
    while (this.pos < this.src.length && this.peek() !== "|" && this.peek() !== ")") {
      const item = this.parseItem();
      if (!item) return null;
      items.push(item);
    }
    return { type: "seq", items };
  }

  private parseItem(): RegexNode | null {
    let atom = this.parseAtom();
    if (!atom) return null;
    const q = this.parseQuantifier();
    if (q) atom = { type: "quant", child: atom, min: q.min, max: q.max };
    return atom;
  }

  private parseQuantifier(): { min: number; max: number | null } | null {
    const c = this.peek();
    const lazy = () => {
      if (this.peek() === "?") this.pos++;
    };
    if (c === "*") {
      this.pos++;
      lazy();
      return { min: 0, max: null };
    }
    if (c === "+") {
      this.pos++;
      lazy();
      return { min: 1, max: null };
    }
    if (c === "?") {
      this.pos++;
      lazy();
      return { min: 0, max: 1 };
    }
    if (c === "{") {
      const m = /^\{(\d+)(?:,(\d*))?\}/.exec(this.src.slice(this.pos));
      if (!m) return null;
      this.pos += m[0].length;
      lazy();
      const min = Number(m[1]);
      const max = m[2] === undefined ? min : m[2] === "" ? null : Number(m[2]);
      if (max !== null && max < min) return null;
      return { min, max };
    }
    return null;
  }

  private parseAtom(): RegexNode | null {
    const c = this.peek();
    if (c === undefined) return null;
    if (c === "(") return this.parseGroup();
    if (c === "[") return this.parseClass();
    if (c === ".") {
      this.pos++;
      return { type: "dot" };
    }
    if (c === "^" || c === "$") {
      this.pos++;
      return { type: "anchor" };
    }
    if (c === "\\") return this.parseEscape();
    if (c === "*" || c === "+" || c === "?") return null;
    if (c === "{") {
      // A valid quantifier here is a syntax error in Unicode mode; otherwise a literal.
      const save = this.pos;
      if (this.parseQuantifier()) return null;
      this.pos = save + 1;
      return { type: "char", cp: "{".codePointAt(0)! };
    }
    const cp = this.src.codePointAt(this.pos)!;
    this.pos += cp > 0xffff ? 2 : 1;
    return { type: "char", cp };
  }

  private parseGroup(): RegexNode | null {
    this.pos++; // consume "("
    let assert = false;
    if (this.peek() === "?") {
      const next = this.src[this.pos + 1];
      if (next === ":") this.pos += 2;
      else if (next === "=" || next === "!") {
        this.pos += 2;
        assert = true;
      } else return null; // (?<, (?i, (?P — outside the dialect.
    }
    const child = this.parseAlt();
    if (!child || this.peek() !== ")") return null;
    this.pos++;
    return assert ? { type: "assert", child } : { type: "group", child };
  }

  /** Parses an escape outside a class. Returns null outside the dialect. */
  private parseEscape(): RegexNode | null {
    this.pos++; // consume "\"
    const e = this.peek();
    if (e === undefined) return null;
    const set = (ranges: [number, number][]): RegexNode => ({ type: "set", first: [{ kind: "ranges", ranges }] });
    const neg = (ranges: [number, number][]): RegexNode => ({ type: "set", first: [{ kind: "neg", ranges }] });
    if (e === "d") {
      this.pos++;
      return set(DIGITS);
    }
    if (e === "D") {
      this.pos++;
      return neg(DIGITS);
    }
    if (e === "w") {
      this.pos++;
      return set(WORD);
    }
    if (e === "W") {
      this.pos++;
      return neg(WORD);
    }
    if (e === "s") {
      this.pos++;
      return set(SPACE);
    }
    if (e === "S") {
      this.pos++;
      return neg(SPACE);
    }
    if (e === "b" || e === "B") {
      this.pos++;
      return { type: "anchor" };
    }
    if (e === "n") {
      this.pos++;
      return { type: "char", cp: 10 };
    }
    if (e === "r") {
      this.pos++;
      return { type: "char", cp: 13 };
    }
    if (e === "t") {
      this.pos++;
      return { type: "char", cp: 9 };
    }
    if (e === "v") {
      this.pos++;
      return { type: "char", cp: 11 };
    }
    if (e === "f") {
      this.pos++;
      return { type: "char", cp: 12 };
    }
    if (e === "0") {
      // A null escape; \0 followed by a digit is a legacy octal escape.
      if (/[0-9]/.test(this.src[this.pos + 1] ?? "")) return null;
      this.pos++;
      return { type: "char", cp: 0 };
    }
    if (e === "x" || e === "u") {
      const cp = this.parseUnicodeEscape();
      if (cp === null) return null;
      return { type: "char", cp };
    }
    if (/[1-9]/.test(e) || e === "k" || e === "p" || e === "P" || e === "c") return null;
    const cp = this.src.codePointAt(this.pos);
    if (cp === undefined) return null;
    this.pos += cp > 0xffff ? 2 : 1;
    return { type: "char", cp };
  }

  /** Parses \xHH, \uHHHH or \u{...} after the backslash and letter. */
  private parseUnicodeEscape(): number | null {
    const letter = this.src[this.pos];
    this.pos++;
    if (letter === "x") {
      const hex = this.src.slice(this.pos, this.pos + 2);
      if (!/^[0-9A-Fa-f]{2}$/.test(hex)) return null;
      this.pos += 2;
      return Number.parseInt(hex, 16);
    }
    if (this.peek() === "{") {
      const end = this.src.indexOf("}", this.pos);
      if (end < 0) return null;
      const hex = this.src.slice(this.pos + 1, end);
      if (!/^[0-9A-Fa-f]+$/.test(hex)) return null;
      const cp = Number.parseInt(hex, 16);
      if (cp > 0x10ffff) return null;
      this.pos = end + 1;
      return cp;
    }
    const hex = this.src.slice(this.pos, this.pos + 4);
    if (!/^[0-9A-Fa-f]{4}$/.test(hex)) return null;
    this.pos += 4;
    return Number.parseInt(hex, 16);
  }

  private parseClass(): RegexNode | null {
    this.pos++; // consume "["
    let negated = false;
    if (this.peek() === "^") {
      negated = true;
      this.pos++;
    }
    const ranges: [number, number][] = [];
    const negSets: [number, number][][] = [];
    let first = true;
    while (this.pos < this.src.length) {
      const c = this.peek();
      if (c === "]" && !first) {
        this.pos++;
        return { type: "set", first: classFirstSet(negated, ranges, negSets) };
      }
      first = false;
      const atom = this.parseClassAtom();
      if (!atom) return null;
      if (atom.kind === "neg") {
        negSets.push(atom.ranges);
        continue;
      }
      if (this.peek() === "-" && this.pos + 1 < this.src.length && this.src[this.pos + 1] !== "]") {
        if (atom.ranges.length !== 1 || atom.ranges[0][0] !== atom.ranges[0][1]) return null;
        this.pos++;
        const hi = this.parseClassAtom();
        if (!hi || hi.kind !== "ranges" || hi.ranges.length !== 1 || hi.ranges[0][0] !== hi.ranges[0][1]) return null;
        const lo = atom.ranges[0][0];
        const hiCp = hi.ranges[0][0];
        if (lo > hiCp) return null;
        ranges.push([lo, hiCp]);
      } else {
        ranges.push(...atom.ranges);
      }
    }
    return null; // unterminated class
  }

  /** One class item: a single char, a positive escape set, or a negated escape set. */
  private parseClassAtom(): { kind: "ranges" | "neg"; ranges: [number, number][] } | null {
    const c = this.peek();
    if (c === undefined) return null;
    if (c === "\\") {
      this.pos++;
      const e = this.peek();
      if (e === undefined) return null;
      const set = (ranges: [number, number][]): { kind: "ranges"; ranges: [number, number][] } => ({ kind: "ranges", ranges });
      const neg = (ranges: [number, number][]): { kind: "neg"; ranges: [number, number][] } => ({ kind: "neg", ranges });
      if (e === "d") {
        this.pos++;
        return set(DIGITS);
      }
      if (e === "D") {
        this.pos++;
        return neg(DIGITS);
      }
      if (e === "w") {
        this.pos++;
        return set(WORD);
      }
      if (e === "W") {
        this.pos++;
        return neg(WORD);
      }
      if (e === "s") {
        this.pos++;
        return set(SPACE);
      }
      if (e === "S") {
        this.pos++;
        return neg(SPACE);
      }
      if (e === "b") {
        this.pos++;
        return set([[8, 8]]); // backspace inside a class
      }
      if (e === "n") {
        this.pos++;
        return set([[10, 10]]);
      }
      if (e === "r") {
        this.pos++;
        return set([[13, 13]]);
      }
      if (e === "t") {
        this.pos++;
        return set([[9, 9]]);
      }
      if (e === "v") {
        this.pos++;
        return set([[11, 11]]);
      }
      if (e === "f") {
        this.pos++;
        return set([[12, 12]]);
      }
      if (e === "0") {
        if (/[0-9]/.test(this.src[this.pos + 1] ?? "")) return null;
        this.pos++;
        return set([[0, 0]]);
      }
      if (e === "x" || e === "u") {
        const cp = this.parseUnicodeEscape();
        if (cp === null) return null;
        return set([[cp, cp]]);
      }
      if (/[1-9]/.test(e) || e === "k" || e === "p" || e === "P" || e === "c") return null;
      const cp = this.src.codePointAt(this.pos);
      if (cp === undefined) return null;
      this.pos += cp > 0xffff ? 2 : 1;
      return set([[cp, cp]]);
    }
    const cp = this.src.codePointAt(this.pos)!;
    this.pos += cp > 0xffff ? 2 : 1;
    return { kind: "ranges", ranges: [[cp, cp]] };
  }
}

/** The first-set of a character class, as a disjunction of character sets. */
function classFirstSet(negated: boolean, ranges: [number, number][], negSets: [number, number][][]): FirstSet {
  if (!negated) {
    const out: FirstSet = [];
    if (ranges.length > 0) out.push({ kind: "ranges", ranges });
    for (const r of negSets) out.push({ kind: "neg", ranges: r });
    return out.length > 0 ? out : [EMPTY_SET];
  }
  // A negated class holding a negated escape (e.g. [^\D]) is a union of
  // complements; over-approximate it as "any" rather than under-checking.
  if (negSets.length > 0) return [ANY_SET];
  return [{ kind: "neg", ranges }];
}

/** The characters a node can start with. */
function firstSet(node: RegexNode): FirstSet {
  switch (node.type) {
    case "char":
      return [{ kind: "ranges", ranges: [[node.cp, node.cp]] }];
    case "dot":
      return [ANY_SET];
    case "set":
      return node.first;
    case "anchor":
    case "assert":
      return [];
    case "group":
    case "quant":
      return firstSet(node.child);
    case "seq": {
      const out: FirstSet = [];
      for (const item of node.items) {
        out.push(...firstSet(item));
        if (!nullable(item)) break;
      }
      return out;
    }
    case "alt":
      return node.branches.flatMap(firstSet);
  }
}

/** Can the node match the empty string? */
function nullable(node: RegexNode): boolean {
  switch (node.type) {
    case "char":
    case "dot":
    case "set":
      return false;
    case "anchor":
    case "assert":
      return true;
    case "group":
      return nullable(node.child);
    case "quant":
      return node.min === 0 || nullable(node.child);
    case "seq":
      return node.items.every(nullable);
    case "alt":
      return node.branches.some(nullable);
  }
}

/** Does the subtree hold an unbounded quantifier (`*`, `+`, `{n,}`)? */
function containsUnboundedQuant(node: RegexNode): boolean {
  switch (node.type) {
    case "quant":
      return node.max === null || containsUnboundedQuant(node.child);
    case "seq":
      return node.items.some(containsUnboundedQuant);
    case "alt":
      return node.branches.some(containsUnboundedQuant);
    case "group":
    case "assert":
      return containsUnboundedQuant(node.child);
    default:
      return false;
  }
}

/** Unwraps groups to the node they hold. */
function unwrapGroup(node: RegexNode): RegexNode {
  let n = node;
  while (n.type === "group") n = n.child;
  return n;
}

/** A quantified atom, unwrapping a group whose only content is one. */
function asQuantifiedAtom(node: RegexNode): { type: "quant"; child: RegexNode; min: number; max: number | null } | null {
  if (node.type === "quant") return node;
  if (node.type === "group" || node.type === "assert") {
    const inner = unwrapSingle(node.child);
    return inner ? asQuantifiedAtom(inner) : null;
  }
  return null;
}

/** A sequence with exactly one item unwraps to it; anything else stays. */
function unwrapSingle(node: RegexNode): RegexNode | null {
  if (node.type === "seq") return node.items.length === 1 ? node.items[0] : null;
  if (node.type === "group") return unwrapSingle(node.child);
  return node;
}

/**
 * Rule 4 for one concatenation: two unbounded-quantified atoms with
 * overlapping first sets, separated only by zero-width assertions or items
 * that can match the empty string. A consuming item resets the run; a
 * non-nullable quantified atom is itself consuming, so it replaces the run
 * after being checked against it.
 */
function checkSequence(items: RegexNode[]): PatternRejection | undefined {
  let run: FirstSet[] = [];
  for (const item of items) {
    const q = asQuantifiedAtom(item);
    if (q && q.max === null) {
      const set = firstSet(q.child);
      if (run.some((seen) => firstSetsOverlap(seen, set))) return "ADJACENT_OVERLAP";
      run = nullable(q) ? [...run, set] : [set];
      continue;
    }
    if (item.type === "anchor" || item.type === "assert" || nullable(item)) continue;
    run = [];
  }
  return undefined;
}

/** Runs safety rules 2–4 over the AST. Rule 1 (length) is checked by the caller. */
function checkRegexSafety(node: RegexNode): PatternRejection | undefined {
  switch (node.type) {
    case "quant": {
      // Rule 2: a quantifier with upper bound above 1 over a subexpression
      // holding an unbounded quantifier.
      if ((node.max === null || node.max > 1) && containsUnboundedQuant(node.child)) return "NESTED_QUANTIFIER";
      // Rule 3: an unbounded quantifier over an alternation whose branches
      // can start with the same character (after unwrapping groups).
      if (node.max === null) {
        const inner = unwrapGroup(node.child);
        if (inner.type === "alt" && inner.branches.length >= 2) {
          const firsts = inner.branches.map(firstSet);
          for (let i = 0; i < firsts.length; i++) {
            for (let j = i + 1; j < firsts.length; j++) {
              if (firstSetsOverlap(firsts[i], firsts[j])) return "OVERLAPPING_ALTERNATION";
            }
          }
        }
      }
      return checkRegexSafety(node.child);
    }
    case "seq": {
      const bad = checkSequence(node.items);
      if (bad) return bad;
      for (const item of node.items) {
        const found = checkRegexSafety(item);
        if (found) return found;
      }
      return undefined;
    }
    case "alt": {
      for (const branch of node.branches) {
        const found = checkRegexSafety(branch);
        if (found) return found;
      }
      return undefined;
    }
    case "group":
    case "assert":
      return checkRegexSafety(node.child);
    default:
      return undefined;
  }
}

function formatDate(value: unknown, pattern: string): string {
  const date = new Date(toText(value));
  if (Number.isNaN(date.getTime())) return "";
  const pad = (n: number, width = 2) => String(n).padStart(width, "0");
  const month = (style: "long" | "short") => date.toLocaleString(undefined, { month: style });
  const day = (style: "long" | "short") => date.toLocaleString(undefined, { weekday: style });
  const tokens: Record<string, () => string> = {
    yyyy: () => String(date.getFullYear()),
    YYYY: () => String(date.getFullYear()),
    YY: () => pad(date.getFullYear() % 100),
    yy: () => pad(date.getFullYear() % 100),
    MMMM: () => month("long"),
    MMM: () => month("short"),
    MM: () => pad(date.getMonth() + 1),
    M: () => String(date.getMonth() + 1),
    dd: () => pad(date.getDate()),
    d: () => String(date.getDate()),
    EEEE: () => day("long"),
    E: () => day("short"),
    HH: () => pad(date.getHours()),
    H: () => String(date.getHours()),
    hh: () => pad(date.getHours() % 12 || 12),
    h: () => String(date.getHours() % 12 || 12),
    mm: () => pad(date.getMinutes()),
    ss: () => pad(date.getSeconds()),
    a: () => (date.getHours() < 12 ? "AM" : "PM"),
  };
  return pattern.replace(/'([^']*)'|yyyy|YYYY|yy|YY|MMMM|MMM|MM|M|dd|d|EEEE|E|HH|H|hh|h|mm|ss|a/g, (match, quoted: string | undefined) => (quoted !== undefined ? quoted : (tokens[match]?.() ?? match)));
}

/** Runs a Basic Catalog function. `activation` is true inside a user's press. */
export function callFunction(name: string, rawArgs: Record<string, unknown>, scope: Scope, activation = false): unknown {
  const arg = (key: string) => resolve(rawArgs[key], scope, activation);
  switch (name) {
    case "@index":
      return scope.index === null ? undefined : scope.index + (Number(arg("offset")) || 0);
    case "required": {
      const v = arg("value");
      return valid(!(v === null || v === undefined || v === "" || (Array.isArray(v) && v.length === 0)));
    }
    case "regex": {
      const v = toText(arg("value"));
      // A value over the cap is a silent non-match, never an error.
      if ([...v].length > 4096) return valid(false);
      const checked = checkPattern(toText(arg("pattern")));
      if (!checked.regex) return { valid: false, code: "EXPRESSION_ERROR", message: `The pattern is not allowed (${checked.rejection}).` };
      return valid(checked.regex.test(v));
    }
    case "length": {
      const n = [...toText(arg("value"))].length;
      const min = rawArgs.min === undefined ? null : Number(arg("min"));
      const max = rawArgs.max === undefined ? null : Number(arg("max"));
      return valid((min === null || n >= min) && (max === null || n <= max));
    }
    case "numeric": {
      const raw = arg("value");
      const n = typeof raw === "number" ? raw : Number(toText(raw).trim());
      if (toText(raw).trim() === "" || Number.isNaN(n)) return valid(false);
      const min = rawArgs.min === undefined ? null : Number(arg("min"));
      const max = rawArgs.max === undefined ? null : Number(arg("max"));
      return valid((min === null || n >= min) && (max === null || n <= max));
    }
    case "email":
      return valid(/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(toText(arg("value"))));
    case "formatString":
      return formatString(toText(rawArgs.value === undefined ? "" : isBinding(rawArgs.value) || isCall(rawArgs.value) ? arg("value") : rawArgs.value), scope);
    case "formatNumber": {
      const decimals = rawArgs.decimals === undefined ? undefined : Number(arg("decimals"));
      return new Intl.NumberFormat(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals, useGrouping: arg("grouping") !== false }).format(Number(arg("value")) || 0);
    }
    case "formatCurrency": {
      const decimals = rawArgs.decimals === undefined ? undefined : Number(arg("decimals"));
      try {
        return new Intl.NumberFormat(undefined, { style: "currency", currency: toText(arg("currency")) || "USD", minimumFractionDigits: decimals, maximumFractionDigits: decimals, useGrouping: arg("grouping") !== false }).format(Number(arg("value")) || 0);
      } catch {
        return toText(arg("value"));
      }
    }
    case "formatDate":
      return formatDate(arg("value"), toText(arg("format")) || "yyyy-MM-dd");
    case "pluralize": {
      const n = Number(arg("value")) || 0;
      const category = new Intl.PluralRules().select(n);
      const pick = rawArgs[category] !== undefined ? arg(category) : arg("other");
      return toText(pick);
    }
    case "and":
      return Array.isArray(rawArgs.values) && rawArgs.values.every((v) => toBoolean(resolve(v, scope, activation)));
    case "or":
      return Array.isArray(rawArgs.values) && rawArgs.values.some((v) => toBoolean(resolve(v, scope, activation)));
    case "not":
      return !toBoolean(arg("value"));
    case "openUrl":
      // Runs only from an action; see `localAction`.
      return undefined;
    default:
      return undefined;
  }
}

/** `formatString`: `${/abs/path}`, `${relative}`, `${fn(arg: value)}`; `\${` is a literal. */
export function formatString(template: string, scope: Scope): string {
  let out = "";
  let i = 0;
  while (i < template.length) {
    if (template.startsWith("\\${", i)) {
      out += "${";
      i += 3;
      continue;
    }
    if (template.startsWith("${", i)) {
      const end = matchingBrace(template, i + 2);
      if (end < 0) {
        out += template.slice(i);
        break;
      }
      out += toText(evalExpression(template.slice(i + 2, end).trim(), scope));
      i = end + 1;
      continue;
    }
    out += template[i];
    i++;
  }
  return out;
}

function matchingBrace(text: string, from: number): number {
  let depth = 1;
  let quote: string | null = null;
  for (let i = from; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === quote) quote = null;
      continue;
    }
    if (c === "'" || c === '"') quote = c;
    else if (c === "{") depth++;
    else if (c === "}" && --depth === 0) return i;
  }
  return -1;
}

function evalExpression(expr: string, scope: Scope): unknown {
  if (expr.startsWith("${") && expr.endsWith("}")) return evalExpression(expr.slice(2, -1).trim(), scope);
  if (/^(['"]).*\1$/.test(expr)) return expr.slice(1, -1);
  if (expr === "true" || expr === "false") return expr === "true";
  if (expr === "null") return null;
  if (/^-?\d+(\.\d+)?$/.test(expr)) return Number(expr);
  const call = /^(@?[\p{XID_Start}_][\p{XID_Continue}]*)\((.*)\)$/su.exec(expr);
  if (call) {
    const args: Record<string, unknown> = {};
    for (const part of splitArgs(call[2])) {
      const colon = part.indexOf(":");
      if (colon < 0) continue;
      const key = part.slice(0, colon).trim();
      args[key] = { "@literal": evalExpression(part.slice(colon + 1).trim(), scope) };
    }
    // Arguments are already evaluated; hand them over as literals.
    const literalArgs = Object.fromEntries(Object.entries(args).map(([k, v]) => [k, (v as { "@literal": unknown })["@literal"]]));
    return callFunction(call[1], literalArgs, scope);
  }
  return getPointer(scope.data, absolutePath(expr, scope.base));
}

function splitArgs(text: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let quote: string | null = null;
  let start = 0;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      if (c === quote) quote = null;
      continue;
    }
    if (c === "'" || c === '"') quote = c;
    else if (c === "(" || c === "{") depth++;
    else if (c === ")" || c === "}") depth--;
    else if (c === "," && depth === 0) {
      parts.push(text.slice(start, i));
      start = i + 1;
    }
  }
  if (text.slice(start).trim()) parts.push(text.slice(start));
  return parts;
}

// ---------------------------------------------------------------------------
// Checks and actions

/** The first failing `@checks` rule, or a pass. */
export function runChecks(component: A2uiComponent, scope: Scope): ValidationResult {
  const checks = component["@checks"];
  if (!Array.isArray(checks)) return { valid: true };
  for (const check of checks) {
    if (!isObject(check)) continue;
    const result = resolve(check.condition, scope);
    const message = typeof check.message === "string" ? check.message : undefined;
    if (isObject(result) && typeof result.valid === "boolean") {
      if (!result.valid) return { valid: false, message: typeof result.message === "string" ? result.message : message, severity: "error" };
    } else if (!toBoolean(result)) {
      return { valid: false, message, severity: "error" };
    }
  }
  return { valid: true };
}

/** Resolves an action context: literal values stay, bindings and calls are evaluated. */
function resolveContext(context: unknown, scope: Scope): JsonObject {
  if (!isObject(context)) return {};
  const out: JsonObject = {};
  for (const [key, value] of Object.entries(context)) {
    const resolved = resolve(value, scope, true);
    out[key] = resolved === undefined ? null : (clone(resolved) as JsonValue);
  }
  return out;
}

export interface ActionEvent {
  /** What the user sees as their message. */
  text: string;
  /** The full chat message: the text plus the A2UI envelopes in a fence. */
  message: string;
}

/**
 * Builds the renderer-to-agent `action` envelope for a pressed component,
 * and, when the surface asked for it, the renderer data model. `label` is
 * the button's own text, for the readable line.
 */
export function buildAction(surface: Surface, component: A2uiComponent, scope: Scope, label: string, now = new Date()): ActionEvent | null {
  const action = component.action;
  if (!isObject(action) || !isObject(action.event)) return null;
  const event = action.event;
  if (typeof event.name !== "string" || !event.name) return null;
  const userMessage = event.userMessage === undefined ? "" : toText(resolve(event.userMessage, scope, true)).trim();
  const envelope = {
    version: A2UI_VERSION,
    action: {
      name: event.name,
      surfaceId: surface.id,
      sourceComponentId: component.id,
      timestamp: now.toISOString(),
      ...(userMessage ? { userMessage } : {}),
      context: resolveContext(event.context, scope),
    },
  };
  const lines = [JSON.stringify(envelope)];
  if (surface.sendDataModel) lines.push(JSON.stringify({ version: A2UI_VERSION, surfaces: { [surface.id]: surface.data } }));
  const text = userMessage || (label ? `I pressed “${label}”.` : `I used the “${event.name}” action.`);
  return { text, message: `${text}\n\n\`\`\`${EVENT_FENCE}\n${lines.join("\n")}\n\`\`\`` };
}

/** A local `functionCall` action. Only `openUrl` with an http(s) address does anything. */
export function localAction(component: A2uiComponent, scope: Scope): { openUrl: string } | null {
  const action = component.action;
  if (!isObject(action) || !isCall(action.functionCall)) return null;
  const call = action.functionCall;
  if (call["@call"] !== "openUrl") return null;
  const url = toText(resolve(call.args?.url, scope, true)).trim();
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" || parsed.protocol === "http:" ? { openUrl: parsed.toString() } : null;
  } catch {
    return null;
  }
}

/**
 * Media URLs: only `data:` loads inline. An http(s) address is returned as
 * `remote` so the renderer can offer to open it; anything else is dropped
 * (spec: render nothing rather than an error).
 */
export function mediaSource(value: unknown): { inline?: string; remote?: string } {
  const url = toText(value).replace(/[\t\n\r]/g, "").trim();
  if (!url) return {};
  if (/^data:(image|video|audio)\//i.test(url)) return { inline: url };
  if (/^https?:\/\/[^/\\\s]+/i.test(url)) return { remote: url };
  return {};
}

/** The parts of a user message sent from a surface: the readable text and whether an A2UI event rode along. */
export function splitEventMessage(content: string): { text: string; event: boolean } {
  const fence = new RegExp("\\n*```" + EVENT_FENCE + "\\n[\\s\\S]*?```\\s*$");
  if (!fence.test(content)) return { text: content, event: false };
  return { text: content.replace(fence, "").trimEnd(), event: true };
}

/** Children of a container: a static id list, or one template per item of a data list. */
export function childEntries(children: unknown, scope: Scope): { id: string; scope: Scope; key: string }[] {
  if (Array.isArray(children)) return children.filter((c): c is string => typeof c === "string").map((id, i) => ({ id, scope, key: `${i}:${id}` }));
  if (isObject(children) && typeof children.componentId === "string" && typeof children.path === "string") {
    const path = absolutePath(children.path, scope.base);
    const list = getPointer(scope.data, path);
    if (!Array.isArray(list)) return [];
    return list.map((_, i) => ({ id: children.componentId as string, scope: { data: scope.data, base: `${path}/${i}`, index: i }, key: `${path}/${i}` }));
  }
  return [];
}

/** A short plain-words error for the model, in the spec's error shape. The path is the first problem's field, so the model knows what to fix. */
export function errorReport(surfaceId: string, problems: A2uiProblem[]): string {
  const first = problems[0];
  const envelope = { version: A2UI_VERSION, error: { code: "VALIDATION_FAILED", surfaceId, path: first?.path ?? "/", message: problems.slice(0, 5).map((p) => p.message).join(" ") } };
  return `The interface you drew had problems. Please send a corrected version.\n\n\`\`\`${EVENT_FENCE}\n${JSON.stringify(envelope)}\n\`\`\``;
}
