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

/**
 * Shareable skins. A skin is a small JSON file of CSS custom-property
 * overrides. There is no skin IPC in this build, so skins are kept in
 * localStorage and shared through Import / Export of that JSON file. See
 * docs/evidence/skins-and-plugins.md for the limitation this creates.
 */
export interface Skin {
  name: string;
  overrides: Record<string, string>;
}

const SKINS_KEY = "localaide-skins";
const ACTIVE_SKIN_KEY = "localaide-active-skin";

/** Only these design tokens can be overridden, and values are checked first. */
export const SKIN_PROPERTIES: readonly string[] = [
  "--font-sans",
  "--text-xs",
  "--text-xs--line-height",
  "--radius-md", "--radius-lg", "--radius-xl", "--radius-2xl", "--radius-3xl",
  "--color-ink-50", "--color-ink-100", "--color-ink-200", "--color-ink-300",
  "--color-ink-400", "--color-ink-500", "--color-ink-600", "--color-ink-700",
  "--color-ink-800", "--color-ink-900", "--color-ink-950",
  "--color-ember-50", "--color-ember-100", "--color-ember-200", "--color-ember-300",
  "--color-ember-400", "--color-ember-500", "--color-ember-600", "--color-ember-700",
  "--color-ember-800", "--color-ember-900", "--color-ember-950",
  "--color-on-ember", "--color-edge", "--color-veil",
  "--color-emerald-300", "--color-emerald-400", "--color-rose-300", "--color-rose-400",
  "--color-amber-200", "--color-amber-300", "--color-amber-400", "--color-amber-500", "--color-amber-600",
  "--shadow-elev", "--shadow-glow",
  "--dur-fast", "--dur-base", "--dur-slow",
] as const;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** Drops unknown properties and CSS that could escape a declaration. */
export function cleanSkinOverrides(raw: unknown): Record<string, string> {
  const out: Record<string, string> = {};
  if (!isRecord(raw)) return out;
  for (const [key, value] of Object.entries(raw)) {
    if (!SKIN_PROPERTIES.includes(key) || typeof value !== "string") continue;
    const trimmed = value.trim();
    if (trimmed.length > 0 && trimmed.length <= 400 && !/[{};]/.test(trimmed)) out[key] = trimmed;
  }
  return out;
}

/** Parse an imported skin file. Null means it was not a usable skin. */
export function parseSkin(text: string): Skin | null {
  try {
    const raw: unknown = JSON.parse(text);
    if (!isRecord(raw) || typeof raw.name !== "string") return null;
    const name = raw.name.trim().slice(0, 80);
    return name ? { name, overrides: cleanSkinOverrides(raw.overrides) } : null;
  } catch {
    return null;
  }
}

/** Saved skins, newest first. Bad storage never throws into the UI. */
export function listSkins(): Skin[] {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(SKINS_KEY) ?? "[]");
    if (!Array.isArray(raw)) return [];
    return raw.filter(isRecord)
      .map((s) => ({
        name: typeof s.name === "string" ? s.name.trim().slice(0, 80) : "",
        overrides: cleanSkinOverrides(s.overrides),
      }))
      .filter((s) => s.name.length > 0)
      .reverse();
  } catch {
    return [];
  }
}

export function saveSkin(skin: Skin): void {
  const name = skin.name.trim().slice(0, 80);
  if (!name) return;
  const clean: Skin = { name, overrides: cleanSkinOverrides(skin.overrides) };
  const skins = listSkins().filter((s) => s.name !== name);
  skins.push(clean);
  try {
    localStorage.setItem(SKINS_KEY, JSON.stringify(skins));
  } catch {
    // Storage can be unavailable; the skin still applies this session.
  }
}

export function deleteSkin(name: string): void {
  try {
    localStorage.setItem(SKINS_KEY, JSON.stringify(listSkins().filter((s) => s.name !== name)));
  } catch {
    // Storage can be unavailable.
  }
}

/** Set the overrides on <html>, clearing any token the skin does not define. */
export function applySkin(overrides: Record<string, string>): void {
  const clean = cleanSkinOverrides(overrides);
  for (const prop of SKIN_PROPERTIES) {
    const value = clean[prop];
    if (value) document.documentElement.style.setProperty(prop, value);
    else document.documentElement.style.removeProperty(prop);
  }
  try {
    localStorage.setItem(ACTIVE_SKIN_KEY, JSON.stringify(clean));
  } catch {
    // Storage can be unavailable; the skin still applies this session.
  }
}

/** The overrides to restore at startup, if any. */
export function activeSkinOverrides(): Record<string, string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(ACTIVE_SKIN_KEY) ?? "{}");
    return cleanSkinOverrides(raw);
  } catch {
    return {};
  }
}
