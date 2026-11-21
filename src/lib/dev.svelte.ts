// Developer mode and modules, shared by every screen. Loaded once at start
// and after every change in Settings → Developer.
import { api } from "./api/client";
import type { ChatModeInfo, DeveloperSettings, DeveloperUpdate } from "./api/bindings";
import { requestLog } from "./requestLog.svelte";

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

function apply(settings: DeveloperSettings): void {
  dev.settings = settings;
  requestLog.enabled = flag("request_log");
  const root = document.documentElement;
  root.dataset.motion = flag("no_motion") ? "off" : "on";
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
