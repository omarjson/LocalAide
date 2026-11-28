/** True when the renderer runs inside the Electron shell. */
export const isElectron =
  typeof window !== "undefined" && typeof window.__LOCALAIDE__ !== "undefined";
