// Prompt skills: markdown files with a small frontmatter header under
// ~/.aiassistant/skills/, plus usage counters for the "save this as a
// skill?" hint. Ported from services/skills/mod.rs.

import fs from "node:fs";
import path from "node:path";
import { skillsDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";

export const MAX_SKILL_BYTES = 256 * 1024;
const MAX_SKILLS = 200;

const oneLine = (value) => String(value ?? "").split(/\s+/).filter(Boolean).join(" ");

/** File-name-safe slug: lowercase, alphanumerics and dashes only. */
const slugify = (name) => {
  let slug = "";
  for (const c of String(name)) {
    if (/[a-zA-Z0-9]/.test(c)) slug += c.toLowerCase();
    else if ((c === " " || c === "-" || c === "_") && !slug.endsWith("-")) slug += "-";
  }
  return slug.replace(/^-+|-+$/g, "");
};

/** Split a leading --- frontmatter block into key/value pairs. */
function splitFrontmatter(raw) {
  const rest = raw.trimStart();
  if (rest.startsWith("---")) {
    const after = rest.slice(3);
    const end = after.indexOf("\n---");
    if (end >= 0) {
      const front = new Map();
      for (const line of after.slice(0, end).split("\n")) {
        const at = line.indexOf(":");
        if (at > 0) front.set(line.slice(0, at).trim(), line.slice(at + 1).trim());
      }
      const body = after.slice(end + 4).replace(/^[\n\r]+/, "");
      return [front, body];
    }
  }
  return [new Map(), raw];
}

/** Serialize a skill back to its markdown file format. */
function renderSkillFile(skill) {
  let out = "---\n";
  out += `name: ${skill.name}\n`;
  out += `description: ${skill.description}\n`;
  if (skill.tags.length) out += `tags: ${skill.tags.map(oneLine).join(", ")}\n`;
  if (skill.source) out += `source: ${oneLine(skill.source)}\n`;
  if (skill.license) out += `license: ${oneLine(skill.license)}\n`;
  out += "---\n\n";
  out += `${skill.body.trim()}\n`;
  return out;
}

function readSkillFile(file) {
  const stat = fs.statSync(file);
  if (stat.size > MAX_SKILL_BYTES) throw AppError.invalidInput("Skill file exceeds the size limit");
  const raw = fs.readFileSync(file, "utf8");
  const [front, body] = splitFrontmatter(raw);
  const stem = path.basename(file, ".md");
  const tags = front.has("tags")
    ? String(front.get("tags"))
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean)
    : [];
  return {
    name: front.get("name") || stem,
    description: front.get("description") ?? "",
    tags,
    body: body.trim(),
    path: file,
    source: front.get("source") || null,
    license: front.get("license") || null,
  };
}

/** Only files inside the skills folder are addressable — no traversal. */
function guardPath(pathStr) {
  const dir = path.resolve(skillsDir());
  const requested = path.isAbsolute(String(pathStr)) ? path.resolve(String(pathStr)) : path.resolve(dir, String(pathStr));
  if (requested !== dir && !requested.startsWith(dir + path.sep)) {
    throw AppError.invalidInput("Skill paths must stay inside the skills folder");
  }
  if (!fs.existsSync(requested)) throw AppError.invalidInput("Skill file does not exist");
  return requested;
}

function freePath(name) {
  let slug = slugify(name) || "skill";
  let candidate = path.join(skillsDir(), `${slug}.md`);
  let n = 2;
  while (fs.existsSync(candidate)) {
    candidate = path.join(skillsDir(), `${slug}-${n}.md`);
    n += 1;
  }
  return candidate;
}

export function listSkills() {
  fs.mkdirSync(skillsDir(), { recursive: true });
  let names;
  try {
    names = fs.readdirSync(skillsDir());
  } catch (e) {
    throw AppError.internal(`Could not read skills folder: ${e}`);
  }
  const skills = [];
  for (const name of names.sort()) {
    const file = path.join(skillsDir(), name);
    if (!name.toLowerCase().endsWith(".md")) continue;
    try {
      skills.push(readSkillFile(file));
    } catch {
      /* an unreadable file is skipped, not fatal */
    }
  }
  return skills.sort((a, b) => a.name.localeCompare(b.name));
}

