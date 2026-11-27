// Web search and page reading over public HTTP APIs. Every provider works
// without a key: DuckDuckGo, Wikipedia, the Jina Reader, Exa (through its
// public MCP endpoint) and Firecrawl. A key pasted into Settings raises the
// free limits. Ported from services/websearch/mod.rs.

import { AppError } from "../core/errors.mjs";
import { loadConfig, updateConfig } from "../core/config.mjs";

const MAX_RESPONSE_BYTES = 2 * 1024 * 1024;
const USER_AGENT = "Mozilla/5.0 (compatible; LocalAide/0.3; +https://github.com/omarjson/LocalAide)";
/** The order "Automatic" tries providers in. Each is keyless. */
const AUTO_SEARCH_ORDER = ["duckduckgo", "exa", "wikipedia"];

const BASES = {
  // The lite endpoint serves a small regular table of results and is far
  // cheaper to parse than the full HTML page.
  ddg: "https://lite.duckduckgo.com",
  wikipedia: "https://en.wikipedia.org",
  jina: "https://r.jina.ai",
  exa: "https://api.exa.ai",
  exaMcp: "https://mcp.exa.ai",
  firecrawl: "https://api.firecrawl.dev",
};

/** One selectable provider, as the Settings screen renders it. */
export const PROVIDER_TABLE = [
  { id: "auto", label: "Automatic", needs_key: false, takes_key: false, supports: "search", note: "Tries DuckDuckGo, then Exa, then Wikipedia, and uses the first that answers." },
  { id: "duckduckgo", label: "DuckDuckGo", needs_key: false, takes_key: false, supports: "search", note: "Private web search. No key." },
  { id: "exa", label: "Exa", needs_key: false, takes_key: true, supports: "search+crawl", note: "Search built for AI. Works without a key at a free rate limit; a key raises it." },
  { id: "firecrawl", label: "Firecrawl", needs_key: false, takes_key: true, supports: "search+crawl", note: "Search and clean page text. Works without a key (a small free allowance per computer), with a key, or on your own computer." },
  { id: "wikipedia", label: "Wikipedia", needs_key: false, takes_key: false, supports: "search", note: "Encyclopedia articles only. No key." },
  { id: "jina", label: "Jina Reader", needs_key: false, takes_key: false, supports: "crawl", note: "Reads a page as text. No key." },
];

const knownProvider = (id) => PROVIDER_TABLE.some((p) => p.id === id);

/** The user's web settings, read from the config on every request. */
function webKeys() {
  const ws = loadConfig().web_search ?? {};
  return {
    exa_api_key: ws.exa_api_key ?? null,
    firecrawl_api_key: ws.firecrawl_api_key ?? null,
    firecrawl_base_url: ws.firecrawl_base_url ?? null,
  };
}

const keyFor = (keys, id) => {
  const key = id === "exa" ? keys.exa_api_key : id === "firecrawl" ? keys.firecrawl_api_key : null;
  const trimmed = key?.trim();
  return trimmed ? trimmed : null;
};

const firecrawlBase = (keys, fallback) => {
  const base = keys.firecrawl_base_url?.trim().replace(/\/+$/, "");
  return base || fallback;
};

/** True when page reading should go to Firecrawl by default. */
const prefersFirecrawl = (keys) =>
  keyFor(keys, "firecrawl") !== null ||
  (keys.firecrawl_base_url ?? "").trim() !== "";

const defaultProvider = (cfg) => {
  const wanted = cfg.web_search?.provider;
  return knownProvider(wanted) ? wanted : "auto";
};

// ---- HTTP helpers ----------------------------------------------------

function checkStatus(status) {
  if (status === 429) {
    throw new AppError("WEB_RATE_LIMITED", "The provider is rate-limiting this machine — wait a moment or switch provider in Settings.");
  }
  if (status === 401 || status === 403) {
    throw new AppError("WEB_AUTH_FAILED", "The provider rejected the API key. Check it in Settings → Web search.");
  }
  if (status < 200 || status >= 300) {
    throw AppError.internal(`The web provider returned HTTP ${status}`);
  }
}

