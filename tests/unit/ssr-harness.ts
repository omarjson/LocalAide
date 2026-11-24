// Minimal component-render harness for node:test.
//
// Loads a .svelte component through the repo's real Vite + Svelte plugin
// (so .svelte, .ts and package imports all resolve exactly like the app
// build) and renders it to HTML with svelte/server. No browser, no jsdom,
// no test-runner plugin: the only test runner is node:test.
//
// Usage:
//   import { closeSsrServer, MAC_USER_AGENT, renderComponent } from "./ssr-harness.ts";
//
//   const html = await renderComponent("/src/components/TitleBar.svelte",
//     { tabs, activeKey: "a", visible: true }, MAC_USER_AGENT);
//
//   after(() => closeSsrServer());
//
// Notes:
// - `platform()` reads navigator.userAgent, so pass a user agent to render
//   the macOS / Windows variant. Without one, Node's own UA applies and
//   the component renders its Linux variant. Node's global `navigator` is
//   a getter-only property, so the harness swaps it with defineProperty
//   and restores it after each render.
// - `onMount` and event handlers never run in SSR: what this harness can
//   assert is the rendered HTML (structure, attributes, classes, text).
//   Behaviour that needs interaction (clicking the window menu open,
//   double-click to zoom) stays out of reach here.
// - The Vite dev server is created once per test file and must be closed
//   in an `after()` hook, or the file watcher keeps the test process alive.

import { createServer, type ViteDevServer } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

export const MAC_USER_AGENT =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
export const WINDOWS_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

let server: ViteDevServer | null = null;
let renderFn: ((component: unknown, options: { props?: Record<string, unknown> }) => { html: string }) | null = null;

async function getServer(): Promise<ViteDevServer> {
  if (server) return server;
  server = await createServer({
    root: repoRoot,
    // The repo's vite.config.ts enables vite-plugin-checker, which spawns
    // a background type checker — far too heavy for a unit test. The
    // svelte plugin alone is all SSR loading needs.
    configFile: false,
    plugins: [svelte({ compilerOptions: { runes: true } })],
    // svelte must stay external in SSR: if Vite pre-bundles it into
    // .vite/deps_ssr, the component and svelte/server end up with two
    // copies of svelte/internal/server and rendering crashes (the bundled
    // copy's renderer context is null). Top-level optimizeDeps only
    // applies to the client environment in Vite 8, so the exclude has to
    // go under environments.ssr.
    environments: { ssr: { optimizeDeps: { exclude: ["svelte"] } } },
    server: { port: 0 },
    logLevel: "silent",
    appType: "custom",
  });
  // Listening runs the plugins' buildStart hooks (vite:css populates
  // its module cache there). Without it, SSR-loading a component with
  // a <style> block fails in vite:css-post. Port 0 picks a free port;
  // nothing ever connects to it.
  await server.listen();
  return server;
}

/** `render` from the runner's module graph (see ssr-render-entry.ts). */
async function getRender(): Promise<
  (component: unknown, options: { props?: Record<string, unknown> }) => { html: string }
> {
  if (renderFn) return renderFn;
  const srv = await getServer();
  const entry = await srv.ssrLoadModule("/tests/unit/ssr-render-entry.ts");
  renderFn = entry.render as typeof renderFn;
  return renderFn;
}

/**
 * Render a component to its SSR HTML. `path` is repo-root-relative
 * ("/src/components/TitleBar.svelte"). `props` are the component props.
 * `userAgent` picks the platform variant when the component reads it.
 */
export async function renderComponent(
  path: string,
  props: Record<string, unknown> = {},
  userAgent?: string,
): Promise<string> {
  const previousNavigator = globalThis.navigator;
  try {
    if (userAgent !== undefined) {
      // Plain assignment is a no-op: Node's navigator is getter-only.
      Object.defineProperty(globalThis, "navigator", {
        value: { userAgent },
        configurable: true,
      });
    }
    const srv = await getServer();
    const mod = await srv.ssrLoadModule(path);
    const render = await getRender();
    const { html } = render(mod.default, { props });
    return html;
  } finally {
    if (userAgent !== undefined) {
      Object.defineProperty(globalThis, "navigator", {
        value: previousNavigator,
        configurable: true,
      });
    }
  }
}

/** Stop the dev server started by renderComponent; call in after(). */
export async function closeSsrServer(): Promise<void> {
  if (server) {
    await server.close();
    server = null;
    renderFn = null;
  }
}