export function createSkill(name, description, tags, body, source = null, license = null) {
  const cleanName = oneLine(name);
  if (String(body ?? "").length > MAX_SKILL_BYTES) {
    throw AppError.invalidInput("This skill is too long to save (over 256 KB).");
  }
  if (!cleanName || !String(body ?? "").trim()) {
    throw AppError.invalidInput("A skill needs a name and a prompt body");
  }
  const existing = listSkills();
  if (existing.length >= MAX_SKILLS) {
    throw AppError.invalidInput(`The skills folder holds ${MAX_SKILLS} skills already; delete one first`);
  }
  if (existing.some((s) => s.name.toLowerCase() === cleanName.toLowerCase())) {
    throw AppError.invalidInput(`A skill named '${cleanName}' already exists`);
  }
  fs.mkdirSync(skillsDir(), { recursive: true });
  const file = freePath(cleanName);
  const skill = {
    name: cleanName,
    description: oneLine(description),
    tags: Array.isArray(tags) ? tags.map(String) : [],
    body: String(body ?? ""),
    path: file,
    source: source ? oneLine(source) || null : null,
    license: license ? oneLine(license) || null : null,
  };
  fs.writeFileSync(file, renderSkillFile(skill), "utf8");
  return skill;
}

export function updateSkill(pathStr, name, description, tags, body) {
  const file = guardPath(pathStr);
  const previous = readSkillFile(file);
  const skill = {
    name: oneLine(name),
    description: oneLine(description),
    tags: Array.isArray(tags) ? tags.map(String) : [],
    body: String(body ?? ""),
    path: file,
    source: previous.source,
    license: previous.license,
  };
  if (!skill.name || !skill.body.trim()) {
    throw AppError.invalidInput("A skill needs a name and a prompt body");
  }
  fs.writeFileSync(file, renderSkillFile(skill), "utf8");
  return skill;
}

export function readSkill(pathStr) {
  return readSkillFile(guardPath(pathStr));
}

export function deleteSkill(pathStr) {
  fs.unlinkSync(guardPath(pathStr));
}

// ---- Learned skills (the model saving one on its own) -------------------

const learnRecord = (skill, previous, stamp) => ({ path: skill.path, name: skill.name, previous });

/**
 * Save a skill the model learned from a chat: a new one, or a better
 * version of the skill with the same name. What it replaced is kept in
 * .history/, so the chat's Undo can put it back.
 */
export function learnSkill(name, description, body) {
  const cleanName = oneLine(name);
  if (!cleanName || !String(body ?? "").trim()) {
    throw AppError.invalidInput("A skill needs a name and instructions.");
  }
  const existing = listSkills().find((s) => s.name.toLowerCase() === cleanName.toLowerCase());
  let skill;
  let previous = null;
  if (existing) {
    previous = fs.readFileSync(existing.path, "utf8");
    const desc = String(description ?? "").trim() || existing.description;
    const tags = existing.tags.includes("learned") ? existing.tags : [...existing.tags, "learned"];
    skill = updateSkill(existing.path, existing.name, desc, tags, body);
  } else {
    skill = createSkill(cleanName, description, ["learned"], body);
  }
  const created = previous === null;
  const history = path.join(skillsDir(), ".history");
  fs.mkdirSync(history, { recursive: true });
  let changeId = `${Date.now()}-${slugify(skill.name)}`;
  let n = 2;
  while (fs.existsSync(path.join(history, `${changeId}.json`))) {
    changeId = `${Date.now()}-${slugify(skill.name)}-${n}`;
    n += 1;
  }
  fs.writeFileSync(path.join(history, `${changeId}.json`), JSON.stringify(learnRecord(skill, previous)), "utf8");
  // Keep the newest 100 changes.
  try {
    const files = fs.readdirSync(history).sort();
    for (const old of files.slice(0, Math.max(0, files.length - 100))) {
      fs.unlinkSync(path.join(history, old));
    }
  } catch {
    /* best effort */
  }
  return { skill, changeId, created };
}

/** Undo one learned change: put the old text back, or remove a new skill. */
export function undoSkill(changeId) {
  if (!/^[A-Za-z0-9-]+$/.test(String(changeId ?? ""))) {
    throw AppError.invalidInput("That change is not known.");
  }
  const file = path.join(skillsDir(), ".history", `${changeId}.json`);
  let record;
  try {
    record = JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    throw AppError.invalidInput("That change was already undone or is too old.");
  }
  if (record.previous) {
    const restore = guardPath(record.path);
    fs.writeFileSync(restore, record.previous, "utf8");
  } else {
    try {
      fs.unlinkSync(guardPath(record.path));
    } catch {
      /* already gone */
    }
  }
  fs.unlinkSync(file);
  return record.name;
}

// ---- Usage counters (the save-as-skill hint) ----------------------------

const usagePath = () => path.join(skillsDir(), "usage.json");

function loadUsage() {
  try {
    return JSON.parse(fs.readFileSync(usagePath(), "utf8"));
  } catch {
    return {};
  }
}

