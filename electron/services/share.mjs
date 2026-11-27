// Share my assistant: a local web server behind a Cloudflare quick tunnel.
// The page lets a phone read conversations (Viewer) or also chat, answer
// approvals for its own requests and run the self-test (Remote). Tokens are
// made once, shown once, and kept only as SHA-256 hashes. Ported from
// services/share/{mod,auth,pairing,server,tunnel,qr}.rs.

import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { AppError } from "../core/errors.mjs";
import { loadConfig } from "../core/config.mjs";
import { log } from "../core/log.mjs";
import { cloudflaredBinary, ensureCloudflared, cloudflaredStatus } from "./download.mjs";
import { listThreads, loadThread, saveThread } from "./threads.mjs";
import { startChat, cancelJob, decideTool } from "./chat.mjs";
import { engineStatus } from "./engine.mjs";
import { runSelfTest } from "./selftest.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const PAGE = fs.readFileSync(path.join(here, "..", "share", "page.html"), "utf8");
const SCRIPT = fs.readFileSync(path.join(here, "..", "share", "app.js"), "utf8");
const MAX_BODY = 2 * 1024 * 1024;
const MAX_FAILURES_PER_MINUTE = 30;

// ---- Tokens (auth.rs) ----------------------------------------------------

const sha256 = (text) => crypto.createHash("sha256").update(text).digest();
const constantTimeEq = (a, b) =>
  a.length === b.length && crypto.timingSafeEqual(a, b);

const randomToken = () => crypto.randomBytes(24).toString("hex");

function makeTokens() {
  const viewerPlain = randomToken();
  const remotePlain = randomToken();
  return {
    viewerHash: sha256(viewerPlain),
    remoteHash: sha256(remotePlain),
    viewerPlain,
    remotePlain,
    /** The role a presented token grants, if any. */
    check(presented) {
      const t = String(presented ?? "").trim();
      if (t.length !== 48 || !/^[0-9a-fA-F]+$/.test(t)) return null;
      const h = sha256(t);
      // Check both, so the time taken does not say which one matched.
      const isRemote = constantTimeEq(h, this.remoteHash);
      const isViewer = constantTimeEq(h, this.viewerHash);
      return isRemote ? "remote" : isViewer ? "viewer" : null;
    },
  };
}

/** The token from an `Authorization: Bearer …` header. */
const bearer = (header) => {
  const h = header?.trim();
  if (!h) return null;
  const match = /^bearer\s+(.+)$/i.exec(h);
  return match ? match[1].trim() : null;
};

// ---- Pairing (pairing.rs) -------------------------------------------------

const PAIRING_QUERY = "token";
const pairingHash = (token) => sha256(token);
const pairingMatches = (stored, presented) => {
  const t = String(presented ?? "").trim();
  if (t.length !== 48 || !/^[0-9a-fA-F]+$/.test(t)) return false;
  return constantTimeEq(stored, sha256(t));
};
const tokenFromQuery = (url) => {
  const query = url.split("?")[1]?.split("#")[0];
  if (!query) return null;
  for (const pair of query.split("&")) {
    const [key, value] = pair.split("=");
    if (key === PAIRING_QUERY) return value;
  }
  return null;
};
const pairingUrl = (base, token) => `${base.replace(/\/+$/, "")}/?${PAIRING_QUERY}=${token}`;

/**
 * The confirmation code for a link token: the first five bytes of its
 * SHA-256, Crockford-base32'd (no 0/O/1/I/L/U, so reading it aloud is
 * unambiguous). The phone derives the same code from the token it holds.
 */
export function pairCodeFrom(token) {
  const ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const digest = sha256(String(token ?? ""));
  let code = "";
  let acc = 0n;
  let bits = 0;
  for (const byte of digest.subarray(0, 5)) {
    acc = (acc << 8n) | BigInt(byte);
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      const index = Number((acc >> BigInt(bits)) & 0x1fn);
      code += ALPHABET[index];
    }
  }
  return `${code.slice(0, 4)}-${code.slice(4, 8)}`;
}

