// Preview entry (development only). Installs the fake IPC backend, then boots
// the real app. Open via /preview.html on the Vite dev server.
import { installShim } from "./shim";

installShim();

// Theme init normally runs from /theme-init.js in index.html before mount.
document.documentElement.dataset.theme = "dark";

import "../main";
