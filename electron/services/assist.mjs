// Small helpers around the chat: improving a prompt, suggesting the next
// one, learned starter prompts, opening files, and the interface scale.
// Ported from commands/assist.rs.

import fs from "node:fs";
import path from "node:path";
import { execFile } from "node:child_process";
import { shell, BrowserWindow } from "electron";
import { appDir, conversationsDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { quickAnswer } from "./chat.mjs";

const ENHANCE_PROMPT =
  "You rewrite a user's message to an AI assistant so it gets a better answer. Keep the user's intent, language and facts. Make it clear and specific: the goal, the needed context, and the form of the answer. Add nothing the user did not imply, and do not answer it. Reply with the rewritten message only, no preface and no quotes.";

const NEXT_PROMPT =
  "Suggest the 3 most useful next messages the user could send in this conversation, written as the user would write it, in their language, each with a different idea. At most 12 words each. Reply with exactly 3 lines, one message per line, no numbering.";

const TITLE_PROMPT =
  "Write a short title for this conversation: 3 to 6 words, no quotes, no period, in the language of the conversation. Reply with the title only.";

/** Rewrite the draft in the chat box into a clearer prompt. */
export async function enhancePrompt(text, target) {
  const draft = String(text ?? "").trim().slice(0, 8000);
  if (!draft) throw AppError.invalidInput("Write something first.");
  const better = await quickAnswer(draft, { system: ENHANCE_PROMPT, maxTokens: 600, target });
  if (!better) throw AppError.internal("The model gave no prompt back.");
  return better;
}

/** Up to three suggested next messages, from the last question and answer. */
export async function suggestNext(question, answer, target) {
  const q = String(question ?? "").slice(0, 1500);
  const tail = String(answer ?? "").slice(-2500);
  if (!q.trim() && !tail.trim()) return [];
  const text = `The user asked:\n${q}\n\nThe assistant answered (end of it):\n${tail}`;
  const next = await quickAnswer(text, { system: NEXT_PROMPT, maxTokens: 160, target });
  return next
    .split("\n")
    .map((line) => {
      let l = line.trim();
      for (const prefix of ["- ", "* ", "1. ", "2. ", "3. ", "1) ", "2) ", "3) "]) {
        if (l.startsWith(prefix)) {
          l = l.slice(prefix.length);
          break;
        }
      }
      return l.trim();
    })
    .filter((line) => line.length >= 3 && line.length <= 160)
    .slice(0, 3);
}

/** A compact title for the chat, or null when the model declined. */
export async function chatTitle(question, answer, target) {
  const q = String(question ?? "").slice(0, 1200);
  const tail = String(answer ?? "").slice(-1500);
  if (!q.trim()) return null;
  const text = `The conversation starts with this question:\n${q}\n\nThe assistant answered (end of it):\n${tail}`;
  const raw = await quickAnswer(text, { system: TITLE_PROMPT, maxTokens: 32, target });
  const title = (raw.split("\n")[0] ?? "").trim().replaceAll('"', "").trim();
  if (title.length >= 3 && title.length <= 60) {
    return title.replace(/[.,;:\-]+$/, "");
  }
  return null;
}

// ---- Starter prompts learned from saved chats --------------------------

const normalized = (text) =>
  text
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
    .slice(0, 8)
    .join(" ");

const titleOf = (prompt) => {
  const title = prompt.split(/\s+/).slice(0, 4).join(" ").replace(/[^\p{L}\p{N}_-]+$/u, "");
  return title.charAt(0).toUpperCase() + title.slice(1);
};

/** The ways the user's own saved chats most often begin, newest first. */
export function learnedStarters(dir, limit) {
  let names;
  try {
    names = fs.readdirSync(dir);
  } catch {
    return [];
  }
  const seen = new Map();
  const files = names
    .filter((n) => n.endsWith(".json"))
    .sort()
    .slice(-300)
    .reverse();
  for (const name of files) {
    let saved;
    try {
      saved = JSON.parse(fs.readFileSync(path.join(dir, name), "utf8"));
    } catch {
      continue;
    }
    const first = (saved?.messages ?? []).find((m) => m?.role === "user");
    const prompt = String(first?.content ?? "").trim();
    const words = prompt.split(/\s+/).filter(Boolean).length;
    if (words < 3 || words > 40 || prompt.length > 220 || prompt.includes("```")) continue;
    const key = normalized(prompt);
    const entry = seen.get(key) ?? { uses: 0, at: saved.updated_at ?? 0, prompt };
    entry.uses += 1;
    if ((saved.updated_at ?? 0) > entry.at) {
      entry.at = saved.updated_at ?? 0;
      entry.prompt = prompt;
    }
    seen.set(key, entry);
  }
  return [...seen.values()]
    .sort((a, b) => b.uses - a.uses || b.at - a.at)
    .slice(0, limit)
    .map(({ uses, prompt }) => ({ title: titleOf(prompt), prompt, uses }));
}

export const chatStarters = () => learnedStarters(conversationsDir(), 4);

// ---- Opening the files a chat made --------------------------------------

const RUNNABLE = new Set([
  "exe", "bat", "cmd", "com", "msi", "msix", "appx", "ps1", "psm1", "vbs", "vbe", "js", "jse",
  "wsf", "wsh", "scr", "lnk", "url", "reg", "hta", "cpl", "pif", "sh", "bash", "zsh", "fish",
  "command", "tool", "app", "pkg", "dmg", "jar", "py", "pyw", "pl", "rb", "appimage", "desktop",
  "run", "bin", "workflow", "scpt", "applescript", "inf",
]);

/** Programs and scripts are never opened with their default app. */
function runsWhenOpened(file) {
  const ext = path.extname(file).slice(1).toLowerCase();
  return RUNNABLE.has(ext);
}

/** Resolves a chat file path inside its folder (or the shared workspace). */
export function chatFile(folder, pathStr) {
  const root = folder && String(folder).trim() ? String(folder).trim() : path.join(appDir(), "workspace");
  let relative = String(pathStr ?? "").trim();
  if (relative.startsWith("workspace/")) relative = relative.slice("workspace/".length);
  const real = path.resolve(root, relative || ".");
  if (real !== root && !real.startsWith(root + path.sep)) {
    throw AppError.invalidInput("That path is outside the chat's folder.");
  }
  if (!fs.existsSync(real)) {
    throw AppError.invalidInput(`${pathStr} is not there any more.`);
  }
  return real;
}

export function openFile(folder, pathStr) {
  const real = chatFile(folder, pathStr);
  if (runsWhenOpened(real)) {
    throw AppError.invalidInput(
      "This file is a program or script, so LocalAide does not run it. Open it with an editor, or show it in its folder.",
    );
  }
  return shell.openPath(real).then((error) => {
    if (error) throw AppError.internal(`The file could not be opened: ${error}`);
  });
}

export function revealFile(folder, pathStr) {
  const real = chatFile(folder, pathStr);
  if (!String(pathStr ?? "").trim()) {
    return shell.openPath(real).then((error) => {
      if (error) throw AppError.internal(`The folder could not be shown: ${error}`);
    });
  }
  shell.showItemInFolder(real);
}

/** Editors found on this computer, for "Open with". */
export function editors() {
  const out = [];
  if (process.platform === "win32") {
    const local = process.env.LOCALAPPDATA ? path.join(process.env.LOCALAPPDATA) : null;
    const programs = process.env.ProgramFiles;
    const candidates = [
      ["vscode", "Visual Studio Code", local ? path.join(local, "Programs/Microsoft VS Code/Code.exe") : null],
      ["vscode", "Visual Studio Code", programs ? path.join(programs, "Microsoft VS Code/Code.exe") : null],
      ["cursor", "Cursor", local ? path.join(local, "Programs/cursor/Cursor.exe") : null],
      ["notepadpp", "Notepad++", programs ? path.join(programs, "Notepad++/notepad++.exe") : null],
      ["notepad", "Notepad", process.env.SystemRoot ? path.join(process.env.SystemRoot, "System32/notepad.exe") : null],
    ];
    for (const [id, name, file] of candidates) {
      if (file && fs.existsSync(file) && !out.some((a) => a.id === id)) {
        out.push({ id, name, program: file });
      }
    }
  } else if (process.platform === "darwin") {
    for (const [id, name, app] of [
      ["vscode", "Visual Studio Code", "Visual Studio Code.app"],
      ["cursor", "Cursor", "Cursor.app"],
      ["zed", "Zed", "Zed.app"],
      ["sublime", "Sublime Text", "Sublime Text.app"],
      ["textedit", "TextEdit", "TextEdit.app"],
      ["preview", "Preview", "Preview.app"],
    ]) {
      for (const root of ["/Applications", "/System/Applications"]) {
        const file = path.join(root, app);
        if (fs.existsSync(file)) {
          out.push({ id, name, program: app });
          break;
        }
      }
    }
  } else {
    for (const [id, name, bin] of [
      ["vscode", "Visual Studio Code", "code"],
      ["zed", "Zed", "zed"],
      ["gnome-text-editor", "Text Editor", "gnome-text-editor"],
      ["gedit", "gedit", "gedit"],
      ["kate", "Kate", "kate"],
    ]) {
      const found = lookupOnPath(bin);
      if (found) out.push({ id, name, program: found });
    }
  }
  return out;
}

function lookupOnPath(bin) {
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    const file = path.join(dir, bin + (process.platform === "win32" ? ".exe" : ""));
    try {
      if (fs.existsSync(file)) return file;
    } catch {
      /* keep looking */
    }
  }
  return null;
}