/** Reads a body with a size cap, so a hostile endpoint cannot fill memory. */
async function readCapped(response) {
  const buffer = await response.arrayBuffer();
  if (buffer.byteLength > MAX_RESPONSE_BYTES) {
    throw AppError.invalidInput("The web response exceeds the size cap");
  }
  return new TextDecoder().decode(buffer);
}

async function fetchUrl(url, options) {
  try {
    return await fetch(url, {
      ...options,
      signal: AbortSignal.timeout(30_000),
      headers: { "User-Agent": USER_AGENT, ...(options?.headers ?? {}) },
    });
  } catch (e) {
    if (e instanceof AppError) throw e;
    throw AppError.internal(`Web request failed: ${e}`);
  }
}

async function getJson(url, accept = "*/*") {
  const response = await fetchUrl(url, { headers: { Accept: accept } });
  const title = response.headers.get("x-title");
  checkStatus(response.status);
  return { body: await readCapped(response), title };
}

async function postJson(url, headers, body) {
  const response = await fetchUrl(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...headers },
    body,
  });
  checkStatus(response.status);
  return readCapped(response);
}

// ---- URL validation ---------------------------------------------------

function isPrivateV4(ip) {
  const [a, b] = ip.split(".").map(Number);
  const inRange = (lo, hi) => a === lo[0] && b >= lo[1] && b <= hi[1];
  return (
    a === 0 || a === 10 || a === 127 ||
    (a === 100 && (b & 0xc0) === 64) ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    (a === 198 && (b === 18 || b === 19)) ||
    (a === 224) || a >= 240
  );
}

/** Hosts that only make sense inside the user's own computer or network. */
function isPrivateHost(parsed) {
  const name = parsed.hostname.toLowerCase().replace(/\.+$/, "");
  if (parsed.protocol === "http:" || parsed.protocol === "https:") {
    if (name === "localhost" || !name.includes(".")) return true;
    if (/\.(localhost|local|internal|lan|home\.arpa|intranet)$/.test(name)) return true;
    // IPv4 (parsed hostname is already normalized for IPv4)
    if (/^\d{1,3}(\.\d{1,3}){3}$/.test(name)) return isPrivateV4(name);
    // Bracketed IPv6 arrives as [::1]
    if (name.startsWith("[") && name.endsWith("]")) {
      const v6 = name.slice(1, -1).toLowerCase();
      if (v6 === "::1" || v6 === "::") return true;
      if (v6.startsWith("fe8") || v6.startsWith("fe9") || v6.startsWith("fea") || v6.startsWith("feb")) return true;
      if (/^f[cd]/.test(v6)) return true;
      const mapped = /^::ffff:(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})$/.exec(v6);
      if (mapped) return isPrivateV4(mapped[1]);
      return false;
    }
    return false;
  }
  return true;
}

/**
 * Crawl targets must be plain http(s) URLs on a public host, without
 * embedded credentials: the address goes to an online reader, so addresses
 * inside the user's own computer or network never leave it.
 */
export function validateCrawlUrl(url) {
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    throw AppError.invalidInput("That is not a valid URL to read.");
  }
  if (parsed.username || parsed.password) {
    throw AppError.invalidInput("Do not put a username or password in the URL.");
  }
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw AppError.invalidInput("Only http and https pages can be read.");
  }
  if (isPrivateHost(parsed)) {
    throw AppError.invalidInput(
      "Pages on this computer or your local network cannot be read here, because the address would be sent to an online reader.",
    );
  }
  return parsed;
}

// ---- Small HTML/JSON parsers -----------------------------------------

const decodeEntities = (s) =>
  s
    .replaceAll("&amp;", "&")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&#x27;", "'")
    .replaceAll("&apos;", "'")
    .replaceAll("&nbsp;", " ");

