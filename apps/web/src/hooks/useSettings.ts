import { useEffect, useSyncExternalStore } from "react";

/**
 * The renderer's own settings: the choices that belong to this window rather
 * than to the backend's config file.
 *
 * Vendored from GOAT (`apps/web/src/hooks/useSettings.ts`, MIT). GOAT read
 * these from a server that pushed a settings stream; LocalAide keeps the
 * handful the shell needs in the renderer and asks the backend only for the
 * ones the backend owns (theme, interface scale, approval timers).
 */

export type ClientSettings = {
  /** How long the shell's panel animations run. Zero turns them off. */
  panelAnimationDurationMs: number;
  /** The theme the user picked: "light", "dark" or "system". */
  theme: "light" | "dark" | "system";
  /** The interface scale, as a factor of the normal size. */
  uiScale: number;
};

const DEFAULT_SETTINGS: ClientSettings = {
  panelAnimationDurationMs: 200,
  theme: "system",
  uiScale: 1,
};

let current: ClientSettings = DEFAULT_SETTINGS;
const listeners = new Set<() => void>();

export function getClientSettings(): ClientSettings {
  return current;
}

export function subscribeClientSettings(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Merges a change into the client settings and tells the shell about it. */
export function updateClientSettings(update: Partial<ClientSettings>): void {
  current = { ...current, ...update };
  for (const listener of listeners) listener();
}

/** Reads one value out of the client settings. */
export function useClientSettings<T>(selector: (settings: ClientSettings) => T): T {
  const settings = useSyncExternalStore(
    subscribeClientSettings,
    getClientSettings,
    getClientSettings,
  );
  return selector(settings);
}

/** Whether the window is in dark mode right now. */
export function useIsDark(): boolean {
  const theme = useClientSettings((settings) => settings.theme);
  if (theme !== "system") return theme === "dark";
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

/** Puts the theme class on the document, the way GOAT's shell expects it. */
export function useThemeClass(): void {
  const isDark = useIsDark();
  useEffect(() => {
    document.documentElement.classList.toggle("dark", isDark);
    document.documentElement.dataset.theme = isDark ? "dark" : "light";
  }, [isDark]);
}
