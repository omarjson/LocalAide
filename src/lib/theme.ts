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

/** Accent hue families behind the ember fills. Ember itself is the brand. */
export type Accent = "ember" | "ocean" | "violet" | "forest" | "rose" | "gold";
const ACCENTS: Accent[] = ["ember", "ocean", "violet", "forest", "rose", "gold"];
const ACCENT_KEY = "localaide-accent";

/** Shows an accent now and remembers it for the first paint of the next launch. */
export function applyAccent(accent: Accent): void {
  if (accent === "ember") delete document.documentElement.dataset.accent;
  else document.documentElement.dataset.accent = accent;
  try {
    localStorage.setItem(ACCENT_KEY, accent);
  } catch {
    // Storage unavailable; the accent still applies for this session.
  }
}

/** The accent on screen now. */
export function currentAccent(): Accent {
  const value = document.documentElement.dataset.accent as Accent | undefined;
  return value && ACCENTS.includes(value) ? value : "ember";
}

/** Whole-app font families. */
export type AppFont = "system" | "serif" | "mono";
const FONTS: AppFont[] = ["system", "serif", "mono"];
const FONT_KEY = "localaide-font";

export function applyFont(font: AppFont): void {
  if (font === "system") delete document.documentElement.dataset.font;
  else document.documentElement.dataset.font = font;
  try {
    localStorage.setItem(FONT_KEY, font);
  } catch {
    // Storage unavailable; the font still applies for this session.
  }
}

export function currentFont(): AppFont {
  const value = document.documentElement.dataset.font as AppFont | undefined;
  return value && FONTS.includes(value) ? value : "system";
}

/** Chat spacing: comfortable (default) or compact. */
export type Density = "comfortable" | "compact";
const DENSITY_KEY = "localaide-density";

export function applyDensity(density: Density): void {
  if (density === "comfortable") delete document.documentElement.dataset.density;
  else document.documentElement.dataset.density = density;
  try {
    localStorage.setItem(DENSITY_KEY, density);
  } catch {
    // Storage unavailable; the density still applies for this session.
  }
}

export function currentDensity(): Density {
  return document.documentElement.dataset.density === "compact" ? "compact" : "comfortable";
}

