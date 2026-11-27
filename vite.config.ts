import { defineConfig } from "vite";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { checker } from "vite-plugin-checker";
import tailwindcss from "@tailwindcss/vite";

const dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    svelte({
      compilerOptions: {
        runes: true,
      },
    }),
    tailwindcss(),
    checker({
      typescript: true,
    }),
  ],
  // The Electron window loads dist/index.html from file://, so every asset
  // path must be relative.
  base: "./",
  resolve: {
    alias: {
      // The renderer keeps its Tauri specifiers; they resolve to the shim.
      "@tauri-apps/api/core": path.join(dirname, "src/lib/platform/tauri.ts"),
      "@tauri-apps/api/event": path.join(dirname, "src/lib/platform/tauri.ts"),
      "@tauri-apps/api/window": path.join(dirname, "src/lib/platform/tauri.ts"),
      "@tauri-apps/api/webview": path.join(dirname, "src/lib/platform/tauri.ts"),
      "@tauri-apps/api/app": path.join(dirname, "src/lib/platform/tauri.ts"),
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
