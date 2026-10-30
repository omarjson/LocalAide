import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { checker } from "vite-plugin-checker";
import tailwindcss from "@tailwindcss/vite";

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
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
