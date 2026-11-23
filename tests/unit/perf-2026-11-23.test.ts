// Regressions for the 2026-11-23 idle-heat profiling on Omar's Intel Mac:
// the WebView redrew every frame (per-frame layout + gradient rasterization
// on the GPU, 95 C at idle). Cause: always-on infinite animations — the
// pulsing SVG logo eye runs on WebKit's main thread — plus a backdrop-blur
// header and full-viewport gradients repainted each frame.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

test("no always-on infinite animation runs on the idle screen", () => {
  const logo = read("src/components/LogoMark.svelte");
  assert.doesNotMatch(logo, /\.logo-glow \{[^}]*infinite/, "the SVG logo eye pulses once on mount, then stays still");
  assert.match(logo, /animation: logo-pulse 2\.6s ease-in-out 1;/, "one pulse, not a loop");

  const empty = read("src/components/EmptyState.svelte");
  assert.doesNotMatch(empty, /empty-float/, "the empty-state icon no longer floats forever");
  assert.doesNotMatch(empty, /infinite/, "nothing loops on the idle empty screen");
});

test("activity-only animations run on the compositor, not the main thread", () => {
  const bubble = read("src/components/MessageBubble.svelte");
  assert.match(bubble, /\.typing span \{[^}]*will-change: transform, opacity;/, "typing dots are composited");
  const thinking = read("src/components/ThinkingBlock.svelte");
  assert.match(thinking, /\.shimmer \{[^}]*will-change: opacity;/, "the breathing label is composited");
  const library = read("src/components/LibraryStatus.svelte");
  assert.match(library, /\.bar span \{[^}]*will-change: transform;/, "the index sweep is composited");
  const picker = read("src/components/ModelPicker.svelte");
  assert.match(picker, /\.effort-max \.effort-fill::before \{[^}]*will-change: transform;/, "the effort sheen is composited");
  const session = read("src/components/SessionPicker.svelte");
  assert.match(session, /will-change: opacity;\s*animation: session-pulse/, "the session pulse is composited");
  const css = read("src/main.css");
  assert.match(css, /\.skeleton::after \{[^}]*will-change: transform;/, "the skeleton shimmer is composited");
  assert.match(css, /\.progress-indeterminate \{[^}]*will-change: transform;/, "the progress slide is composited");
});

test("the ember glow is rasterized once, and the chat header no longer blurs its backdrop every frame", () => {
  const css = read("src/main.css");
  assert.match(css, /\.ember-glow \{[^}]*transform: translateZ\(0\);/, "the gradient panel gets its own layer");
  const chat = read("src/screens/ChatScreen.svelte");
  assert.doesNotMatch(chat, /<header class="[^"]*backdrop-blur/, "the chat header is solid, not a live blur");
});