/** Drop `<...>` spans from a DDG snippet fragment. */
function stripTags(s) {
  let out = "";
  let depth = 0;
  for (const c of s) {
    if (c === "<") depth += 1;
    else if (c === ">") depth = Math.max(0, depth - 1);
    else if (depth === 0) out += c;
  }
  return out;
}

const collapseSpace = (s) => s.split(/\s+/).filter(Boolean).join(" ");

/** The value of one attribute in an HTML open tag, either quote style. */
function tagAttr(tag, name) {
  let search = 0;
  while (true) {
    const found = tag.indexOf(name, search);
    if (found < 0) return null;
    search = found + name.length;
    const beforeOk = found > 0 && /\s/.test(tag[found - 1]);
    const rest = tag.slice(search).trimStart();
    if (!beforeOk || !rest.startsWith("=")) continue;
    const value = rest.slice(1).trimStart();
    const q = value[0];
    if (q === '"' || q === "'") {
      const end = value.indexOf(q, 1);
      return end >= 0 ? value.slice(1, end) : null;
    }
    const end = value.search(/[\s>]/);
    return end >= 0 ? value.slice(0, end) : value;
  }
}

const tagHasClass = (tag, classes) => {
  const value = tagAttr(tag, "class");
  return value ? value.split(/\s+/).some((c) => classes.includes(c)) : false;
};

const DDG_TITLE_CLASSES = ["result-link", "result__a"];
const DDG_SNIPPET_CLASSES = ["result-snippet", "result__snippet"];

function openTags(body, name) {
  const out = [];
  const needle = `<${name}`;
  let from = 0;
  while (true) {
    const found = body.indexOf(needle, from);
    if (found < 0) break;
    const start = found;
    const after = start + needle.length;
    from = after;
    const next = body[after];
    if (!(next === undefined || /\s/.test(next) || next === ">")) continue;
    const end = body.indexOf(">", after);
    if (end < 0) break;
    out.push([start, end + 1]);
  }
  return out;
}

/** Inverse of urlencode for uddg= hrefs. */
function urldecode(s) {
  try {
    return decodeURIComponent(s.replace(/\+/g, "%2B"));
  } catch {
    return s;
  }
}

/** Pull the uddg= target out of a DDG redirect href, or pass through. */
function resolveDdgHref(href) {
  const decoded = decodeEntities(href);
  const full = decoded.startsWith("//") ? `https:${decoded}` : decoded;
  const start = full.indexOf("uddg=");
  if (start < 0) return full;
  const rest = full.slice(start + 5);
  const encoded = rest.split("&")[0] ?? rest;
  return urldecode(encoded);
}

/** Parse a DuckDuckGo results page (the lite table or the full HTML). */
export function parseDdgHtml(body, limit) {
  const lowered = body.toLowerCase();
  if (lowered.includes("anomaly-modal") || lowered.includes("unusual traffic") || lowered.includes("bots use duckduckgo too")) {
    throw new AppError(
      "WEB_RATE_LIMITED",
      "DuckDuckGo asked for a check that LocalAide cannot pass. Try again later, or choose Automatic or Exa in Settings → Web search.",
    );
  }
  const anchors = openTags(body, "a").filter(([start, end]) => tagHasClass(body.slice(start, end), DDG_TITLE_CLASSES));
  const hits = [];
  for (let i = 0; i < anchors.length && hits.length < limit; i += 1) {
    const [start, openEnd] = anchors[i];
    const tag = body.slice(start, openEnd);
    const href = tagAttr(tag, "href");
    if (!href) continue;
    const close = body.indexOf("</a>", openEnd);
    if (close < 0) continue;
    const title = decodeEntities(stripTags(body.slice(openEnd, close))).trim();
    const windowEnd = i + 1 < anchors.length ? anchors[i + 1][0] : body.length;
    const windowStart = openEnd + close + 4;
    let snippet = "";
    if (windowStart < windowEnd) {
      const window = body.slice(windowStart, windowEnd);
      let best = null;
      for (const name of ["td", "div", "a", "span"]) {
        for (const [s, e] of openTags(window, name)) {
          if (tagHasClass(window.slice(s, e), DDG_SNIPPET_CLASSES) && (best === null || s < best[0])) {
            best = [s, e];
          }
        }
      }
      if (best) {
        const tail = window.slice(best[1]);
        let end = tail.length;
        for (const c of ["</td>", "</div>", "</a>", "</span>"]) {
          const at = tail.indexOf(c);
          if (at >= 0) end = Math.min(end, at);
        }
        snippet = collapseSpace(decodeEntities(stripTags(tail.slice(0, end))));
      }
    }
    if (title) hits.push({ title, url: resolveDdgHref(href), snippet, source: "duckduckgo" });
  }
  return hits;
}

