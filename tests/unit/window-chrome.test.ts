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

test("the chrome row carries both the window identity and the chat tabs", () => {
  const bar = read("src/components/TitleBar.svelte");
  assert.match(bar, /platform\(\) === "mac"/);
  assert.match(bar, /\{#if !isMac\}/, "custom buttons are hidden on macOS");
  assert.match(bar, /role="tablist"/, "chat tabs live in the same row");
  assert.match(bar, /data-tauri-drag-region/, "the row is still draggable");

  const app = read("src/App.svelte");
  assert.doesNotMatch(app, /import TabStrip/, "the old separate tab strip is gone");
  assert.match(app, /<TitleBar[\s\S]*tabs=\{chats\.tabs\}/, "tabs are passed into the chrome row");
});