function saveUsage(map) {
  fs.mkdirSync(skillsDir(), { recursive: true });
  fs.writeFileSync(usagePath(), JSON.stringify(map, null, 2), "utf8");
}

/** Record that a custom system prompt was used once. */
export function recordPromptUsage(prompt) {
  const key = String(prompt ?? "").trim();
  if (!key) return 0;
  const map = loadUsage();
  map[key] = (map[key] ?? 0) + 1;
  saveUsage(map);
  return map[key];
}

/** Prompts used repeatedly but not yet saved as a skill body. */
export function skillSuggestions(minCount = 3) {
  const saved = new Set(listSkills().map((s) => s.body.trim()));
  return Object.entries(loadUsage())
    .filter(([prompt, count]) => count >= minCount && !saved.has(prompt))
    .map(([prompt, use_count]) => ({ prompt, use_count }))
    .sort((a, b) => b.use_count - a.use_count);
}

// ---- Public collections (skills_sources) --------------------------------

export const SOURCES = [
  {
    id: "anthropic",
    label: "Anthropic examples",
    description: "Official example skills from Anthropic.",
    url: "https://github.com/anthropics/skills",
    license_note: "Mostly MIT; check each skill.",
  },
  {
    id: "prompts-chat",
    label: "Awesome ChatGPT prompts",
    description: "A large community collection of role prompts.",
    url: "https://github.com/f/awesome-chatgpt-prompts",
    license_note: "CC0-1.0.",
  },
  {
    id: "copilot",
    label: "GitHub Copilot instructions",
    description: "Community instruction files for coding agents.",
    url: "https://github.com/github/awesome-copilot",
    license_note: "MIT.",
  },
];

// ---- Remote discovery (GitHub skill folders + the prompts CSV) ----------

const API_BASE = "https://api.github.com";
const RAW_BASE = "https://raw.githubusercontent.com";
const MAX_ITEMS = 100;
const MAX_CSV_BYTES = 2 * 1024 * 1024;

const USER_AGENT = "LocalAide-skills";

async function getJson(url) {
  const response = await fetch(url, {
    headers: { Accept: "application/vnd.github+json", "User-Agent": USER_AGENT },
    signal: AbortSignal.timeout(20_000),
  });
  if (!response.ok) {
    throw AppError.internal(`GitHub answered ${response.status} for that collection.`);
  }
  return response.json();
}

async function getRaw(url, cap) {
  const response = await fetch(url, {
    headers: { Accept: "text/plain", "User-Agent": USER_AGENT },
    signal: AbortSignal.timeout(20_000),
  });
  if (!response.ok) {
    throw AppError.internal(`GitHub answered ${response.status} for that skill.`);
  }
  const buffer = await response.arrayBuffer();
  if (cap && buffer.byteLength > cap) {
    throw AppError.invalidInput("That skill is too large to import.");
  }
  return new TextDecoder().decode(buffer);
}

/** "owner/name" or "owner/name/dir" → its parts. */
function parseRepoRef(ref) {
  const parts = String(ref ?? "").split("/").filter(Boolean);
  if (parts.length < 2 || !/^[\w.-]+$/.test(parts[0]) || !/^[\w.-]+$/.test(parts[1])) {
    throw AppError.invalidInput("Use a repository as owner/name, optionally with a skills folder.");
  }
  return { owner: parts[0], repo: parts[1], dir: parts.slice(2).join("/") };
}

/** The listing to fetch for a source id (or an explicit owner/name[/dir]). */
function resolveTarget(source, repo) {
  if (source === "anthropic") return { owner: "anthropics", repo: "skills", dir: "skills" };
  if (source === "copilot") return { owner: "github", repo: "awesome-copilot", dir: "skills" };
  if (source === "prompts-chat") return { csv: true };
  if (source === "github") {
    if (!repo) throw AppError.invalidInput("Enter the repository as owner/name.");
    return parseRepoRef(repo);
  }
  throw AppError.invalidInput(`'${source}' is not a collection this build knows.`);
}

const skillUrl = (target, id) =>
  `https://github.com/${target.owner}/${target.repo}/tree/${target.dir ? `${target.dir}/` : ""}${id}`;

/** A SKILL.md file: frontmatter name/description/license, then the body. */
function parseSkillMarkdown(id, raw, target) {
  const [front, body] = splitFrontmatter(raw);
  return {
    id,
    name: front.get("name") || id,
    description: front.get("description") ?? "",
    body: body.trim(),
    license: front.get("license") || null,
    url: skillUrl(target, id),
  };
}