/** The text of an MCP tools/call answer, JSON or SSE. */
export function parseMcpResult(body) {
  const trimmed = body.trim();
  let message;
  if (trimmed.startsWith("{")) {
    try {
      message = JSON.parse(trimmed);
    } catch {
      throw AppError.internal("The search service sent an answer LocalAide could not read.");
    }
  } else {
    let found = null;
    for (const line of trimmed.split("\n")) {
      if (!line.startsWith("data:")) continue;
      try {
        const value = JSON.parse(line.slice(5).trim());
        if (value?.result || value?.error) found = value;
      } catch {
        /* not a JSON frame */
      }
    }
    if (!found) throw AppError.internal("The search service sent an empty answer.");
    message = found;
  }
  if (message.error) {
    const text = message.error.message ?? "unknown error";
    if (String(text).toLowerCase().includes("rate")) {
      throw new AppError("WEB_RATE_LIMITED", "The free search limit is used up for now. Wait a little, or add a key in Settings → Web search.");
    }
    throw AppError.internal(`The search service said: ${text}`);
  }
  const result = message.result ?? {};
  const text = Array.isArray(result.content)
    ? result.content.filter((p) => typeof p?.text === "string").map((p) => p.text).join("\n")
    : "";
  if (result.isError === true) {
    const lower = text.toLowerCase();
    if (lower.includes("rate limit") || lower.includes("429") || (lower.includes("free") && lower.includes("limit"))) {
      throw new AppError("WEB_RATE_LIMITED", "The free search limit is used up for now. Wait a little, or add a key in Settings → Web search.");
    }
    throw AppError.internal(`The search service said: ${text.slice(0, 300)}`);
  }
  return text;
}

const truncateChars = (s, max) =>
  [...s].length <= max ? s : `${[...s.slice(0, max)].join("").trimEnd()}…`;

/** Exa MCP search output: JSON with results, or Title:/URL: lines. */
export function parseExaMcpSearch(text, limit) {
  try {
    const json = JSON.parse(text.trim());
    if (Array.isArray(json?.results)) {
      return json.results.slice(0, limit).map((r) => {
        const url = r.url;
        const title = r.title?.trim() ? r.title.trim() : url;
        const snippet =
          r.summary ?? r.text ?? (Array.isArray(r.highlights) ? r.highlights.filter((h) => typeof h === "string").join(" … ") : "");
        return { title, url, snippet: truncateChars(collapseSpace(snippet), 400), source: "exa" };
      }).filter((h) => h.url);
    }
  } catch {
    /* the plain-text form follows */
  }
  const hits = [];
  let title = null;
  let url = null;
  let snippet = "";
  const flush = () => {
    if (url) {
      hits.push({ title: title ?? url, url, snippet: truncateChars(collapseSpace(snippet), 400), source: "exa" });
    }
    title = null;
    url = null;
    snippet = "";
  };
  for (const rawLine of text.split("\n")) {
    const line = rawLine.trim();
    if (line.startsWith("Title:")) {
      flush();
      title = line.slice(6).trim();
    } else if (line.startsWith("URL:")) {
      url = line.slice(4).trim();
    } else if (line.startsWith("Published") || line.startsWith("Author") || line.startsWith("ID:")) {
      continue;
    } else if (line && url && snippet.length < 600) {
      const textPart = line.startsWith("Text:")
        ? line.slice(5)
        : line.startsWith("Highlights:")
          ? line.slice(11)
          : line.startsWith("Summary:")
            ? line.slice(8)
            : line;
      snippet += `${textPart.trim()} `;
    }
  }
  flush();
  return hits.slice(0, limit);
}