/** The QR for text, as a compact SVG with quiet zones. */
export async function qrSvg(text) {
  const { QRCode } = await import("qrcode");
  // One-module border: the SVG sits on a card with its own padding.
  return QRCode.toString(text, {
    type: "svg",
    errorCorrectionLevel: "M",
    margin: 2,
    color: { dark: "#14100c", light: "#ffffff" },
  });
}

// ---- The tunnel (tunnel.rs) ----------------------------------------------

/** The first trycloudflare.com link in a log line. */
export function findTunnelUrl(line) {
  const start = line.indexOf("https://");
  if (start < 0) return null;
  const rest = line.slice(start);
  const end = rest.search(/[\s|""]/) === -1 ? rest.length : rest.search(/[\s|""]/);
  const url = rest.slice(0, end);
  const host = url.replace(/^https:\/\//, "");
  const suffix = ".trycloudflare.com";
  return host.endsWith(suffix) &&
    host.length > suffix.length &&
    /^[a-zA-Z0-9.-]+$/.test(host)
    ? url
    : null;
}

/** A Cloudflare quick tunnel, which needs no account. */
function tunnelOpen(binary, port) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      binary,
      ["tunnel", "--no-autoupdate", "--url", `http://127.0.0.1:${port}`],
      { stdio: ["ignore", "ignore", "pipe"], windowsHide: true },
    );
    let stderr = "";
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill();
      reject(new AppError("TUNNEL_FAILED", "The Cloudflare tunnel did not open within 45 seconds. Check the internet connection."));
    }, 45_000);
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
      if (settled) return;
      for (const line of stderr.split("\n")) {
        const url = findTunnelUrl(line);
        if (url) {
          settled = true;
          clearTimeout(timer);
          resolve({ child, url });
          return;
        }
      }
      // Keep only the tail for the error message.
      const lines = stderr.split("\n");
      if (lines.length > 30) stderr = lines.slice(-30).join("\n");
    });
    child.on("exit", () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(new AppError("TUNNEL_FAILED", "cloudflared stopped before the tunnel opened."));
    });
    child.on("error", (e) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(AppError.internal(`cloudflared did not start: ${e}`));
    });
  });
}

// ---- The running share session -------------------------------------------

let running = null;

const nowSecs = () => Math.floor(Date.now() / 1000);

/** Live counters, so Settings → Share shows what the link did. */
const stats = { requests: 0, refused: 0, lastSeen: 0 };

const failureTimes = [];

const tooManyFailures = () => {
  const minuteAgo = Date.now() - 60_000;
  while (failureTimes.length && failureTimes[0] < minuteAgo) failureTimes.shift();
  return failureTimes.length >= MAX_FAILURES_PER_MINUTE;
};

const noteFailure = () => failureTimes.push(Date.now());

function secureHeaders(res, kind) {
  res.writeHead(200 === res.statusCode ? 200 : res.statusCode, {
    "Content-Type": kind,
    "Content-Security-Policy":
      "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    "Referrer-Policy": "no-referrer",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
    "X-Frame-Options": "DENY",
    "Permissions-Policy": "camera=(), microphone=(), geolocation=()",
  });
}

const jsonRes = (res, value, status = 200) => {
  res.statusCode = status;
  secureHeaders(res, "application/json; charset=utf-8");
  res.end(JSON.stringify(value));
};

const errorRes = (res, status, message) => jsonRes(res, { error: message }, status);

/** The chat's current model, as /api/me reports it. */
function currentModel() {
  const cfg = loadConfig();
  const status = engineStatus();
  const id = (cfg.selected_models ?? [])[0] ?? null;
  return {
    name: id ?? status.model ?? null,
    online: cfg.cloud_llm_enabled === true,
    loaded: status.loaded,
  };
}

// Approvals started from a shared link: approval_id → job_id. Only those
// can be answered through /api/decide.
const shareApprovals = new Map();
const chatJobs = new Set();

