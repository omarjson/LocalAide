/// <reference types="vite/client" />

import type { LocalAideBridge, LocalAideInternals } from "@goat/client-runtime/localaide";

declare global {
  interface Window {
    /** The bridge Electron's preload exposes. */
    __LOCALAIDE__?: LocalAideBridge;
    /** The Tauri-compatible surface the preload also exposes. */
    __TAURI_INTERNALS__?: LocalAideInternals;
  }
}

export {};
