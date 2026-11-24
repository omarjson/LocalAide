import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, test } from "node:test";
import { MAC_USER_AGENT, closeSsrServer, renderComponent } from "./ssr-harness.ts";

after(() => closeSsrServer());

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

// Two tabs: one idle, one busy, so the active/inactive and busy/idle
// branches both render.
const tabs = [
  { key: "a", id: "a", title: "First chat", folder: null, busy: false, lastUsed: 1 },
  { key: "b", id: "b", title: "Second chat", folder: null, busy: true, lastUsed: 2 },
];

test("macOS uses the native traffic lights, not custom window buttons", () => {
  const conf = JSON.parse(read("src-tauri/tauri.macos.conf.json"));
  const win = conf.app.windows[0];
  assert.equal(win.label, "main");
  assert.equal(win.decorations, true);
  assert.equal(win.hiddenTitle, true);
  assert.equal(win.titleBarStyle, "Overlay");
  assert.ok(
    win.trafficLightPosition.x >= 0 && win.trafficLightPosition.y >= 0 &&
      win.trafficLightPosition.y < 20,
    `traffic lights must sit inside the 40px chrome row: ${JSON.stringify(win.trafficLightPosition)}`,
  );
  // The same window size as the base config, so the platform overlay
  // replaces rather than resets the window.
  const base = JSON.parse(read("src-tauri/tauri.conf.json")).app.windows[0];
  for (const key of ["width", "height", "minWidth", "minHeight"] as const) {
    assert.equal(win[key], base[key], `${key} must agree with the base config`);
  }
});

test("macOS renders a clean chrome with tabs below it", async () => {
  const html = await renderComponent(
    "/src/components/TitleBar.svelte",
    { tabs, activeKey: "a", visible: true },
    MAC_USER_AGENT,
  );

  // Chat tabs are rendered, with their titles.
  assert.match(html, /role="tablist"/, "chat tabs are rendered");
  assert.match(html, /First chat/);
  assert.match(html, /Second chat/);
  // The row is still draggable.
  assert.match(html, /data-tauri-drag-region/, "the row is still draggable");
  // macOS tabs moved out of the chrome row: they crowded the title bar
  // and the traffic lights. The tab list follows the header.
  assert.ok(
    html.indexOf("</header>") !== -1 && html.indexOf("</header>") < html.indexOf('role="tablist"'),
    "macOS renders the tabs in their own row below the chrome",
  );
  // No logo in the macOS chrome: real Mac apps don't put one next to the
  // traffic lights, and the traffic lights already are the window
  // controls. No logo window menu, no custom window buttons.
  assert.doesNotMatch(html, /aria-label="Window menu"/, "the logo window menu is Windows/Linux only");
  assert.doesNotMatch(html, /role="menu"/, "no window menu on macOS");
  assert.doesNotMatch(html, /aria-label="Minimize the window"/, "custom buttons are hidden on macOS");
  assert.doesNotMatch(html, /aria-label="Close the window"/, "custom buttons are hidden on macOS");
  // The project name is gone from the title bar (the logo's aria-label
  // still names it, but no text node does).
  assert.doesNotMatch(html, />LocalAide</, "the project name is gone from the title bar");
  // Tabs are flat pills, not bordered boxes, and the chrome carries no
  // divider when the tab row follows it — one line under the tab row is
  // the only divider, like a normal Mac app.
  assert.doesNotMatch(html, /rounded-t-lg/, "tabs are flat pills, not bordered boxes");
  assert.doesNotMatch(html, /border-ink-700/, "tabs are flat pills, not bordered boxes");
  assert.match(html, /bg-ink-900/, "the active tab is filled");
  assert.equal(
    (html.match(/border-b/g) ?? []).length,
    1,
    "exactly one divider: under the tab row",
  );
});

test("Windows/Linux renders the logo menu, tabs and custom buttons in the chrome", async () => {
  // No user agent: Node's own UA applies, so platform() is Linux.
  const html = await renderComponent("/src/components/TitleBar.svelte", {
    tabs,
    activeKey: "a",
    visible: true,
  });

  // The logo opens a window menu on Windows/Linux.
  assert.match(html, /aria-label="Window menu"/, "the logo opens a window menu");
  assert.match(html, /aria-label="Minimize the window"/, "custom buttons show off macOS");
  assert.match(html, /aria-label="Close the window"/, "custom buttons show off macOS");
  // Tabs stay in the chrome row on Windows/Linux.
  assert.ok(
    html.indexOf('role="tablist"') !== -1 && html.indexOf('role="tablist"') < html.indexOf("</header>"),
    "tabs sit in the chrome row off macOS",
  );
});

// What SSR cannot see. These stay source-text checks, with the reason each
// one cannot be rendered:
// - Double-click to zoom is an event handler: SSR never serializes one.
// - The window menu only renders once opened (menuOpen starts false), so
//   its items are unobservable without interaction.
// - The dead `onMenu` handler name is unobservable in rendered output.
// - App.svelte is the app root: rendering it would pull the whole app
//   (stores, screens, Tauri commands) into a unit test.
test("chrome behaviour that needs interaction or the app root stays source-checked", () => {
  const bar = read("src/components/TitleBar.svelte");
  assert.match(bar, /ondblclick/, "double-clicking empty chrome zooms the window");
  assert.doesNotMatch(bar, /onMenu/, "the dead title-bar menu button is gone");
  assert.match(bar, /role="menuitem"[\s\S]*?>Minimize</, "the window menu offers Minimize");
  assert.match(bar, /role="menuitem"[\s\S]*?>Close</, "the window menu offers Close");

  const app = read("src/App.svelte");
  assert.doesNotMatch(app, /import TabStrip/, "the old separate tab strip is gone");
  assert.match(app, /<TitleBar[\s\S]*tabs=\{chats\.tabs\}/, "tabs are passed into the chrome");
});
