// MCP (Model Context Protocol) client: LocalAide talks to the user's
// servers over stdio (a program it starts) or Streamable HTTP, lists their
// tools, and offers them to the chat. Ported from services/mcp/ — the
// transport handshake, the config shapes and the tool naming are the same.

import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { AppError } from "../core/errors.mjs";
import { loadConfig, updateConfig } from "../core/config.mjs";
import { log } from "../core/log.mjs";

const PROTOCOL_VERSION = "2025-06-18";
const CLIENT_INFO = { name: "LocalAide", version: "0.3.0" };
const CONNECT_TIMEOUT_MS = 15_000;
const CALL_TIMEOUT_MS = 120_000;

// ---- Transports ---------------------------------------------------------

/** One JSON-RPC exchange over newline-delimited stdio. */
class StdioTransport {
  constructor(config) {
    this.config = config;
    this.child = null;
    this.pending = new Map();
    this.buffer = "";
    this.nextId = 0;
    this.dead = false;
  }

  async start() {
    const { command, args = [], env = {}, cwd } = this.config;
    if (!command) throw AppError.invalidInput("That server has no command to run.");
    const child = spawn(command, args, {
      cwd: cwd || undefined,
      env: { ...process.env, ...env },
      stdio: ["pipe", "pipe", "pipe"],
      windowsHide: true,
    });
    this.child = child;
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      this.buffer += chunk;
      let index;
      while ((index = this.buffer.indexOf("\n")) >= 0) {
        const line = this.buffer.slice(0, index).trim();
        this.buffer = this.buffer.slice(index + 1);
        if (!line) continue;
        try {
          this.dispatch(JSON.parse(line));
        } catch {
          /* a non-JSON line (a banner, a log) is ignored */
        }
      }
    });
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      const text = String(chunk).trim();
      if (text) log("info", "mcp server stderr", text.slice(0, 300));
    });
    child.on("exit", () => {
      this.dead = true;
      for (const [, pending] of this.pending) pending.reject(AppError.internal("The MCP server stopped."));
      this.pending.clear();
    });
    child.on("error", (e) => {
      this.dead = true;
      for (const [, pending] of this.pending) pending.reject(AppError.internal(`The MCP server could not start: ${e}`));
      this.pending.clear();
    });
  }

  dispatch(message) {
    if (message.id === undefined || message.id === null) return; // a notification
    const pending = this.pending.get(message.id);
    if (!pending) return;
    this.pending.delete(message.id);
    if (message.error) pending.reject(new AppError("PROVIDER_ERROR", message.error.message ?? "The MCP server reported an error."));
    else pending.resolve(message.result);
  }

  request(method, params, timeoutMs = CONNECT_TIMEOUT_MS) {
    if (this.dead) return Promise.reject(AppError.internal("The MCP server is not running."));
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(AppError.internal(`${method} timed out.`));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (e) => {
          clearTimeout(timer);
          reject(e);
        },
      });
      this.child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  }

  notify(method, params) {
    try {
      this.child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n");
    } catch {
      /* a dead server just fails the next request */
    }
  }

  stop() {
    this.dead = true;
    if (this.child) {
      try {
        if (process.platform === "win32") {
          spawn("taskkill", ["/pid", String(this.child.pid), "/t", "/f"], { windowsHide: true });
        } else {
          this.child.kill("SIGTERM");
        }
      } catch {
        /* already gone */
      }
    }
  }
}

/** One JSON-RPC exchange over Streamable HTTP. */
class HttpTransport {
  constructor(config, serverId) {
    this.config = config;
    this.serverId = serverId;
    this.session = null;
    this.nextId = 0;
  }

  async start() {
    // The session comes from the initialize response below.
  }

