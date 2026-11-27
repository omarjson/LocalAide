// The OpenCode link: talk to a server the user runs (Settings →
// Integrations). Health reads /global/health; a hand-off creates a session
// and posts the task without waiting for the answer. Ported from
// services/opencode.rs.

import { AppError } from "../core/errors.mjs";
import { loadConfig, updateConfig } from "../core/config.mjs";

const DEFAULT_URL = "http://127.0.0.1:4096";
const MAX_RESPONSE_BYTES = 2 * 1024 * 1024;
const MAX_TASK_CHARS = 20_000;

export const defaultUrl = () => DEFAULT_URL;

const baseUrl = () => {
  const saved = loadConfig().opencode?.base_url?.trim().replace(/\/+$/, "");
  return saved || null;
};

const authHeaders = () => {
  const password = loadConfig().opencode?.password;
  return password ? { Authorization: `Basic ${Buffer.from(`:${password}`).toString("base64")}` } : {};
};

async function request(method, path, body) {
  const base = baseUrl();
  if (!base) {
    throw AppError.invalidInput("Link an OpenCode server first. Add its address in Settings → Integrations.");
  }
  let response;
  try {
    response = await fetch(`${base}${path}`, {
      method,
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...authHeaders(),
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
      signal: AbortSignal.timeout(30_000),
    });
  } catch (e) {
    throw new AppError(
      "OPENCODE_UNREACHABLE",
      'OpenCode is not answering at that address. Start it with "opencode serve" and try again.',
      { detail: String(e), retryable: true },
    );
  }
  if (response.status === 401 || response.status === 403) {
    throw new AppError("OPENCODE_AUTH", "OpenCode asked for a password. Enter the one set in OPENCODE_SERVER_PASSWORD.");
  }
  if (!response.ok) {
    throw new AppError("OPENCODE_ERROR", `OpenCode answered with an error (${response.status}).`);
  }
  const text = await response.text();
  if (text.length > MAX_RESPONSE_BYTES) {
    throw AppError.invalidInput("OpenCode sent too much data.");
  }
  return text;
}

/** `GET /global/health`: the server's version, when it answers. */
export async function check() {
  if (!baseUrl()) return { version: null };
  const text = await request("GET", "/global/health");
  try {
    const body = JSON.parse(text);
    return { version: typeof body?.version === "string" ? body.version : null };
  } catch {
    return { version: null };
  }
}

/** A short title for the session: the task's first line, capped. */
const sessionTitle = (given, task) => {
  const source = String(given ?? "").trim() || task.split("\n")[0].trim();
  return source.slice(0, 80) || "OpenCode task";
};

function parseSessionId(body) {
  let id;
  try {
    id = JSON.parse(body)?.id;
  } catch {
    id = undefined;
  }
  if (typeof id !== "string" || !id || id.includes("/") || id.includes("..") || id.includes("\\")) {
    throw AppError.internal("OpenCode did not send a session id back.");
  }
  return id;
}

/** Create a session and send it the task without waiting for the answer. */
export async function send(title, task) {
  const clean = String(task ?? "").trim();
  if (!clean) {
    throw AppError.invalidInput("There is nothing to send to OpenCode.");
  }
  if ([...clean].length > MAX_TASK_CHARS) {
    throw AppError.invalidInput("That is too long to send to OpenCode.");
  }
  const sessionTitleOut = sessionTitle(title, clean);
  const text = await request("POST", "/session", { title: sessionTitleOut });
  const sessionId = parseSessionId(text);
  await request("POST", `/session/${sessionId}/prompt_async`, {
    parts: [{ type: "text", text: clean }],
  });
  return { session_id: sessionId, title: sessionTitleOut };
}

/** Save the address (and optionally the password) the app talks to. */
export function set({ baseUrl: url, password }) {
  updateConfig((cfg) => {
    cfg.opencode.base_url = url ? String(url).trim().replace(/\/+$/, "") : null;
    if (password !== undefined && password !== null) {
      cfg.opencode.password = String(password) || null;
    }
  });
  return get();
}

export function get() {
  const cfg = loadConfig();
  return {
    base_url: cfg.opencode?.base_url ?? null,
    default_url: DEFAULT_URL,
    has_password: Boolean(cfg.opencode?.password),
  };
}
