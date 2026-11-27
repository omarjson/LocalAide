// Developer mode: experimental flags, module switches, and the chat modes
// the picker offers. Ported from commands/developer.rs.

import { AppError } from "../core/errors.mjs";
import { loadConfig, updateConfig } from "../core/config.mjs";
import { MODES } from "./modes.mjs";

const FLAG_LIST = [
  { id: "request_log", label: "Request log panel", description: "Show backend timings in Developer.", default: false },
  { id: "no_motion", label: "Turn off animations", description: "Skip transitions everywhere.", default: false },
  { id: "compact", label: "Compact layout", description: "Tighter spacing.", default: false },
  { id: "answer_stats", label: "Answer statistics", description: "Tokens, time and speed under each answer.", default: false },
  { id: "plain_text", label: "Plain-text view", description: "Show the raw answer text.", default: false },
  { id: "dev_prompt", label: "Edit the system prompt", description: "Change the instructions the model starts with.", default: false },
];

const MODULE_LIST = [
  { id: "memory", label: "Notes and file recall", description: "Chat looks in your notes and documents.", default: true },
  { id: "search", label: "Search screen", description: "The Library screen in the sidebar.", default: true },
  { id: "web", label: "Web in chat", description: "The model may search the web.", default: true },
  { id: "skills", label: "Skills", description: "Saved prompts you can reuse.", default: true },
  { id: "online", label: "Online providers", description: "Cloud models with your key.", default: true },
  { id: "opencode", label: "OpenCode link", description: "Hand work to OpenCode.", default: true },
  { id: "command_palette", label: "Command palette", description: "Ctrl+K quick actions.", default: true },
];

export function developerGet() {
  const dev = loadConfig().developer ?? {};
  return {
    enabled: dev.enabled === true,
    flags: dev.flags ?? {},
    modules: dev.modules ?? {},
    flag_list: FLAG_LIST,
    module_list: MODULE_LIST,
    modes: Object.values(MODES).map((m) => ({
      id: m.id,
      label: m.label,
      description: m.description,
      instruction: m.instruction,
    })),
  };
}

const boolMap = (value, allowedIds, what) => {
  if (value === undefined || value === null) return null;
  if (typeof value !== "object" || Array.isArray(value)) {
    throw AppError.invalidInput(`That is not a list of ${what} switches.`);
  }
  const out = {};
  for (const [id, on] of Object.entries(value)) {
    if (!allowedIds.includes(id)) continue;
    out[id] = on === true;
  }
  return out;
};

export function developerSet(update) {
  const u = update ?? {};
  updateConfig((cfg) => {
    if (u.enabled !== undefined) cfg.developer.enabled = u.enabled === true;
    const flags = boolMap(u.flags, FLAG_LIST.map((f) => f.id), "flag");
    if (flags) cfg.developer.flags = { ...(cfg.developer.flags ?? {}), ...flags };
    const modules = boolMap(u.modules, MODULE_LIST.map((m) => m.id), "module");
    if (modules) cfg.developer.modules = { ...(cfg.developer.modules ?? {}), ...modules };
  });
  return developerGet();
}

export function developerReset() {
  updateConfig((cfg) => {
    cfg.developer = { enabled: false, flags: {}, modules: {} };
  });
  return developerGet();
}
