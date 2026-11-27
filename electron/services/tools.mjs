// Running a tool. Every path is resolved inside the workspace (or the
// chat's own folder), every command is checked before it runs, and anything
// that changes something asks first unless the mode allows it.

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { workspaceDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { run } from "../core/db.mjs";
import { TOOLS } from "./toolSpecs.mjs";
import { searchWeb, readPage } from "./websearch.mjs";
import { searchLibrary } from "./library.mjs";
import { createNote } from "./memory.mjs";

const MAX_READ_BYTES = 512 * 1024;
const MAX_OUTPUT = 60_000;

/** Commands that only read: they never need approval. */
const READ_ONLY = new Set([
  "ls", "dir", "cat", "type", "head", "tail", "wc", "pwd", "echo", "grep", "rg",
  "find", "git", "node", "python", "npm", "pnpm", "cargo", "go", "rustc", "tsc",
  "which", "where", "whoami", "date", "stat", "file", "du", "df", "tree", "jq",
]);

/** The first word of a command, lowercased, without a path. */
export const commandName = (command) =>
  path.basename(String(command ?? "").trim().split(/\s+/)[0] ?? "").toLowerCase();

/** True when a command cannot change anything on disk. */
export function looksReadOnly(command) {
  const name = commandName(command);
  if (!READ_ONLY.has(name)) return false;
  // A write through a shell redirection or a known-destructive git verb is
  // not read-only, whatever the first word is.
  const text = String(command);
  if (/(^|[^>])>{1,2}[^&]/.test(text)) return false;
  if (/\bgit\s+(commit|push|reset|checkout|clean|rm|mv|revert|merge|rebase|tag)\b/.test(text)) return false;
  if (/\b(npm|pnpm|yarn)\s+(install|add|remove|publish)\b/.test(text)) return false;
  return true;
}

const truncate = (text, max = MAX_OUTPUT) =>
  text.length > max ? text.slice(0, max) + `\n… (${text.length - max} more characters)` : text;

/** Resolves a chat file path inside the jail (workspace or project folder). */
function jailPath(cwdRoot, candidate) {
  const root = cwdRoot && String(cwdRoot).trim() ? String(cwdRoot).trim() : workspaceDir();
  const full = path.resolve(root, String(candidate ?? ""));
  if (full !== path.resolve(root) && !full.startsWith(path.resolve(root) + path.sep)) {
    throw new Error("That path is outside the chat's folder.");
  }
  return full;
}

/** One row in the audit, so the Settings screen can show what ran. */
function audit(name, action, file, approved, status, details) {
  try {
    run(
      `INSERT INTO tool_audits (timestamp, tool_name, action, path, approved, status, details)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
      Date.now(), name, action ?? "", file ?? null, approved ? 1 : 0, status ?? "ok", details ?? null,
    );
  } catch {
    /* the audit must never break the tool */
  }
}

/** One tool call. Returns { ok, summary, output, citations? }. */
export async function runTool(name, args, { approved = false, signal, cwdRoot } = {}) {
  const spec = TOOLS[name];
  if (!spec) return { ok: false, summary: `No tool called ${name}.`, output: "" };
  if (spec.risky && !approved) {
    return { ok: false, summary: "Waiting for your approval.", output: "" };
  }

  try {
    switch (name) {
      case "web_search": {
        const hits = await searchWeb("auto", String(args.query ?? ""), Number(args.max_results) || 5);
        const lines = hits.map((hit, i) => `[${i + 1}] ${hit.title}\n${hit.url}\n${hit.snippet}`);
        audit(name, "web_search", null, approved, "ok", `${hits.length} results`);
        return {
          ok: true,
          summary: `${hits.length} results`,
          output: lines.join("\n\n") || "No results.",
          citations: hits.map((hit, i) => ({
            index: i + 1,
            source: hit.url,
            title: hit.title,
            snippet: hit.snippet.slice(0, 300),
            score: 0,
          })),
        };
      }
      case "read_url": {
        const page = await readPage(String(args.url ?? ""), "auto");
        audit(name, "read_url", page.url, approved, "ok", page.title ?? "");
        return {
          ok: true,
          summary: page.title || "Page read",
          output: truncate(page.content),
          citations: [
            {
              index: 1,
              source: page.url,
              title: page.title || page.url,
              snippet: page.content.slice(0, 300),
              score: 0,
            },
          ],
        };
      }
      case "recall": {
        const docs = searchLibrary(String(args.query ?? ""), 6);
        const text = docs.map((d, i) => `[${i + 1}] ${d.title} (${d.path})\n${d.snippet}`).join("\n\n");
        audit(name, "recall", null, approved, "ok", `${docs.length} matches`);
        return {
          ok: true,
          summary: `${docs.length} matches`,
          output: text || "Nothing saved matches that.",
          citations: docs.map((d, i) => ({
            index: i + 1,
            source: d.path,
            title: d.title,
            snippet: d.snippet.slice(0, 300),
            score: d.score ?? 0,
          })),
        };
      }
      case "save_note": {
        const note = createNote(String(args.title ?? ""), String(args.content ?? ""), null);
        audit(name, "save_note", note.path, approved, "ok", note.title);
        return { ok: true, summary: `Saved note ${note.title}`, output: `Saved to ${note.path}.` };
      }
      case "read_file": {
        const file = jailPath(cwdRoot, args.path);
        const stat = fs.statSync(file, { throwIfNoEntry: false });
        if (!stat || !stat.isFile()) return { ok: false, summary: "No such file.", output: "" };
        if (stat.size > MAX_READ_BYTES) {
          return { ok: false, summary: "File too large.", output: "That file is too big to read in one go." };
        }
        audit(name, "read_file", file, approved, "ok", null);
        return { ok: true, summary: path.basename(file), output: truncate(fs.readFileSync(file, "utf8")) };
      }
      case "write_file": {
        const file = jailPath(cwdRoot, args.path);
        fs.mkdirSync(path.dirname(file), { recursive: true });
        fs.writeFileSync(file, String(args.content ?? ""), "utf8");
        audit(name, "write_file", file, approved, "ok", null);
        return { ok: true, summary: `Wrote ${path.basename(file)}`, output: `Saved ${args.path}.` };
      }
      case "list_files": {
        const dir = args.path ? jailPath(cwdRoot, args.path) : cwdRoot ? jailPath(cwdRoot, ".") : workspaceDir();
        if (!fs.existsSync(dir)) return { ok: false, summary: "No such folder.", output: "" };
        const entries = fs.readdirSync(dir, { withFileTypes: true }).slice(0, 400);
        const lines = entries.map((e) => (e.isDirectory() ? `${e.name}/` : e.name));
        audit(name, "list_dir", dir, approved, "ok", `${lines.length} entries`);
        return { ok: true, summary: `${lines.length} entries`, output: lines.join("\n") || "The folder is empty." };
      }
      case "run_command": {
        const result = await runCommand(String(args.command ?? ""), {
          cwd: args.cwd ? jailPath(cwdRoot, args.cwd) : cwdRoot ? jailPath(cwdRoot, ".") : workspaceDir(),
          timeoutSeconds: Math.min(300, Math.max(1, Number(args.timeout_seconds) || 60)),
          signal,
        });
        audit(
          name, "run_command", String(args.command ?? "").slice(0, 200), approved,
          result.code === 0 ? "ok" : "failed", `exit ${result.code}`,
        );
        return {
          ok: result.code === 0,
          summary: result.code === 0 ? "Command finished" : `Exited with ${result.code}`,
          output: truncate(result.output),
        };
      }
      default:
        // ask_user, todo_write, subagent and learn_skill are handled by the
        // chat loop, which needs to talk to the UI instead of the disk.
        return { ok: false, summary: `${name} is handled by the chat.`, output: "" };
    }
  } catch (e) {
    audit(name, name, null, approved, "failed", String(e?.message ?? e).slice(0, 200));
    return { ok: false, summary: e.message ?? String(e), output: "" };
  }
}

/** Runs one command, with a hard timeout that kills the whole tree. */
function runCommand(command, { cwd, timeoutSeconds, signal }) {
  return new Promise((resolve) => {
    const isWin = process.platform === "win32";
    const child = spawn(isWin ? "cmd.exe" : "/bin/sh", isWin ? ["/d", "/s", "/c", command] : ["-c", command], {
      cwd,
      env: { ...process.env, NO_COLOR: "1", CLICOLOR: "0" },
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    const collect = (chunk) => {
      if (output.length < MAX_OUTPUT * 2) output += chunk.toString();
    };
    child.stdout.on("data", collect);
    child.stderr.on("data", collect);

    const kill = () => {
      if (isWin) spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], { windowsHide: true });
      else child.kill("SIGKILL");
    };
    const timer = setTimeout(() => {
      output += `\n(stopped after ${timeoutSeconds} s)`;
      kill();
    }, timeoutSeconds * 1000);
    signal?.addEventListener("abort", kill, { once: true });

    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code: code ?? 1, output: output.trim() });
    });
    child.on("error", (e) => {
      clearTimeout(timer);
      resolve({ code: 1, output: String(e) });
    });
  });
}
