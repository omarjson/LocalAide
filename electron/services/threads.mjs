// Saved chats: one JSON file per conversation under ~/.aiassistant/conversations.
// The file shape matches the Rust StoredThread, so chats saved by the Tauri
// build open here unchanged.

import fs from "node:fs";
import path from "node:path";
import { conversationsDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";

const nowSecs = () => Math.floor(Date.now() / 1000);

/** Thread ids become file names, so only a short plain token is accepted. */
export function validThreadId(id) {
  return (
    typeof id === "string" &&
    id.length > 0 &&
    id.length <= 64 &&
    /^[A-Za-z0-9_-]+$/.test(id)
  );
}

const fileFor = (id) => {
  if (!validThreadId(id)) throw AppError.invalidInput("That conversation id is not valid.");
  return path.join(conversationsDir(), `${id}.json`);
};

export function readThread(id) {
  try {
    return JSON.parse(fs.readFileSync(fileFor(id), "utf8"));
  } catch {
    return null;
  }
}

export function ensureThread(id, { title = "Chat" } = {}) {
  const existing = readThread(id);
  if (existing) return existing;
  if (!validThreadId(id)) return null;
  const now = nowSecs();
  const thread = {
    id,
    title,
    system_prompt: null,
    created_at: now,
    updated_at: now,
    messages: [],
    folder: null,
  };
  writeThread(thread);
  return thread;
}

function writeThread(thread) {
  const file = fileFor(thread.id);
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(thread, null, 2));
  fs.renameSync(tmp, file);
}

export function appendMessage(threadId, message) {
  if (!validThreadId(threadId)) return;
  const thread = readThread(threadId) ?? ensureThread(threadId);
  if (!thread) return;
  thread.messages = Array.isArray(thread.messages) ? thread.messages : [];
  thread.messages.push(message);
  thread.updated_at = nowSecs();
  if (!thread.title || thread.title === "Chat") {
    const first = thread.messages.find((m) => m.role === "user" && String(m.content ?? "").trim());
    if (first) thread.title = String(first.content).trim().slice(0, 60);
  }
  writeThread(thread);
}

export function listThreads() {
  const dir = conversationsDir();
  const out = [];
  for (const name of fs.readdirSync(dir)) {
    if (!name.endsWith(".json")) continue;
    try {
      const stored = JSON.parse(fs.readFileSync(path.join(dir, name), "utf8"));
      out.push({
        id: stored.id,
        title: stored.title ?? "Chat",
        system_prompt: stored.system_prompt ?? null,
        created_at: stored.created_at ?? 0,
        updated_at: stored.updated_at ?? 0,
        message_count: Array.isArray(stored.messages) ? stored.messages.length : 0,
        folder: stored.folder ?? null,
      });
    } catch {
      /* a half-written file is skipped, not fatal */
    }
  }
  return out.sort((a, b) => b.updated_at - a.updated_at);
}

export function saveThread({ threadId, title, systemPrompt, messages, folder }) {
  if (!validThreadId(threadId)) throw AppError.invalidInput("That conversation id is not valid.");
  const existing = readThread(threadId);
  const thread = {
    id: threadId,
    title: String(title ?? "Chat"),
    system_prompt: systemPrompt ?? null,
    created_at: existing?.created_at ?? nowSecs(),
    updated_at: nowSecs(),
    messages: Array.isArray(messages) ? messages : [],
    folder: folder || existing?.folder || null,
  };
  writeThread(thread);
  return thread;
}

export function loadThread(id) {
  return readThread(id)?.messages ?? [];
}

export function deleteThread(id) {
  const file = fileFor(id);
  if (fs.existsSync(file)) fs.unlinkSync(file);
}

/** The saved chat as Markdown or raw JSON, for the export dialog. */
export function exportThread(id, format) {
  const thread = readThread(id);
  if (!thread) throw AppError.invalidInput("Save the chat first. It has no messages yet.");
  const stem = fileStem(thread.title);
  if (String(format).toLowerCase() === "json") {
    return { content: JSON.stringify(thread, null, 2), filename: `${stem}.json` };
  }
  let md = `# ${thread.title}\n\n`;
  if (thread.system_prompt?.trim()) md += `**System Prompt:** ${thread.system_prompt}\n\n---\n\n`;
  for (const msg of thread.messages ?? []) {
    md += `### ${msg.role === "user" ? "User" : "Assistant"}\n\n${msg.content ?? ""}\n\n`;
  }
  return { content: md, filename: `${stem}.md` };
}

/** A title that every file system accepts. */
export function fileStem(title) {
  const cleaned = String(title ?? "")
    .replace(/[^\p{L}\p{N}_-]+/gu, "_")
    .slice(0, 60)
    .replace(/^_+|_+$/g, "");
  return cleaned || "chat";
}
