// Idle-heat regressions (the 2026-11-23 profiling run: the old WebView sat
// at 95 C because the idle screen ran per-frame animations and a live
// backdrop blur). The GOAT restyle kept the invariants — now asserted
// against the current classes:
//   1. The idle screen runs no infinite animations (spinners during an
//      actual load are activity, not idle).
//   2. Everything that animates runs on the compositor (will-change).
//   3. The chat header is solid, not a live backdrop blur.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

test("no always-on infinite animation runs on the idle screen", () => {
  const logo = read("src/components/LogoMark.svelte");
  assert.doesNotMatch(logo, /infinite/, "the wordmark never loops");

  const mascot = read("src/components/GhostMascot.svelte");
  // The ghost's float is gated to the working state; the idle/hello paths run once.
  assert.match(mascot, /\.ghost-working \{[^}]*infinite/, "the float belongs to the working state");
  assert.doesNotMatch(mascot, /\.ghost-idle \{[^}]*infinite/, "the idle ghost is still");

  const empty = read("src/components/EmptyState.svelte");
  assert.doesNotMatch(empty, /empty-float/, "the empty-state icon does not float forever");
});

test("activity-only animations run on the compositor, not the main thread", () => {
  const bubble = read("src/components/MessageBubble.svelte");
  assert.match(bubble, /\.typing span \{[^}]*will-change: transform, opacity;/, "typing dots are composited");
  const thinking = read("src/components/ThinkingBlock.svelte");
  assert.match(thinking, /will-change: opacity;/, "the breathing label is composited");
  const library = read("src/components/LibraryStatus.svelte");
  assert.match(library, /will-change: transform;/, "the index sweep is composited");
  const session = read("src/components/SessionPicker.svelte");
  assert.match(session, /will-change: opacity;/, "the session pulse is composited");
  const mascot = read("src/components/GhostMascot.svelte");
  assert.match(mascot, /will-change: transform;/, "the working ghost is composited");
  const css = read("src/main.css");
  assert.match(css, /\.skeleton::after \{[^}]*will-change: transform;/, "the skeleton shimmer is composited");
  assert.match(css, /\.progress-indeterminate \{[^}]*will-change: transform;/, "the progress slide is composited");
});

test("the chat header is solid, not a live backdrop blur", () => {
  const chat = read("src/components/ChatHeader.svelte");
  assert.doesNotMatch(chat, /<header[^>]*backdrop-blur/, "the chat header does not blur its backdrop every frame");
});
