// Developer mode and modules, shared by every screen. Loaded once at start
// and after every change in Settings → Developer.
import { api } from "./api/client";
import type { ChatModeInfo, DeveloperSettings, DeveloperUpdate } from "./api/bindings";

export const dev = $state<{ settings: DeveloperSettings | null }>({ settings: null });

/** True when developer mode is on and the flag is on. */
export function flag(id: string): boolean {
  const s = dev.settings;
  return !!s && s.enabled && !!s.flags[id];
}

/** Modules are on unless switched off, with or without developer mode. */
export function moduleOn(id: string): boolean {
  return dev.settings?.modules[id] ?? true;
}

export function chatModes(): ChatModeInfo[] {
  return dev.settings?.modes ?? [];
}

/** Flags whose job is to change the running UI. Each also becomes a
 * `data-<id>` attribute on <html>, so CSS and playwright can see it. */
export const EFFECT_FLAGS = [
  "perf_overlay",
  "trace_ipc_sizes",
  "vector_search_debug",
  "mock_slow_backend",
  "show_keychain_state",
  "log_redaction_preview",
  "agent_debug",
  "tab_debug",
  "session_search",
  "multi_run",
  "agent_files",
  "goal_mode",
  "remote_ui",
  "plugin_panels",
] as const;

export type EffectFlag = (typeof EFFECT_FLAGS)[number];

/** The effect flags currently on, for the Settings summary and status chips. */
export function activeEffectFlags(): EffectFlag[] {
  return EFFECT_FLAGS.filter((id) => flag(id));
}

function apply(settings: DeveloperSettings): void {
  dev.settings = settings;
  const root = document.documentElement;
  root.dataset.motion = flag("no_motion") ? "off" : "on";
  // Each debug flag is reflected on <html>. CSS uses [data-tab_debug="on"]
  // and friends; removing the attribute when off keeps the DOM honest.
  for (const id of EFFECT_FLAGS) {
    if (id !== "perf_overlay") {
      if (flag(id)) root.dataset[id] = "on";
      else delete root.dataset[id];
    }
  }
  // The developer flag forces compact; otherwise the Appearance → Density
  // setting decides, so re-apply what it saved.
  if (flag("compact")) {
    root.dataset.density = "compact";
  } else {
    try {
      const saved = localStorage.getItem("localaide-density");
      if (saved === "compact") root.dataset.density = "compact";
      else delete root.dataset.density;
    } catch {
      delete root.dataset.density;
    }
  }
}

export async function loadDev(): Promise<void> {
  try {
    apply(await api.developerGet());
  } catch (e: unknown) {
    console.error("Developer settings could not load", e);
  }
}

export async function updateDev(update: DeveloperUpdate): Promise<void> {
  apply(await api.developerSet(update));
}

export async function resetDev(): Promise<void> {
  apply(await api.developerReset());
}