/** Reads the request body with the same 2 MB cap as the Rust server. */
function readBody(req) {
  return new Promise((resolve, reject) => {
    const declared = Number(req.headers["content-length"] ?? 0);
    if (declared > MAX_BODY) {
      reject(new Error("The request is too large."));
      return;
    }
    const chunks = [];
    let total = 0;
    req.on("data", (chunk) => {
      total += chunk.length;
      if (total > MAX_BODY) {
        reject(new Error("The request is too large."));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      const text = Buffer.concat(chunks).toString("utf8");
      if (!text) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(text));
      } catch {
        reject(new Error("The request is not valid JSON."));
      }
    });
    req.on("error", () => reject(new Error("The request could not be read.")));
  });
}

/** One request to the share server. */
async function handle(req, res, shared, state) {
  const url = req.url ?? "/";
  const urlPath = url.split("?")[0] || "/";
  const method = (req.method ?? "GET").toUpperCase();

  // A small public health check: no token, no data, just "we are up".
  if (method === "GET" && urlPath === "/health") {
    jsonRes(res, { ok: true, service: "localaide-share" });
    return;
  }

  // Every other request needs the one-time pairing token: ?token= or the
  // X-LocalAide-Token header, checked against the in-memory hash.
  const presented = tokenFromQuery(url) ?? req.headers["x-localaide-token"] ?? "";
  if (!pairingMatches(shared.pairingHash, presented)) {
    stats.refused += 1;
    noteFailure();
    errorRes(res, 401, "This server needs its one-time pairing token. Open the pairing link shown on your computer.");
    return;
  }

  // The page holds no data. Its script gets the same token so the browser
  // can load it without knowing the password separately.
  if (method === "GET" && (urlPath === "/" || urlPath === "/index.html")) {
    const page = PAGE.replace("/app.js", `/app.js?${PAIRING_QUERY}=${presented}`);
    secureHeaders(res, "text/html; charset=utf-8");
    res.end(page);
    return;
  }
  if (method === "GET" && urlPath === "/app.js") {
    secureHeaders(res, "text/javascript; charset=utf-8");
    res.end(SCRIPT);
    return;
  }
  if (!urlPath.startsWith("/api/")) {
    errorRes(res, 404, "Not found.");
    return;
  }

  // The confirmation code for the token in the request.
  if (method === "GET" && urlPath === "/api/pair-code") {
    const token = bearer(req.headers.authorization);
    const role = token ? shared.tokens.check(token) : null;
    if (role) {
      jsonRes(res, { code: pairCodeFrom(token), role });
    } else {
      stats.refused += 1;
      errorRes(res, 401, "This link needs its token.");
    }
    return;
  }

  if (tooManyFailures()) {
    await new Promise((r) => setTimeout(r, 1000));
    errorRes(res, 429, "Too many refused requests. Try again in a minute.");
    return;
  }
  const token = bearer(req.headers.authorization);
  const role = token ? shared.tokens.check(token) : null;
  if (!role) {
    stats.refused += 1;
    noteFailure();
    errorRes(res, 401, "This link needs its token. Open the full link you were given.");
    return;
  }
  stats.requests += 1;
  stats.lastSeen = nowSecs();
  const remote = role === "remote";
  const needsRemote = () => errorRes(res, 403, "This link can only view. Ask for a Remote user link to chat.");

  if (method === "GET" && urlPath === "/api/me") {
    jsonRes(res, { role, version: appVersion(), model: currentModel() });
    return;
  }
  if (method === "GET" && urlPath === "/api/threads") {
    jsonRes(res, listThreads());
    return;
  }
  if (method === "GET" && urlPath.startsWith("/api/threads/")) {
    const id = urlPath.slice("/api/threads/".length);
    try {
      jsonRes(res, loadThread(id));
    } catch {
      errorRes(res, 404, "That conversation is not here.");
    }
    return;
  }
  if (method === "POST" && urlPath.startsWith("/api/threads/")) {
    if (!remote) return needsRemote();
    try {
      const body = await readBody(req);
      saveThread({
        threadId: urlPath.slice("/api/threads/".length),
        title: String(body.title ?? "Chat"),
        systemPrompt: null,
        messages: Array.isArray(body.messages) ? body.messages : [],
        folder: null,
      });
      jsonRes(res, { ok: true });
    } catch (e) {
      errorRes(res, 400, String(e?.message ?? e));
    }
    return;
  }
  if (method === "POST" && urlPath === "/api/chat") {
    if (!remote) return needsRemote();
    try {
      const body = await readBody(req);
      // The link chats with the app's current model; it cannot pick another.
      delete body.target;
      const jobId = startChat(body, shared.channelId);
      chatJobs.add(jobId);
      // NDJSON: the job id first, then the events as they arrive.
      res.writeHead(200, { "Content-Type": "application/x-ndjson; charset=utf-8", ...secureFields() });
      res.write(`${JSON.stringify({ type: "job", payload: { job_id: jobId } })}\n`);
      shared.onFrame = (event) => {
        if (!state.stopping) res.write(`${JSON.stringify(event)}\n`);
      };
      shared.jobDone = () => {
        chatJobs.delete(jobId);
        res.end();
      };
    } catch (e) {
      errorRes(res, 400, String(e?.message ?? e));
    }
    return;
  }
  if (method === "POST" && urlPath === "/api/decide") {
    if (!remote) return needsRemote();
    try {
      const body = await readBody(req);
      const approvalId = String(body.approval_id ?? "");
      const jobId = approvalId.split(":")[0];
      if (!chatJobs.has(jobId)) {
        errorRes(res, 403, "Only questions from this link can be answered here.");
        return;
      }
      const decision = String(body.decision ?? "");
      if (!["allow", "allow_chat", "deny"].includes(decision)) {
        errorRes(res, 400, "Answer allow, allow_chat or deny.");
        return;
      }
      jsonRes(res, { delivered: decideTool(approvalId, decision) });
    } catch (e) {
      errorRes(res, 400, String(e?.message ?? e));
    }
    return;
  }
  if (method === "POST" && urlPath === "/api/cancel") {
    if (!remote) return needsRemote();
    try {
      const body = await readBody(req);
      const job = String(body.job_id ?? "");
      if (chatJobs.has(job)) {
        cancelJob(job);
        jsonRes(res, { ok: true });
      } else {
        errorRes(res, 403, "That answer was not started from this link.");
      }
    } catch (e) {
      errorRes(res, 400, String(e?.message ?? e));
    }
    return;
  }
  if (method === "POST" && urlPath === "/api/selftest") {
    if (!remote) return needsRemote();
    res.writeHead(200, { "Content-Type": "application/x-ndjson; charset=utf-8", ...secureFields() });
    const report = await runSelfTest((step) => {
      if (!state.stopping) res.write(`${JSON.stringify({ type: "step", payload: step })}\n`);
    });
    if (!state.stopping) res.write(`${JSON.stringify({ type: "report", payload: report })}\n`);
    res.end();
    return;
  }
  errorRes(res, 404, "Not found.");
}