  async post(payload, timeoutMs = CONNECT_TIMEOUT_MS) {
    const { url, headers = {} } = this.config;
    if (!url) throw AppError.invalidInput("That server has no address to post to.");
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        ...(this.session ? { "Mcp-Session-Id": this.session } : {}),
        ...oauthBearer(this.serverId ?? ""),
        ...headers,
      },
      body: JSON.stringify(payload),
      signal: AbortSignal.timeout(timeoutMs),
    });
    const session = response.headers.get("mcp-session-id");
    if (session) this.session = session;
    if (!response.ok) {
      throw new AppError("PROVIDER_ERROR", `The MCP server answered ${response.status}.`);
    }
    const contentType = response.headers.get("content-type") ?? "";
    const text = await response.text();
    if (contentType.includes("text/event-stream")) {
      // The answer may arrive as one or more data: lines; the reply with our id wins.
      let result = null;
      for (const line of text.split("\n")) {
        if (!line.startsWith("data:")) continue;
        try {
          const message = JSON.parse(line.slice(5).trim());
          if (payload.id !== undefined && message.id === payload.id) result = message;
        } catch {
          /* keep scanning */
        }
      }
      if (!result) throw AppError.internal("The MCP server sent an empty answer.");
      if (result.error) throw new AppError("PROVIDER_ERROR", result.error.message ?? "The MCP server reported an error.");
      return result.result;
    }
    const message = JSON.parse(text);
    if (message.error) throw new AppError("PROVIDER_ERROR", message.error.message ?? "The MCP server reported an error.");
    return message.result;
  }

  request(method, params, timeoutMs = CONNECT_TIMEOUT_MS) {
    return this.post({ jsonrpc: "2.0", id: ++this.nextId, method, params }, timeoutMs);
  }

  async notify(method, params) {
    try {
      await this.post({ jsonrpc: "2.0", method, params });
    } catch {
      /* notifications are best-effort */
    }
  }

  stop() {
    this.session = null;
  }
}

const transportFor = (serverConfig, serverId) =>
  serverConfig.transport?.type === "http"
    ? new HttpTransport(serverConfig.transport, serverId)
    : new StdioTransport(serverConfig.transport ?? {});

// ---- The connection registry --------------------------------------------

/** Live connections and their cached tool lists, keyed by server id. */
const connections = new Map();

async function connect(serverId) {
  const existing = connections.get(serverId);
  if (existing && !existing.transport.dead) return existing;
  const config = loadConfig().mcp_servers.find((s) => s.id === serverId);
  if (!config) throw AppError.invalidInput("That MCP server no longer exists.");
  const transport = transportFor(config, serverId);
  await transport.start();
  const init = await transport.request("initialize", {
    protocolVersion: PROTOCOL_VERSION,
    capabilities: {},
    clientInfo: CLIENT_INFO,
  });
  transport.notify("notifications/initialized");
  const connection = { transport, name: init?.serverInfo?.name ?? config.name, tools: null };
  connections.set(serverId, connection);
  log("info", "mcp connected", `${config.name}: ${connection.name}`);
  return connection;
}

async function listTools(serverId, { refresh = false } = {}) {
  const connection = await connect(serverId);
  if (!connection.tools || refresh) {
    const result = await connection.transport.request("tools/list", {});
    connection.tools = Array.isArray(result?.tools) ? result.tools : [];
  }
  return connection.tools;
}

/** Calls one tool on one server. `timeoutMs` covers a slow tool run. */
export async function callTool(serverId, toolName, args) {
  const connection = await connect(serverId);
  const result = await connection.transport.request(
    "tools/call",
    { name: toolName, arguments: args ?? {} },
    CALL_TIMEOUT_MS,
  );
  const text = Array.isArray(result?.content)
    ? result.content
        .map((part) => (typeof part?.text === "string" ? part.text : ""))
        .filter(Boolean)
        .join("\n")
    : "";
  return { ok: result?.isError !== true, text: text || "The tool returned no text.", raw: result ?? null };
}

/** Stops every live server; called on quit. */
export function shutdownMcp() {
  for (const [, connection] of connections) connection.transport.stop();
  connections.clear();
}

export const dropConnection = (serverId) => {
  const connection = connections.get(serverId);
  if (connection) {
    connection.transport.stop();
    connections.delete(serverId);
  }
};

// ---- Config commands ------------------------------------------------------

/** A tool's wire name: letters, digits, `_` and `-`, up to 64 characters. */
export function toolNameFor(server, tool) {
  const clean = (s) =>
    String(s)
      .split("")
      .map((c) => (/[A-Za-z0-9_-]/.test(c) ? c : "_"))
      .join("")
      .replace(/^_+|_+$/g, "");
  const serverPart = clean(server).toLowerCase().slice(0, 20);
  return `mcp_${serverPart}_${clean(tool)}`.slice(0, 64);
}