/** Exa MCP page output: text, optionally led by Title: and URL: lines. */
export function parseExaMcpPage(text, url) {
  let title = null;
  const body = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (title === null && trimmed.startsWith("Title:")) {
      title = trimmed.slice(6).trim();
      continue;
    }
    if (body.length === 0 && (trimmed.startsWith("URL:") || trimmed.startsWith("ID:"))) continue;
    body.push(line);
  }
  let content = body.join("\n").trim();
  if (content.startsWith("Text:")) content = content.slice(5).trim();
  return { title: title || null, url, content };
}

/** Parse a Jina Reader response: JSON mode, or plain text. */
export function parseJinaResponse(body, headerTitle, url) {
  try {
    const root = JSON.parse(body);
    if (typeof root?.code === "number") {
      const title = root.data?.title || headerTitle || null;
      return { title: title || null, url, content: root.data?.content ?? "" };
    }
  } catch {
    /* plain text follows */
  }
  return { title: headerTitle || null, url, content: body };
}

/** Parse the Wikipedia REST search response into hits. */
export function parseWikipediaJson(body, limit, base) {
  let root;
  try {
    root = JSON.parse(body);
  } catch (e) {
    throw AppError.invalidInput(`Wikipedia response is invalid: ${e}`);
  }
  return (Array.isArray(root?.pages) ? root.pages : []).slice(0, limit).map((e) => {
    const slug = e.key || String(e.title ?? "").replaceAll(" ", "_");
    return {
      url: `${base.replace(/\/+$/, "")}/wiki/${slug}`,
      title: e.title ?? "",
      snippet: decodeEntities(stripTags(e.excerpt ?? "")),
      source: "wikipedia",
    };
  });
}

/** Parse an Exa /search response. */
export function parseExaJson(body, limit) {
  let root;
  try {
    root = JSON.parse(body);
  } catch (e) {
    throw AppError.invalidInput(`Exa response is invalid: ${e}`);
  }
  return (Array.isArray(root?.results) ? root.results : []).slice(0, limit).map((e) => ({
    title: e.title ? e.title : e.url,
    url: e.url,
    snippet: [...String(e.text ?? "")].slice(0, 400).join(""),
    source: "exa",
  }));
}

/** Parse a Firecrawl /v2/search response. */
export function parseFirecrawlSearchJson(body, limit) {
  let root;
  try {
    root = JSON.parse(body);
  } catch (e) {
    throw AppError.invalidInput(`Firecrawl response is invalid: ${e}`);
  }
  return (root?.data?.web ?? []).slice(0, limit).map((e) => ({
    title: e.title ? e.title : e.url,
    url: e.url,
    snippet: e.description ?? "",
    source: "firecrawl",
  }));
}

/** Parse a Firecrawl /v2/scrape response into a page. */
export function parseFirecrawlScrapeJson(body, url) {
  let root;
  try {
    root = JSON.parse(body);
  } catch (e) {
    throw AppError.invalidInput(`Firecrawl response is invalid: ${e}`);
  }
  const markdown = root?.data?.markdown;
  if (typeof markdown !== "string") {
    throw AppError.invalidInput("Firecrawl returned no page content for that URL");
  }
  return { title: root.data?.metadata?.title ?? null, url, content: markdown };
}

// ---- The engine --------------------------------------------------------