const secureFields = () => ({
  "Referrer-Policy": "no-referrer",
  "Cache-Control": "no-store",
  "X-Content-Type-Options": "nosniff",
  "X-Frame-Options": "DENY",
});

// The app version, read once.
const appVersion = () => {
  try {
    // eslint-disable-next-line global-require
    return require_pkg().version ?? "0.0.0";
  } catch {
    return "0.0.0";
  }
};
function require_pkg() {
  // eslint-disable-next-line global-require
  return createRequirePkg()("../../package.json");
}
import { createRequire as createRequirePkg } from "node:module";

// ---- Start / stop / status -----------------------------------------------

/** Start sharing. tunnel=false serves on 127.0.0.1 only; lan binds 0.0.0.0. */
export async function startShare({ tunnel = true, lan = false } = {}) {
  if (running) {
    throw AppError.invalidInput("Sharing is already on. Stop it first to make new links.");
  }
  if (tunnel && !cloudflaredBinary()) {
    throw new AppError("CLOUDFLARED_MISSING", "cloudflared is not on this computer yet. Download it in Settings → Share.");
  }
  const tokens = makeTokens();
  const pairingToken = randomToken();
  const pairingHash = pairingHash(pairingToken);
  const shared = {
    tokens,
    pairingHash,
    channelId: Math.floor(Math.random() * 1e9),
    onFrame: null,
    jobDone: null,
  };
  const state = { stopping: false, server: null, tunnelChild: null };

  // The bus frames carry {channelId, message}; the server forwards the
  // events of its own channel as NDJSON.
  const { bus } = await import("../core/channels.mjs");
  const onFrame = ({ channelId, message }) => {
    if (channelId === shared.channelId && shared.onFrame) {
      shared.onFrame(message);
      if (message?.type === "done" || message?.type === "error") {
        const done = shared.jobDone;
        shared.onFrame = null;
        shared.jobDone = null;
        done?.();
      }
    }
  };
  bus.on("frame", onFrame);

  const server = http.createServer((req, res) => {
    handle(req, res, shared, state).catch((e) => {
      try {
        errorRes(res, 500, String(e?.message ?? e));
      } catch {
        /* the socket may be gone */
      }
    });
  });
  const port = await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, lan ? "0.0.0.0" : "127.0.0.1", () => resolve(server.address().port));
  });
  state.server = server;

  let url = `http://127.0.0.1:${port}`;
  if (tunnel) {
    try {
      const opened = await tunnelOpen(cloudflaredBinary(), port);
      state.tunnelChild = opened.child;
      url = opened.url;
    } catch (e) {
      bus.off("frame", onFrame);
      server.close();
      throw e;
    }
  }
  log("info", "sharing started", url);

  const remoteLink = `${url}/#k=${tokens.remotePlain}`;
  const pairCode = pairCodeFrom(tokens.remotePlain);
  const qr = await qrSvg(remoteLink);
  running = {
    stop: () => {
      state.stopping = true;
      bus.off("frame", onFrame);
      state.tunnelChild?.kill();
      server.close();
    },
    port,
    url,
    startedAt: nowSecs(),
    pairCode,
    qr,
  };
  return {
    status: shareStatus(),
    viewer_link: `${url}/#k=${tokens.viewerPlain}`,
    remote_link: remoteLink,
    pair_code: pairCode,
    qr_svg: qr,
    pairing_url: pairingUrl(url, pairingToken),
    pairing_token: pairingToken,
  };
}

