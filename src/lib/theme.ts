export type Theme = "dark" | "light";

/** Read by public/theme-init.js before the first paint. */
const STORAGE_KEY = "localaide-theme";

/** Shows a theme now and remembers it for the first paint of the next launch. */
export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Storage can be unavailable. The theme still applies for this session.
  }
}

/** The theme on screen now. */
export function currentTheme(): Theme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}
