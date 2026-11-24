// Entry point for SSR rendering, loaded through Vite's module runner (see
// ssr-harness.ts). `render` must come from the same module graph as the
// components it renders: the runner evaluates its own copy of
// svelte/internal/server, and `render` imported directly in a test file
// would set the SSR context in Node's copy instead — the component then
// crashes reading a null context.
export { render } from "svelte/server";
