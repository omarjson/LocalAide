// The chat box's "/" menu: built-in commands, then matching skills.
import type { Skill } from "./api/bindings";

/** Icons the commands use (all exist in Icon.svelte). */
export type SlashIcon = "plus" | "layers" | "models" | "sliders" | "download" | "copy" | "chat" | "target" | "memory" | "skills" | "globe" | "wrench";

export interface SlashCommand {
  id: string;
  label: string;
  description: string;
  icon: SlashIcon;
}

export const SLASH_COMMANDS: SlashCommand[] = [
  { id: "clear", label: "Start a new chat", description: "Clear this conversation", icon: "plus" },
  { id: "compact", label: "Compact conversation", description: "Stop sending older messages; they stay on screen", icon: "layers" },
  { id: "model", label: "Choose a model", description: "Open the model selector", icon: "models" },
  { id: "settings", label: "Chat settings", description: "Open response style and context settings", icon: "sliders" },
  { id: "export", label: "Export Markdown", description: "Save this conversation as a Markdown file", icon: "download" },
  { id: "copy", label: "Copy conversation", description: "Copy the full conversation to the clipboard", icon: "copy" },
  { id: "by-the-way", label: "By the way", description: "Add a side question without losing the main task", icon: "chat" },
  { id: "goal", label: "Set a goal", description: "Turn a vague task into a clear goal", icon: "target" },
  { id: "scale", label: "Scale the answer", description: "Ask for a shorter or more detailed answer", icon: "layers" },
  { id: "remember", label: "Remember this", description: "Save a memory note after confirmation", icon: "memory" },
  { id: "create-skill", label: "Create a skill", description: "Turn a reusable instruction into a saved skill", icon: "skills" },
  { id: "web", label: "Use web", description: "Let the model search the web in this chat", icon: "globe" },
  { id: "notes", label: "Use notes", description: "Allow notes and document recall", icon: "memory" },
  { id: "tools", label: "Use tools", description: "Open the tools menu", icon: "wrench" },
];

export type SlashItem = { kind: "command"; value: SlashCommand } | { kind: "skill"; value: Skill };

/** The words after "/", lower-cased, or null when the box is not a slash command. */
export function slashQuery(input: string): string | null {
  return input.startsWith("/") ? input.slice(1).toLowerCase() : null;
}

/** Most skills the menu lists at once. */
export const MAX_SKILL_ITEMS = 6;

/**
 * Commands first, then skills, that match what follows the "/". This is
 * exactly the list on screen, so the keyboard cursor indexes it directly.
 */
export function slashItems(input: string, skills: Skill[], commands: SlashCommand[] = SLASH_COMMANDS): SlashItem[] {
  const q = slashQuery(input);
  if (q === null) return [];
  const cmds = commands.filter((c) => c.id.includes(q) || c.label.toLowerCase().includes(q));
  const found = skills
    .filter((s) => s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q) || s.tags.some((t) => t.toLowerCase().includes(q)))
    .slice(0, MAX_SKILL_ITEMS);
  return [...cmds.map((value) => ({ kind: "command" as const, value })), ...found.map((value) => ({ kind: "skill" as const, value }))];
}