export function openWithApps() {
  const apps = editors().map(({ id, name }) => ({ id, name }));
  if (process.platform === "win32" || process.platform === "darwin") {
    apps.push({ id: "other", name: "Another app…" });
  }
  return apps;
}

export function openFileWith(folder, pathStr, appId) {
  const real = chatFile(folder, pathStr);
  const apps = editors();
  if (appId === "other") {
    if (process.platform === "win32") {
      const system = process.env.SystemRoot ?? "C:\\Windows";
      execFile(path.join(system, "System32", "rundll32.exe"), ["shell32.dll,OpenAs_RunDLL", real], (e) => {
        if (e) throw AppError.internal("The app chooser did not open.");
      });
      return;
    }
    throw AppError.invalidInput("Pick one of the listed apps.");
  }
  const app = apps.find((a) => a.id === appId);
  if (!app) throw AppError.invalidInput("That app is not on this computer.");
  if (process.platform === "darwin") {
    execFile("open", ["-a", app.program, real], (e) => {
      if (e) throw AppError.internal("The app did not start.");
    });
  } else {
    execFile(app.program, [real], (e) => {
      if (e) throw AppError.internal("The app did not start.");
    });
  }
}

// ---- Interface scale -----------------------------------------------------

const scaleFile = () => path.join(appDir(), "ui-scale.json");

export function savedScale() {
  try {
    const value = Number(fs.readFileSync(scaleFile(), "utf8").trim());
    if (!Number.isFinite(value)) return 1.0;
    return Math.min(2.5, Math.max(0.7, value));
  } catch {
    return 1.0;
  }
}

export function uiScaleGet() {
  return savedScale();
}

/** Zoom the whole window and remember it for the next start. */
export function uiScaleSet(scale) {
  if (!Number.isFinite(Number(scale))) {
    throw AppError.invalidInput("The scale must be a number.");
  }
  const clamped = Math.round(Math.min(2.5, Math.max(0.7, Number(scale))) * 100) / 100;
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.setZoomFactor(clamped);
  }
  fs.writeFileSync(scaleFile(), String(clamped));
  return clamped;
}

/** The app's saved scale applied to the window at startup. */
export function applyUiScale() {
  const scale = savedScale();
  if (scale !== 1.0) {
    try {
      for (const win of BrowserWindow.getAllWindows()) win.webContents.setZoomFactor(scale);
    } catch {
      /* the window may not exist yet */
    }
  }
}