/** One Exa MCP tool call: initialize, the notification, then tools/call. */
async function mcpCall(base, tool, args) {
  const url = `${base.replace(/\/+$/, "")}/mcp?tools=web_search_advanced_exa,web_fetch_exa`;
  const post = async (payload, session) => {
    const response = await fetchUrl(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        ...(session ? { "Mcp-Session-Id": session } : {}),
      },
      body: JSON.stringify(payload),
    });
    const sessionId = response.headers.get("mcp-session-id");
    const body = response.ok ? await readCapped(response) : "";
    return { status: response.status, session: sessionId, body };
  };
  const init = await post(
    {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "LocalAide", version: "0.3" } },
    },
    null,
  );
  checkStatus(init.status);
  await post({ jsonrpc: "2.0", method: "notifications/initialized" }, init.session).catch(() => {});
  const call = await post(
    { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: tool, arguments: args } },
    init.session,
  );
  checkStatus(call.status);
  return parseMcpResult(call.body);
}

async function searchOne(provider, query, limit, keys) {
  let hits;
  switch (provider) {
    case "duckduckgo": {
      const form = new URLSearchParams({ q: query });
      const response = await fetchUrl(`${BASES.ddg}/lite/`, {
        method: "POST",
        headers: { Accept: "text/html", "Content-Type": "application/x-www-form-urlencoded" },
        body: form.toString(),
      });
      checkStatus(response.status);
      hits = parseDdgHtml(await readCapped(response), limit);
      break;
    }
    case "wikipedia": {
      const { body } = await getJson(
        `${BASES.wikipedia}/w/rest.php/v1/search/page?q=${encodeURIComponent(query)}&limit=${limit}`,
      );
      hits = parseWikipediaJson(body, limit, BASES.wikipedia);
      break;
    }
    case "exa": {
      const key = keyFor(keys, "exa");
      if (key) {
        const response = await postJson(
          `${BASES.exa}/search`,
          { "x-api-key": key },
          JSON.stringify({ query, numResults: limit, contents: { text: { maxCharacters: 500 } } }),
        );
        hits = parseExaJson(response, limit);
      } else {
        const text = await mcpCall(BASES.exaMcp, "web_search_advanced_exa", {
          query,
          numResults: limit,
          textMaxCharacters: 500,
        });
        hits = parseExaMcpSearch(text, limit);
      }
      break;
    }
    case "firecrawl": {
      const base = firecrawlBase(keys, BASES.firecrawl);
      const auth = keyFor(keys, "firecrawl");
      const response = await postJson(
        `${base}/v2/search`,
        auth ? { Authorization: `Bearer ${auth}` } : {},
        JSON.stringify({ query, limit }),
      );
      hits = parseFirecrawlSearchJson(response, limit);
      break;
    }
    default:
      throw new AppError("WEB_UNKNOWN_PROVIDER", `'${provider}' cannot search the web.`);
  }
  // Results become links the user can open, so only plain http(s) kept.
  return hits
    .filter((hit) => {
      try {
        validateCrawlUrl(hit.url);
        return true;
      } catch {
        return false;
      }
    })
    .map((hit) => ({ ...hit, source: provider }));
}

/** Search the web with the chosen provider ("auto" falls through). */
export async function searchWeb(providerName, query, limit, keys = webKeys()) {
  if (!knownProvider(providerName)) {
    throw new AppError("WEB_UNKNOWN_PROVIDER", `'${providerName}' is not a web search provider this build knows.`);
  }
  const cap = Math.max(1, Math.min(20, Math.round(limit) || 8));
  if (providerName !== "auto") return searchOne(providerName, query, cap, keys);
  // Automatic: the first provider with results wins.
  let lastError = null;
  for (const id of AUTO_SEARCH_ORDER) {
    try {
      const hits = await searchOne(id, query, cap, keys);
      if (hits.length) return hits;
    } catch (e) {
      lastError = e;
    }
  }
  if (lastError) throw lastError;
  return [];
}

