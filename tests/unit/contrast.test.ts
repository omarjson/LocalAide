import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// Token-level contrast gate. It recomputes the ratios printed in
// docs/evidence/a11y.md from the shipped CSS, so a future palette edit that
// drops below WCAG 2.2 AA fails CI. It is still arithmetic, not an
// on-screen measurement — that open item stays open.

const css = readFileSync(new URL("../../src/main.css", import.meta.url), "utf8");
const lightAt = css.indexOf(':root[data-theme="light"]');

function palette(from: number, to: number): Record<string, string> {
  const text = css.slice(from, to);
  const result: Record<string, string> = {};
  for (const match of text.matchAll(/--color-([a-z0-9-]+):\s*(#[0-9a-fA-F]{6})/g)) {
    result[match[1]] = match[2];
  }
  return result;
}

const dark = palette(0, lightAt);
const light = palette(lightAt, css.length);

// Read the shipped focus ring and state tint out of the CSS instead of
// hardcoding a color: the ring is a solid outline in --color-<token>, and the
// focused field's box-shadow is a color-mix() of another token. Both move with
// the palette, so the gate follows a rename rather than going stale.
const ringMatch = css.match(/^:focus-visible\s*\{[^}]*?outline:\s*2px solid var\(--color-([a-z0-9-]+)\)/m);
const ringToken = ringMatch?.[1];
const mixMatch = css.match(/box-shadow:\s*0 0 0 3px color-mix\(in oklab, var\(--color-([a-z0-9-]+)\)\s+\d+%,\s*transparent\)/);
const mixToken = mixMatch?.[1];

function luminance(hex: string): number {
  const channels = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const linear = channels.map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(fg: string, bg: string): number {
  const [hi, lo] = [luminance(fg), luminance(bg)].sort((a, b) => b - a);
  return (hi + 0.05) / (lo + 0.05);
}

test("the shipped focus ring and state tint are tokenized", () => {
  assert.ok(ringToken, "a solid :focus-visible ring token must exist in main.css");
  assert.ok(mixToken, "a color-mix() focus-state tint token must exist in main.css");
  assert.ok(!css.includes("#d946ef"), "the retired magenta focus color must not ship");
});

for (const [name, tokens] of [["dark", dark], ["light", light]] as const) {
  test(`${name} theme: text keeps 4.5:1 and borders 3:1`, () => {
    const bg = tokens["ink-950"];
    const surface = tokens["ink-900"];
    assert.ok(bg && surface, "theme tokens must exist");
    for (const role of ["ink-100", "ink-400", "ink-500", "ink-600"]) {
      assert.ok(contrast(tokens[role], bg) >= 4.5, `${role} on page background`);
      assert.ok(contrast(tokens[role], surface) >= 4.5, `${role} on surface`);
    }
    assert.ok(contrast(tokens.edge, bg) >= 3, "edge on page background");
    assert.ok(contrast(tokens.edge, surface) >= 3, "edge on surface");
    assert.ok(contrast(tokens["on-ember"], tokens["ember-600"]) >= 4.5, "button text");
  });

  test(`${name} theme: focus ring and state tint clear the non-text floor`, () => {
    assert.ok(ringToken && mixToken, "focus tokens must be declared in main.css");
    const bg = tokens["ink-950"];
    const ring = tokens[ringToken];
    const tint = tokens[mixToken];
    assert.ok(ring && tint, `${name}: focus/state tokens must resolve to colors`);
    assert.ok(contrast(ring, bg) >= 3, `${name}: --color-${ringToken} focus ring on page background`);
    assert.ok(contrast(tint, bg) >= 3, `${name}: --color-${mixToken} color-mix() state tint on page background`);
  });
}
