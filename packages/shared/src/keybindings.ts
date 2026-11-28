/**
 * Platform helpers shared by the renderer.
 *
 * Vendored from GOAT (`packages/shared/src/keybindings.ts`, MIT). GOAT's
 * keybinding resolver needs the full `@goat/contracts` keybinding schemas and
 * a server that pushes keybindings; LocalAide keeps its own keyboard handling
 * in the renderer, so only the platform helpers are kept.
 */

export function isMacPlatform(platform: string): boolean {
  return /mac|iphone|ipad|ipod/i.test(platform);
}

export function isWindowsPlatform(platform: string): boolean {
  return /^win(dows)?/i.test(platform);
}

/** The platform the renderer runs on. */
export function currentPlatform(): string {
  if (typeof navigator === "undefined") return "unknown";
  return /Mac/i.test(navigator.platform) || /Mac/i.test(navigator.userAgent)
    ? "mac"
    : /Win/i.test(navigator.platform) || /Win/i.test(navigator.userAgent)
      ? "windows"
      : "linux";
}