/** Read one page as text. "auto" prefers Firecrawl, then Jina, then Exa. */
export async function readPage(url, providerName, keys = webKeys()) {
  const checked = validateCrawlUrl(url);
  const target = checked.toString();
  const readWith = async (provider) => {
    switch (provider) {
      case "firecrawl": {
        const base = firecrawlBase(keys, BASES.firecrawl);
        const auth = keyFor(keys, "firecrawl");
        const response = await postJson(
          `${base}/v2/scrape`,
          auth ? { Authorization: `Bearer ${auth}` } : {},
          JSON.stringify({ url: target, formats: ["markdown"], onlyMainContent: true }),
        );
        return parseFirecrawlScrapeJson(response, target);
      }
      case "jina": {
        const { body, title } = await getJson(`${BASES.jina}/${target}`, "application/json");
        return parseJinaResponse(body, title, target);
      }
      case "exa": {
        const text = await mcpCall(BASES.exaMcp, "web_fetch_exa", { urls: [target], maxCharacters: 20000 });
        return parseExaMcpPage(text, target);
      }
      default:
        throw new AppError("WEB_UNKNOWN_PROVIDER", `'${provider}' cannot read pages. Use Firecrawl, Jina Reader or Exa.`);
    }
  };
  if (providerName !== "auto") return readWith(providerName);
  if (prefersFirecrawl(keys)) return readWith("firecrawl");
  try {
    return await readWith("jina");
  } catch (jinaError) {
    try {
      return await readWith("exa");
    } catch {
      throw jinaError;
    }
  }
}

/** The provider table with the local key/default state, for Settings. */
export function webProviders() {
  const cfg = loadConfig();
  const keys = webKeys();
  const def = defaultProvider(cfg);
  return PROVIDER_TABLE.map((p) => ({
    id: p.id,
    label: p.label,
    needs_key: p.needs_key,
    takes_key: p.takes_key,
    supports: p.supports,
    note: p.note,
    base_url: p.id === "firecrawl" ? (keys.firecrawl_base_url || null) : null,
    key_set: keyFor(keys, p.id) !== null,
    is_default: p.id === def,
  }));
}

/** Store or clear a provider API key. Blank clears the slot. */
export function setWebKey(provider, key) {
  const value = key?.trim() || null;
  if (value && value.length > 500) {
    throw AppError.invalidInput("That API key is too long.");
  }
  updateConfig((cfg) => {
    if (provider === "exa") cfg.web_search.exa_api_key = value;
    else if (provider === "firecrawl") cfg.web_search.firecrawl_api_key = value;
    else throw new AppError("WEB_UNKNOWN_PROVIDER", `'${provider}' does not take an API key.`);
  });
}

/** Pick the provider used when the user searches without choosing one. */
export function setWebDefault(provider) {
  if (!knownProvider(provider)) {
    throw new AppError("WEB_UNKNOWN_PROVIDER", `'${provider}' is not a web search provider this build knows.`);
  }
  updateConfig((cfg) => {
    cfg.web_search.provider = provider;
  });
}

/** Point Firecrawl at a server the user runs; null clears it. */
export function setFirecrawlUrl(url) {
  const value = url?.trim().replace(/\/+$/, "") || null;
  if (value) {
    const parsed = new URL(value);
    const loopback =
      parsed.hostname === "localhost" ||
      /^127\./.test(parsed.hostname) ||
      parsed.hostname === "[::1]";
    if (parsed.protocol === "http:" && !loopback) {
      throw AppError.invalidInput(
        "Plain HTTP is only allowed for addresses on this machine (127.0.0.1 or localhost). Use HTTPS for any remote provider.",
      );
    }
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      throw AppError.invalidInput("The Firecrawl address must start with http:// or https://.");
    }
  }
  updateConfig((cfg) => {
    cfg.web_search.firecrawl_base_url = value;
  });
}
