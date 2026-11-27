// Preview entry (development only). Installs the fake IPC backend, then boots
// the real app. Open via /preview.html on the Vite dev server.
import { installShim } from "./shim";

installShim();

// Theme init normally runs from /theme-init.js in index.html before mount.
// Preview has no such script, so honor a saved choice and fall back to dark.
try {
  const saved = window.localStorage.getItem("localaide-theme");
  document.documentElement.dataset.theme = saved === "light" ? "light" : "dark";
} catch {
  document.documentElement.dataset.theme = "dark";
}

import "../main";
