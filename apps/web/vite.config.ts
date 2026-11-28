import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const dirname = path.dirname(fileURLToPath(import.meta.url));

// GOAT's app used vite-plus, which reads the `~/*` path alias straight out
// of tsconfig. Plain Vite needs the alias declared, so the two agree.
export default defineConfig({
  // The app lives in this folder; the root script points Vite at this config.
  root: dirname,
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "~": path.join(dirname, "src"),
      // GOAT's translation runtime needs a build-time locale bundle; LocalAide
      // ships one language, so the calls pass their English text through.
      "gt-react": path.join(dirname, "src/i18n/gtShim.tsx"),
    },
  },
  // The Electron window loads dist/index.html from file://, so every asset
  // path must be relative.
  base: "./",
  build: {
    outDir: path.join(dirname, "../../dist"),
    emptyOutDir: true,
    target: "chrome130",
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
