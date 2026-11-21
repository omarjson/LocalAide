/** Which desktop this is, from the browser's user agent. */
export type Platform = "mac" | "windows" | "linux";

export function platform(): Platform {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (/Mac|iPhone|iPad/.test(ua)) return "mac";
  if (/Windows/.test(ua)) return "windows";
  return "linux";
}

/** The name people know the file manager by. */
export function fileManagerName(): string {
  const p = platform();
  return p === "mac" ? "Finder" : p === "windows" ? "File Explorer" : "Files";
}