/** The DTO the Settings screen renders, from the config's server list. */
export function listServers() {
  return (loadConfig().mcp_servers ?? []).map((server) => {
    const transport = server.transport ?? {};
    const isHttp = transport.type === "http";
    const toHints = (map) =>
      Object.entries(map ?? {}).map(([name, value]) => ({ name, hint: "" }));
    return {
      id: server.id,
      name: server.name,
      enabled: server.enabled !== false,
      auto_approve: server.auto_approve === true,
      kind: isHttp ? "http" : "stdio",
      command: transport.command ?? "",
      args: transport.args ?? [],
      cwd: transport.cwd ?? null,
      url: transport.url ?? "",
      env: toHints(transport.env),
      headers: toHints(transport.headers),
      oauth: server.oauth ?? null,
      connected: connections.has(server.id),
    };
  });
}

/** Validates and stores one server. Values may hold tokens: config only. */
export function saveServer(req) {
  const name = String(req.name ?? "").trim();
  if (!name) throw AppError.invalidInput("Give the server a name.");
  const kind = req.kind === "http" ? "http" : "stdio";
  let transport;
  if (kind === "http") {
    const url = String(req.url ?? "").trim();
    if (!/^https?:\/\//.test(url)) throw AppError.invalidInput("An HTTP server needs its address (it starts with http:// or https://).");
    transport = { type: "http", url, headers: headersFrom(req.headers) };
  } else {
    const command = String(req.command ?? "").trim();
    if (!command) throw AppError.invalidInput("A local server needs the command that starts it.");
    transport = {
      type: "stdio",
      command,
      args: Array.isArray(req.args) ? req.args.map(String) : [],
      env: envFrom(req.env),
      cwd: req.cwd ? String(req.cwd) : undefined,
    };
  }
  updateConfig((cfg) => {
    const server = {
      id: req.id && typeof req.id === "string" ? req.id : `mcp-${crypto.randomUUID().slice(0, 8)}`,
      name,
      enabled: req.enabled !== false,
      auto_approve: req.auto_approve === true,
      transport,
    };
    const existing = (cfg.mcp_servers ?? []).findIndex((s) => s.id === server.id);
    if (existing >= 0) cfg.mcp_servers[existing] = server;
    else cfg.mcp_servers = [...(cfg.mcp_servers ?? []), server];
  });
  return listServers();
}

const headersFrom = (hints) =>
  Object.fromEntries((Array.isArray(hints) ? hints : []).map((h) => [String(h.name), String(h.value ?? h.hint ?? "")]).filter(([name]) => name));
const envFrom = headersFrom;

export function toggleServer(id, change) {
  updateConfig((cfg) => {
    const server = (cfg.mcp_servers ?? []).find((s) => s.id === id);
    if (!server) throw AppError.invalidInput("That MCP server no longer exists.");
    if (change?.enabled !== undefined) server.enabled = change.enabled === true;
    if (change?.auto_approve !== undefined) server.auto_approve = change.auto_approve === true;
  });
  return listServers();
}

export function deleteServer(id) {
  updateConfig((cfg) => {
    cfg.mcp_servers = (cfg.mcp_servers ?? []).filter((s) => s.id !== id);
  });
  dropConnection(id);
  return listServers();
}

/** Connect and list the server's tools, for the Test button. */
export async function testServer(id) {
  const tools = await listTools(id, { refresh: true });
  return tools.map((tool) => ({ name: String(tool.name ?? ""), description: String(tool.description ?? "") }));
}

/** The tools of one server as chat specs, with their config alongside. */
export async function serverSpecs(serverId) {
  const cfg = loadConfig();
  const server = (cfg.mcp_servers ?? []).find((s) => s.id === serverId);
  if (!server || server.enabled === false) return [];
  let tools = [];
  try {
    tools = await listTools(serverId);
  } catch (e) {
    log("warn", "mcp tools unavailable", `${server.name}: ${e?.message ?? e}`);
    return [];
  }
  const specs = [];
  for (const tool of tools) {
    const name = toolNameFor(server.name, tool.name);
    if (specs.some((s) => s.name === name)) continue;
    const description = String(tool.description ?? "").trim() || `${tool.name} (from ${server.name})`;
    specs.push({
      name,
      description: `${description.slice(0, 1000)} (from ${server.name})`,
      parameters: tool.inputSchema ?? { type: "object", properties: {} },
      serverId,
      toolName: String(tool.name ?? ""),
      risky: server.auto_approve !== true,
      group: "mcp",
      title: tool.name,
    });
  }
  return specs;
}

/** The tools of every enabled server (or the ones the chat picked). */
export async function specsForChat(requestedIds) {
  const cfg = loadConfig();
  const servers = (cfg.mcp_servers ?? []).filter(
    (s) => s.enabled !== false && (!Array.isArray(requestedIds) || requestedIds.length === 0 || requestedIds.includes(s.id)),
  );
  const specs = [];
  for (const server of servers) {
    specs.push(...(await serverSpecs(server.id)));
  }
  return specs;
}

/** Runs an MCP tool the model called, by its wire name. */
export async function runChatTool(wireName, args) {
  const specs = await specsForChat(null);
  const spec = specs.find((s) => s.name === wireName);
  if (!spec) return { ok: false, text: `No MCP tool called ${wireName} is available right now.` };
  return callTool(spec.serverId, spec.toolName, args);
}

// ---- Presets and import ---------------------------------------------------

export const MCP_PRESETS = [
  { id: "context7", name: "Context7", description: "Current documentation and code examples for programming libraries.", kind: "http", url: "https://mcp.context7.com/mcp", command: "", args: [], requirement: "", needs_key: false, homepage: "https://context7.com" },
  { id: "deepwiki", name: "DeepWiki", description: "Ask questions about public GitHub repositories.", kind: "http", url: "https://mcp.deepwiki.com/mcp", command: "", args: [], requirement: "", needs_key: false, homepage: "https://deepwiki.com" },
  { id: "exa", name: "Exa search", description: "Web and code search from Exa.", kind: "http", url: "https://mcp.exa.ai/mcp", command: "", args: [], requirement: "", needs_key: false, homepage: "https://exa.ai" },
  { id: "microsoft-learn", name: "Microsoft Learn", description: "Search Microsoft's official documentation.", kind: "http", url: "https://learn.microsoft.com/api/mcp", command: "", args: [], requirement: "", needs_key: false, homepage: "https://learn.microsoft.com" },
  { id: "huggingface", name: "Hugging Face", description: "Search models, datasets and papers on Hugging Face.", kind: "http", url: "https://huggingface.co/mcp", command: "", args: [], requirement: "Add a header Authorization: Bearer <your token> for more tools.", needs_key: false, homepage: "https://huggingface.co/settings/mcp" },
  { id: "github", name: "GitHub", description: "Issues, pull requests and code on GitHub.", kind: "http", url: "https://api.githubcopilot.com/mcp/", command: "", args: [], requirement: "Needs a header Authorization: Bearer <a GitHub token>.", needs_key: true, homepage: "https://github.com/github/github-mcp-server" },
  { id: "filesystem", name: "Files on this computer", description: "Read and edit files in folders you list. Add the folders at the end of the arguments.", kind: "stdio", url: "", command: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem"], requirement: "Needs Node.js (npx).", needs_key: false, homepage: "https://github.com/modelcontextprotocol/servers" },
  { id: "memory-graph", name: "Knowledge graph memory", description: "Remembers facts as a small graph between chats.", kind: "stdio", url: "", command: "npx", args: ["-y", "@modelcontextprotocol/server-memory"], requirement: "Needs Node.js (npx).", needs_key: false, homepage: "https://github.com/modelcontextprotocol/servers" },
  { id: "sequential-thinking", name: "Sequential thinking", description: "Helps the model work through a problem step by step.", kind: "stdio", url: "", command: "npx", args: ["-y", "@modelcontextprotocol/server-sequential-thinking"], requirement: "Needs Node.js (npx).", needs_key: false, homepage: "https://github.com/modelcontextprotocol/servers" },
  { id: "fetch", name: "Fetch", description: "Reads web pages and turns them into text.", kind: "stdio", url: "", command: "uvx", args: ["mcp-server-fetch"], requirement: "Needs Python's uv (uvx).", needs_key: false, homepage: "https://github.com/modelcontextprotocol/servers" },
  { id: "git", name: "Git", description: "Reads the history and changes of a Git repository. Add --repository <folder>.", kind: "stdio", url: "", command: "uvx", args: ["mcp-server-git"], requirement: "Needs Python's uv (uvx).", needs_key: false, homepage: "https://github.com/modelcontextprotocol/servers" },
  { id: "time", name: "Time", description: "Current time and time zone conversion.", kind: "stdio", url: "", command: "uvx", args: ["mcp-server-time"], requirement: "Needs Python's uv (uvx).", needs_key: false, homepage: "https://github.com/modelcontextprotocol/servers" },
  { id: "playwright", name: "Playwright browser", description: "Opens a browser and clicks, types and reads pages for you.", kind: "stdio", url: "", command: "npx", args: ["-y", "@playwright/mcp@latest"], requirement: "Needs Node.js (npx). Downloads a browser the first time.", needs_key: false, homepage: "https://github.com/microsoft/playwright-mcp" },
];

/**
 * ArtCraft Connect: the MCP servers the user's installed ZCode plugins
 * already ship, found in the plugin cache. One click saves one.
 */
export function artcraftPresets() {
  const cacheRoot = path.join(os.homedir(), ".zcode", "cli", "plugins", "cache", "zcode-plugins-official");
  const out = [];
  let pluginDirs;
  try {
    pluginDirs = fs.readdirSync(cacheRoot);
  } catch {
    return out;
  }
  const saved = new Set((loadConfig().mcp_servers ?? []).map((s) => s.name));
  for (const dir of pluginDirs) {
    const versions = (() => {
      try {
        return fs.readdirSync(path.join(cacheRoot, dir));
      } catch {
        return [];
      }
    })();
    for (const version of versions) {
      const manifestPath = path.join(cacheRoot, dir, version, ".zcode-plugin", "plugin.json");
      let manifest;
      try {
        manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
      } catch {
        continue;
      }
      for (const [name, server] of Object.entries(manifest?.mcpServers ?? {})) {
        if (!server || typeof server !== "object") continue;
        const args = Array.isArray(server.args)
          ? server.args.map((a) => String(a).replace(/\$\{ZCODE_PLUGIN_ROOT\}/g, path.join(cacheRoot, dir, version)))
          : [];
        const env = Object.fromEntries(
          Object.entries(server.env ?? {}).map(([k, v]) => [k, String(v).replace(/\$\{ZCODE_PLUGIN_ROOT\}/g, path.join(cacheRoot, dir, version))]),
        );
        out.push({
          id: `artcraft-${dir}-${name}`,
          name: `${dir} ${name}`,
          description: `${name} — from your installed ${dir} plugin`,
          version: String(manifest.version ?? version),
          command: String(server.command ?? "node"),
          args,
          cwd: server.cwd ?? null,
          env,
          ready: true,
          added: saved.has(`${dir} ${name}`),
        });
      }
    }
  }
  return out.slice(0, 50);
}

/**
 * Import servers from a pasted JSON block — Claude Desktop's
 * `{"mcpServers": {...}}` format and plain arrays both work.
 */
export function importServers(text) {
  let parsed;
  try {
    parsed = JSON.parse(String(text ?? ""));
  } catch {
    throw AppError.invalidInput("That is not JSON the import understands.");
  }
  const entries = Object.entries(parsed?.mcpServers ?? (Array.isArray(parsed) ? Object.fromEntries(parsed.map((s, i) => [s.name ?? `server-${i}`, s])) : parsed));
  let added = 0;
  let skipped = 0;
  for (const [name, server] of entries) {
    try {
      const url = String(server.url ?? "");
      const command = String(server.command ?? "");
      const kind = url && !command ? "http" : "stdio";
      saveServer({
        name: String(server.name ?? name),
        enabled: true,
        auto_approve: false,
        kind,
        url,
        command,
        args: Array.isArray(server.args) ? server.args : [],
        env: Array.isArray(server.env) ? server.env : Object.entries(server.env ?? {}).map(([key, value]) => ({ name: key, value: String(value) })),
        headers: Array.isArray(server.headers) ? server.headers : Object.entries(server.headers ?? {}).map(([key, value]) => ({ name: key, value: String(value) })),
        cwd: server.cwd ?? null,
      });
      added += 1;
    } catch {
      skipped += 1;
    }
  }
  return { added, skipped, servers: listServers() };
}

// ---- OAuth 2.1 / PKCE sign-in (ADR-0012) --------------------------------

/** One pending sign-in, keyed by server id. The verifier never leaves here. */
const pendingSignIns = new Map();
/** Live access tokens by server id; injected as Bearer headers. */
const oauthTokens = new Map();

const pkceChallenge = (verifier) =>
  crypto.createHash("sha256").update(verifier).digest("base64url");

const oauthTokenKey = (serverId) => `mcp-oauth:${serverId}`;

/** Build the browser authorization URL with the S256 challenge and state. */
export function oauthAuthorizeUrl(cfg, state, challenge) {
  const check = (url) => {
    try {
      const parsed = new URL(url);
      if (parsed.protocol === "https:") return;
      if (parsed.protocol === "http:" && (parsed.hostname === "localhost" || /^127\./.test(parsed.hostname))) return;
    } catch {}
    throw AppError.invalidInput("OAuth endpoints must be https addresses (or loopback for tests).");
  };
  check(cfg.authorize_url);
  const url = new URL(cfg.authorize_url);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("client_id", cfg.client_id);
  url.searchParams.set("redirect_uri", cfg.redirect_uri);
  url.searchParams.set("state", state);
  url.searchParams.set("code_challenge", challenge);
  url.searchParams.set("code_challenge_method", "S256");
  if (Array.isArray(cfg.scopes) && cfg.scopes.length) url.searchParams.set("scope", cfg.scopes.join(" "));
  if (cfg.audience) url.searchParams.set("audience", cfg.audience);
  return url.toString();
}

/** Start a PKCE sign-in: the browser URL, the state, the redirect. */
export function oauthStart(serverId) {
  const server = (loadConfig().mcp_servers ?? []).find((s) => s.id === serverId);
  const cfg = server?.oauth;
  if (!cfg || !cfg.authorize_url || !cfg.token_url) {
    throw AppError.invalidInput("Save an authorization URL and token URL for this server first.");
  }
  const verifier = crypto.randomBytes(32).toString("base64url");
  const challenge = pkceChallenge(verifier);
  const state = crypto.randomBytes(16).toString("base64url");
  pendingSignIns.set(serverId, { cfg, verifier, state, at: Date.now() });
  return {
    authorize_url: oauthAuthorizeUrl(cfg, state, challenge),
    state,
    redirect_uri: cfg.redirect_uri,
  };
}

/** Finish a PKCE sign-in: check the state, exchange the code, keep the token
 *  in memory only (the config file never holds it). Returns a safe status. */
export async function oauthFinish({ serverId, state, code }) {
  const pending = pendingSignIns.get(String(serverId));
  if (!pending || Date.now() - pending.at > 10 * 60_000) {
    pendingSignIns.delete(String(serverId));
    throw AppError.invalidInput("No sign-in is waiting for that server. Start one again.");
  }
  pendingSignIns.delete(String(serverId));
  if (!state || state !== pending.state) {
    throw AppError.invalidInput("The sign-in state did not match. Please try signing in again.");
  }
  const clean = String(code ?? "").trim();
  if (!clean) throw AppError.invalidInput("Paste the code from the callback address.");
  const body = new URLSearchParams({
    grant_type: "authorization_code",
    code: clean,
    code_verifier: pending.verifier,
    redirect_uri: pending.cfg.redirect_uri,
    client_id: pending.cfg.client_id,
  });
  let response;
  try {
    response = await fetch(pending.cfg.token_url, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
      body: body.toString(),
      redirect: "manual",
      signal: AbortSignal.timeout(30_000),
    });
  } catch (e) {
    throw AppError.internal(`Could not reach the authorization server: ${e}`);
  }
  const text = await response.text();
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    throw AppError.internal("The authorization server sent an unreadable response.");
  }
  if (value.error) {
    const reason = value.error_description ?? (typeof value.error === "string" ? value.error : "authorization failed");
    throw AppError.internal(`Sign-in failed: ${reason}`);
  }
  const accessToken = typeof value.access_token === "string" ? value.access_token : "";
  if (!accessToken) {
    throw AppError.internal("The authorization server did not return an access token.");
  }
  oauthTokens.set(String(serverId), accessToken);
  log("info", "mcp oauth signed in", oauthTokenKey(serverId));
  return {
    server_id: String(serverId),
    signed_in: true,
    message: "Signed in. The access token is kept in this session, not in the config file.",
    token_type: String(value.token_type ?? "Bearer"),
    expires_in: typeof value.expires_in === "number" ? value.expires_in : null,
  };
}

/** The bearer header a live sign-in contributes, or null. */
export function oauthBearer(serverId) {
  const token = oauthTokens.get(String(serverId));
  return token ? { Authorization: `Bearer ${token}` } : null;
}
