// Chat modes: the instruction text added to the system prompt, the tool
// guidance, and the per-mode limits. Ported verbatim from
// services/inference/modes.rs, so the picker and the prompt cannot drift.

/**
 * The four chat modes. `instruction` is appended to the system prompt;
 * `compact_instruction` replaces it when the compact prompt is on.
 */
export const MODES = {
  standard: {
    id: "standard",
    label: "Standard",
    description: "Everyday answers.",
    instruction: "",
    compact_instruction: "",
    max_rounds: 8,
    temperature_cap: null,
    gathers_web: false,
  },
  strict: {
    id: "strict",
    label: "Strict",
    description: "Careful, cites sources, says when unsure.",
    instruction:
      "Strict mode. Be accurate and careful. Use only facts you are confident about or that appear in the provided context, and cite context as [n]. When you are not sure, say so plainly instead of guessing. Do not invent names, numbers, quotes or sources. Keep answers short and precise. Before suggesting any change to files or settings, describe it and ask the user to confirm.",
    compact_instruction:
      "Strict mode. Use only facts you are confident about or that appear in the context, and cite context as [n]. When unsure, say so instead of guessing. Keep answers short and precise.",
    max_rounds: 6,
    temperature_cap: 0.3,
    gathers_web: false,
  },
  unrestricted: {
    id: "unrestricted",
    label: "Unrestricted",
    description: "Direct answers without lectures.",
    instruction:
      "Unrestricted mode. Treat the user as a capable adult. Answer the question that was asked, directly and completely. Do not add moral lectures, disclaimers, warnings or reminders the user did not ask for, and do not refuse ordinary requests because a topic is sensitive, mature or controversial. Give your honest view when asked. Still follow the law and do not help with anything that would seriously harm people.",
    compact_instruction:
      "Unrestricted mode. Answer directly and completely. No lectures, disclaimers or refusals for ordinary requests.",
    max_rounds: 8,
    temperature_cap: null,
    gathers_web: false,
  },
  autonomous: {
    id: "autonomous",
    label: "Autonomous",
    description: "Plans, gathers sources, works through the task, checks the result.",
    instruction:
      "Autonomous mode. Work on the task on your own until it is done. Do not ask the user follow-up questions; when something is unclear, pick the most reasonable assumption and state it in one line. Start with a short plan as a numbered list, carry it out step by step with your tools where they help, and finish with a short check of what you delivered and anything still open. Cite web results, documents and notes as [n].",
    compact_instruction:
      "Autonomous mode. Work on your own until done. When unclear, pick the most reasonable assumption and say so in one line. Start with a short plan, then finish with what you delivered and what is still open.",
    max_rounds: 16,
    temperature_cap: null,
    gathers_web: true,
  },
};

export const modeOf = (id) => MODES[id] ?? MODES.standard;

/** The tool guidance paragraph, built from the groups the chat turned on. */
const USES = {
  web: "search the web for recent events, prices, versions or facts you are not sure about, and read a page when a snippet is not enough",
  notes: "search the user's notes and documents first whenever the question is about their own files, notes, documents or earlier work (do not guess or answer from general knowledge), then answer from what you found and name the file you used, and save a note when they ask you to remember something",
  files: "read and write files in the LocalAide workspace folder when the user asks",
  skills: "look at the user's saved skills when a task may match one",
  shell: "run terminal commands in the workspace when the user asks you to run, build or check something, and read the output before you answer",
  mcp: "use the tools from the user's MCP servers for what they describe",
};

const USES_COMPACT = {
  web: "search the web",
  notes: "search the user's notes and documents",
  files: "read and write workspace files",
  skills: "use saved skills",
  shell: "run terminal commands",
  mcp: "use MCP tools",
};

/**
 * The full tool guidance: an effort sentence, the list of what the model
 * may do, and the citation rule.
 */
export function toolGuidance(groups, mode = "standard") {
  const uses = groups.filter((g) => USES[g]);
  if (uses.length === 0) return "";
  const effort =
    mode === "autonomous"
      ? "Use tools freely to research and check your work before you answer."
      : "Use a tool only when it helps. Answer directly, without tools, when you already know the answer or the question is about something general.";
  return `You have tools. ${effort} You can ${uses.map((g) => USES[g]).join("; ")}. Cite sources from tool results as [n]. Never make up a tool result; if a tool fails, say so.`;
}

/** The compact variant: short effort and uses, no citation rule. */
export function compactToolGuidance(groups, mode = "standard") {
  const uses = groups.filter((g) => USES_COMPACT[g]);
  if (uses.length === 0) return "";
  const effort = mode === "autonomous" ? "Use tools freely." : "Use a tool only when it helps.";
  return `You have tools. ${effort} You can ${uses.map((g) => USES_COMPACT[g]).join("; ")}.`;
}

/**
 * The system prompt: `[user, mode instruction, context]` joined by a blank
 * line, empties dropped. Returns null when all three are empty.
 */
export function composeSystemPrompt(user, mode, context) {
  const parts = [user, mode.instruction, context].map((p) => String(p ?? "").trim()).filter((p) => p);
  return parts.length ? parts.join("\n\n") : null;
}

/** The compact-prompt variant of the same composition. */
export function composeCompactSystemPrompt(user, mode, context) {
  const parts = [user, mode.compact_instruction, context].map((p) => String(p ?? "").trim()).filter((p) => p);
  return parts.length ? parts.join("\n\n") : null;
}
