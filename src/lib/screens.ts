// The single ordered list of app screens. The sidebar, Cmd/Ctrl+1..7, the
// keyboard-help dialog, the command palette and the unit tests all read this
// file, so the number shown next to a screen is always the screen that key
// opens. Skills and Setup are separate: Skills is a switchable module with no
// number, and Setup only exists during first run.
export type ScreenId = "chat" | "search" | "memory" | "models" | "automations" | "usage" | "settings";

export type ScreenIcon =
  | "chat"
  | "search"
  | "memory"
  | "models"
  | "automations"
  | "usage"
  | "settings"
  | "sun"
  | "cloud"
  | "wrench"
  | "plug"
  | "lock"
  | "info"
  | "flask";

export interface ScreenDef {
  id: ScreenId;
  label: string;
  icon: ScreenIcon;
  /** 1-based number used by Cmd/Ctrl+N. */
  key: number;
}

export const SCREEN_REGISTRY: readonly ScreenDef[] = [
  { id: "chat", label: "Chat", icon: "chat", key: 1 },
  { id: "search", label: "Search", icon: "search", key: 2 },
  { id: "memory", label: "Memory", icon: "memory", key: 3 },
  { id: "models", label: "Models", icon: "models", key: 4 },
  { id: "automations", label: "Automations", icon: "automations", key: 5 },
  { id: "usage", label: "Usage", icon: "usage", key: 6 },
  { id: "settings", label: "Settings", icon: "settings", key: 7 },
] as const;

export const SCREEN_IDS: readonly ScreenId[] = SCREEN_REGISTRY.map((s) => s.id);

/** The screen bound to a number key, or undefined outside 1..7. */
export const screenForKey = (key: number): ScreenDef | undefined => SCREEN_REGISTRY.find((s) => s.key === key);

/** The number advertised for a screen, if it has one. */
export const keyForScreen = (id: ScreenId): number | undefined => SCREEN_REGISTRY.find((s) => s.id === id)?.key;

// Settings uses the same idea: seven top-level groups in a fixed order. The
// smaller sections (voice, web search, backups, activity) live inside a group
// so every existing deep link still opens its own section.
export type SettingsGroupId = "general" | "online" | "tools" | "integrations" | "privacy" | "about" | "developer";

export interface SettingsGroupDef {
  id: SettingsGroupId;
  label: string;
  icon: ScreenIcon;
}

export const SETTINGS_GROUPS: readonly SettingsGroupDef[] = [
  { id: "general", label: "General", icon: "sun" },
  { id: "online", label: "Online providers", icon: "cloud" },
  { id: "tools", label: "Tools & MCP", icon: "wrench" },
  { id: "integrations", label: "Integrations", icon: "plug" },
  { id: "privacy", label: "Privacy & files", icon: "lock" },
  { id: "about", label: "About & diagnostics", icon: "info" },
  { id: "developer", label: "Developer", icon: "flask" },
] as const;

/** Which top-level group a Settings section belongs to. */
export const SETTINGS_GROUP_OF: Record<string, SettingsGroupId> = {
  general: "general",
  voice: "general",
  share: "general",
  online: "online",
  web: "online",
  tools: "tools",
  integrations: "integrations",
  privacy: "privacy",
  backups: "privacy",
  about: "about",
  activity: "about",
  developer: "developer",
};