/** List one collection: skill folders (each holding a SKILL.md) or CSV rows. */
export async function discoverSkills(source, repo) {
  const target = resolveTarget(source, repo);
  if (target.csv) {
    const csv = await getRaw(`${RAW_BASE}/f/prompts.chat/HEAD/prompts.csv`, MAX_CSV_BYTES);
    return parsePromptsCsv(csv).map((detail) => ({
      id: detail.id,
      name: detail.name,
      description: detail.description || null,
      preview: detail.body.slice(0, 200),
      url: detail.url,
    }));
  }
  const dirPath = target.dir ? `/${target.dir}` : "";
  const entries = await getJson(`${API_BASE}/repos/${target.owner}/${target.repo}/contents${dirPath}`);
  if (!Array.isArray(entries)) {
    throw AppError.invalidInput("No skill folders were found there. Skills live in folders that each hold a SKILL.md file.");
  }
  const out = [];
  for (const entry of entries) {
    if (entry.type !== "dir") continue;
    const detail = await fetchSkillDetail(target, entry.name).catch(() => null);
    out.push({
      id: entry.name,
      name: detail?.name || entry.name,
      description: detail?.description || null,
      preview: detail?.body.slice(0, 200) ?? null,
      url: skillUrl(target, entry.name),
    });
    if (out.length >= MAX_ITEMS) break;
  }
  if (out.length === 0) {
    throw AppError.invalidInput("No skill folders were found there. Skills live in folders that each hold a SKILL.md file.");
  }
  return out;
}

async function fetchSkillDetail(target, id) {
  const dirPath = target.dir ? `/${target.dir}` : "";
  const raw = await getRaw(`${RAW_BASE}/${target.owner}/${target.repo}/HEAD${dirPath}/${id}/SKILL.md`, 512 * 1024);
  return parseSkillMarkdown(id, raw, target);
}

/** The full text of one listed skill, for the preview. */
export async function remoteSkillDetail(source, id, repo) {
  const target = resolveTarget(source, repo);
  if (target.csv) {
    const csv = await getRaw(`${RAW_BASE}/f/prompts.chat/HEAD/prompts.csv`, MAX_CSV_BYTES);
    const detail = parsePromptsCsv(csv).find((d) => d.id.toLowerCase() === String(id).toLowerCase());
    if (!detail) throw AppError.notFound("That prompt is not in the collection any more.");
    return detail;
  }
  return fetchSkillDetail(target, String(id));
}

/** Descriptions for listed skills, read a page at a time. */
export async function describeSkills(source, ids, repo) {
  const target = resolveTarget(source, repo);
  const wanted = (Array.isArray(ids) ? ids : []).slice(0, 8);
  const out = [];
  for (const id of wanted) {
    try {
      const detail = await fetchSkillDetail(target, String(id));
      out.push({ id, name: detail.name, description: detail.description, license: detail.license });
    } catch {
      out.push({ id, name: id, description: "", license: null });
    }
  }
  return out;
}

/** Save a listed skill on this computer. It remembers where it came from. */
export async function importSkill(source, id, repo) {
  const detail = await remoteSkillDetail(source, id, repo);
  if (!detail.body.trim()) {
    throw AppError.invalidInput("That skill has no text to import.");
  }
  const origin = ["anthropic", "copilot", "prompts-chat"].includes(source) ? source : "github";
  return createSkill(detail.name, detail.description, ["imported", origin], detail.body, detail.url, detail.license);
}

/** A small RFC 4180 reader for the prompts CSV. */
function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = "";
  let inQuotes = false;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (inQuotes) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 1;
        } else {
          inQuotes = false;
        }
      } else {
        field += c;
      }
    } else if (c === '"') {
      inQuotes = true;
    } else if (c === ",") {
      row.push(field);
      field = "";
    } else if (c === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else if (c !== "\r") {
      field += c;
    }
  }
  if (field || row.length) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

function parsePromptsCsv(body) {
  const rows = parseCsv(body);
  const header = rows[0] ?? [];
  const col = (name) => header.findIndex((h) => h.trim().toLowerCase() === name.toLowerCase());
  const act = col("act");
  const prompt = col("prompt");
  if (act < 0 || prompt < 0) return [];
  const kind = col("type");
  const seen = new Set();
  const out = [];
  for (const row of rows.slice(1)) {
    const type = kind >= 0 ? (row[kind] ?? "").trim() : "";
    if (type && type.toLowerCase() !== "text") continue;
    const title = oneLine(row[act] ?? "");
    const text = (row[prompt] ?? "").trim();
    if (!title || !text || seen.has(title.toLowerCase())) continue;
    seen.add(title.toLowerCase());
    out.push({
      id: title,
      name: title,
      description: "",
      body: text,
      license: "CC0-1.0",
      url: "https://prompts.chat",
    });
    if (out.length >= MAX_ITEMS) break;
  }
  return out;
}
