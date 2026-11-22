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
  /** Problems found while building the surface, in plain words. */
  problems: string[];
  deleted: boolean;
}

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
  problems: string[];
}

/**
 * Reads a ```a2ui block: JSONL (one envelope per line), a JSON array of
 * envelopes, or a single envelope. Lines that do not parse are reported.
 */
export function parseStream(source: string): ParsedStream {
  const text = source.trim();
  const problems: string[] = [];
  if (!text) return { envelopes: [], problems: ["The interface block is empty."] };
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
      else problems.push(`Line ${i + 1} is not a JSON object.`);
    } catch {
      problems.push(`Line ${i + 1} is not valid JSON.`);
    }
  });
  return { envelopes, problems };
}

// ---------------------------------------------------------------------------
// Applying messages

/** Applies one envelope to the surfaces. Returns the surface it created, if any, and any problem. */
export function applyEnvelope(surfaces: Record<string, Surface>, envelope: Record<string, unknown>): { created?: string; problem?: string } {
  if (envelope.version !== A2UI_VERSION) return { problem: `Only A2UI version "${A2UI_VERSION}" is supported; a message said ${JSON.stringify(envelope.version ?? null)}.` };
  const keys = MESSAGE_KEYS.filter((k) => k in envelope);
  if (keys.length !== 1) return { problem: "Each message needs exactly one of createSurface, updateComponents, updateDataModel or deleteSurface." };
  const kind = keys[0];
  const body = envelope[kind];
  if (!isObject(body)) return { problem: `${kind} must be an object.` };
  if (kind === "callRendererFunction" || kind === "agentFunctionResponse") {
    return { problem: `${kind} is not supported in the chat, so it was skipped.` };
  }
  const surfaceId = body.surfaceId;
  if (typeof surfaceId !== "string" || !surfaceId) return { problem: `${kind} needs a surfaceId.` };

  if (kind === "createSurface") {
    const existing = surfaces[surfaceId];
    if (existing && !existing.deleted) return { problem: `A surface called "${surfaceId}" already exists (INTEGRITY_ERROR). Delete it first or use another surfaceId.` };
    if ("theme" in body) return { problem: "createSurface may not carry a theme in A2UI 1.0; the app's own theme is used." };
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
      surface.problems.push(catalogId ? `Only the A2UI Basic Catalog is available here, not "${catalogId}".` : "createSurface needs a catalogId (the Basic Catalog).");
    }
    if (Array.isArray(body.components)) addComponents(surface, body.components);
    surfaces[surfaceId] = surface;
    return { created: surfaceId };
  }

  const surface = surfaces[surfaceId];
  if (!surface || surface.deleted) return { problem: `There is no surface called "${surfaceId}" to ${kind === "deleteSurface" ? "delete" : "update"}.` };
  if (kind === "deleteSurface") {
    surface.deleted = true;
    return {};
  }
  if (kind === "updateComponents") {
    if (!Array.isArray(body.components)) return { problem: "updateComponents needs a components list." };
    addComponents(surface, body.components);
    return {};
  }
  // updateDataModel
  if (!("value" in body)) return { problem: "updateDataModel needs a value (null removes the key)." };
  const path = typeof body.path === "string" && body.path ? body.path : "/";
  if (path === "/") {
    surface.data = isObject(body.value) ? (clone(body.value) as JsonObject) : {};
  } else {
    setPointer(surface.data, path, body.value === null ? undefined : (clone(body.value) as JsonValue));
  }
  return {};
}

function addComponents(surface: Surface, list: unknown[]): void {
  list.forEach((raw, i) => {
    if (!isObject(raw)) {
      surface.problems.push(`Component ${i + 1} is not an object.`);
      return;
    }
    const { id, component } = raw;
    if (!isIdentifier(id)) {
      surface.problems.push(`Component ${i + 1} has an id that is not a plain name (letters, digits and underscores, not starting with a digit): ${JSON.stringify(id ?? null)}.`);
      return;
    }
    if (typeof component !== "string" || !COMPONENTS.has(component)) {
      surface.problems.push(`"${id}" uses ${JSON.stringify(component ?? null)}, which is not in the Basic Catalog.`);
      return;
    }
    if (raw["@catalogId"] !== undefined && raw["@catalogId"] !== BASIC_CATALOG_ID) {
      surface.problems.push(`"${id}" asks for another catalog; only the Basic Catalog is available.`);
      return;
    }
    surface.components[id] = raw as A2uiComponent;
  });
}

/** Builds surfaces from one block. */
export function buildSurfaces(source: string, surfaces: Record<string, Surface> = {}): { surfaces: Record<string, Surface>; created: string[]; problems: string[] } {
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

/** Rejects regular expressions outside the A2UI dialect or likely to run away. */
export function safePattern(pattern: string): RegExp | null {
  if ([...pattern].length > 256) return null;
  if (/\\[1-9]|\\k<|\(\?<[=!]|\(\?<[A-Za-z]|\\[pP]\{|\\c[A-Za-z]|\(\?[a-z]/.test(pattern)) return null;
  // A quantified group holding an unbounded quantifier: (a+)+, (a*){2,}...
  if (/\((?:[^()\\]|\\.)*[*+](?:[^()\\]|\\.)*\)(?:[*+]|\{\d+,\d*\})/.test(pattern)) return null;
  try {
    return new RegExp(pattern, "u");
  } catch {
    return null;
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
      const re = safePattern(toText(arg("pattern")));
      if (!re || [...v].length > 4096) return valid(false);
      return valid(re.test(v));
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

/** A short plain-words error for the model, in the spec's error shape. */
export function errorReport(surfaceId: string, problems: string[]): string {
  const envelope = { version: A2UI_VERSION, error: { code: "VALIDATION_FAILED", surfaceId, path: "/", message: problems.slice(0, 5).join(" ") } };
  return `The interface you drew had problems. Please send a corrected version.\n\n\`\`\`${EVENT_FENCE}\n${JSON.stringify(envelope)}\n\`\`\``;
}
