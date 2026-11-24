import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

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

test("the chrome row is clean on macOS; tabs sit below it", () => {
  const bar = read("src/components/TitleBar.svelte");
  assert.match(bar, /platform\(\) === "mac"/);
  assert.match(bar, /\{#if !isMac\}/, "custom buttons are hidden on macOS");
  assert.match(bar, /role="tablist"/, "chat tabs are rendered");
  assert.match(bar, /data-tauri-drag-region/, "the row is still draggable");
  // macOS tabs moved out of the chrome row: they crowded the title bar
  // and the traffic lights. Windows keeps them in the row.
  assert.match(bar, /tabsBelow/, "macOS renders the tabs in their own row below the chrome");
  assert.match(bar, /ondblclick/, "double-clicking empty chrome zooms the window");
  assert.doesNotMatch(bar, /onMenu/, "the dead title-bar menu button is gone");
  assert.doesNotMatch(bar, /LocalAide<\/span>/, "the project name is gone from the title bar");
  // No logo in the macOS chrome: real Mac apps don't put one next to the
  // traffic lights, and the traffic lights already are the window
  // controls. The logo window menu lives on Windows/Linux only.
  assert.match(
    bar,
    /\{#if !isMac\}[\s\S]*?aria-label="Window menu"/,
    "the logo window menu is Windows/Linux only",
  );
  assert.match(bar, /Minimize/, "the window menu offers Minimize");
  assert.match(bar, /Close/, "the window menu offers Close");

  const app = read("src/App.svelte");
  assert.doesNotMatch(app, /import TabStrip/, "the old separate tab strip is gone");
  assert.match(app, /<TitleBar[\s\S]*tabs=\{chats\.tabs\}/, "tabs are passed into the chrome");
});