/** Stop sharing: the tunnel closes, the server stops, the tokens are gone. */
export function stopShare() {
  if (running) {
    running.stop();
    log("info", "sharing stopped", `port ${running.port}`);
    running = null;
  }
  return shareStatus();
}

export function shareStatus() {
  const cloudflared = cloudflaredStatus();
  if (running) {
    return {
      on: true,
      url: running.url,
      started_at: running.startedAt,
      requests: stats.requests,
      refused: stats.refused,
      last_seen: stats.lastSeen > 0 ? stats.lastSeen : null,
      cloudflared,
    };
  }
  return {
    on: false,
    url: null,
    started_at: null,
    requests: stats.requests,
    refused: stats.refused,
    last_seen: stats.lastSeen > 0 ? stats.lastSeen : null,
    cloudflared,
  };
}

/** The pair info of the running session, QR included. */
export function pairInfo() {
  return {
    pair_code: running?.pairCode ?? "",
    qr_svg: running?.qr ?? "",
    on: Boolean(running),
  };
}

/** Fetch cloudflared now, with progress on the channel. */
export async function fetchCloudflared(onProgress) {
  await ensureCloudflared({ onProgress });
  return cloudflaredStatus();
}

/** Called on quit. */
export function shutdownShare() {
  stopShare();
}
