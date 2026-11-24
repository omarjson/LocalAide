// Logic behind the chat box and the model menu, tested without a window.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { ModelChoice, ModelChoices, Skill } from "../../src/lib/api/bindings.ts";
import { formatGb } from "../../src/lib/format.ts";
import { NO_FILTERS, matchesModel, parseFilters, pickerSections, providerList } from "../../src/lib/modelPicker.ts";
import { MAX_SKILL_ITEMS, SLASH_COMMANDS, slashItems, slashQuery } from "../../src/lib/slash.ts";
import { speakableText } from "../../src/lib/speech.ts";

const model = (over: Partial<ModelChoice>): ModelChoice => ({
  key: over.key ?? `${over.provider_id ?? "local"}:${over.model ?? "m"}`,
  provider_id: null,
  provider_label: "This computer",
  model: "m",
  name: "Model",
  local: true,
  reasoning: false,
  vision: false,
  tools: false,
  context: 8192,
  efforts: [],
  free: false,
  favorite: false,
  active: false,
  size_bytes: null,
  ...over,
});

const choices = (models: ModelChoice[], online = true): ModelChoices => ({
  models,
  groups: [
    { provider_id: "or", label: "OpenRouter", enabled: true, local_router: false, needs_refresh: false },
    { provider_id: "off", label: "Hidden", enabled: false, local_router: false, needs_refresh: false },
  ],
  active_key: null,
  local_loaded: true,
  online_allowed: online,
});

const skill = (name: string, over: Partial<Skill> = {}): Skill => ({ name, description: "", tags: [], body: `${name} body`, path: `/skills/${name}.md`, ...over });

test("model menu: search and filters both apply", () => {
  const qwen = model({ name: "Qwen3 8B", model: "qwen3", reasoning: true, tools: true });
  const gemma = model({ name: "Gemma 3", model: "gemma3", vision: true });
  assert.ok(matchesModel(qwen, "qwen", NO_FILTERS));
  assert.ok(!matchesModel(gemma, "qwen", NO_FILTERS));
  assert.ok(matchesModel(qwen, "", { ...NO_FILTERS, reasoning: true, tools: true }));
  assert.ok(!matchesModel(gemma, "", { ...NO_FILTERS, reasoning: true }));
  assert.ok(matchesModel(gemma, "  GEMMA ", { ...NO_FILTERS, vision: true }), "search ignores case and spaces");
});

test("model menu: a starred model shows once, and hidden providers do not show", () => {
  const local = model({ key: "l1", name: "Local" });
  const starredOnline = model({ key: "o1", provider_id: "or", provider_label: "OpenRouter", local: false, favorite: true, name: "Starred online" });
  const online = model({ key: "o2", provider_id: "or", provider_label: "OpenRouter", local: false, name: "Online" });
  const sections = pickerSections(choices([local, starredOnline, online]), "", NO_FILTERS);
  assert.deepEqual(sections.map((s) => s.id), ["starred", "local", "or"]);
  const all = sections.flatMap((s) => s.items.map((m) => m.key));
  assert.equal(all.filter((k) => k === "o1").length, 1, "starred model is not repeated");
  assert.equal(sections[2].starred, 1);
});

test("model menu: with online models switched off, only this computer is listed", () => {
  const local = model({ key: "l1" });
  const sections = pickerSections(choices([local], false), "", NO_FILTERS);
  assert.deepEqual(sections.map((s) => s.id), ["local"]);
});

test("model menu: the same provider saved twice appears once", () => {
  const kilo = model({ key: "k1", provider_id: "kilo", provider_label: "Kilo Code (free, no key)", local: false, free: true, name: "Kilo auto" });
  const dup = choices([kilo]);
  dup.groups = [
    { provider_id: "kilo", label: "Kilo Code (free, no key)", enabled: true, local_router: false, needs_refresh: false },
    { provider_id: "kilo", label: "Kilo Code (free, no key)", enabled: true, local_router: false, needs_refresh: false },
  ];
  assert.deepEqual(providerList(dup).map((p) => p.key), ["local", "kilo"]);
});

test("model menu: saved filters survive bad storage", () => {
  assert.deepEqual(parseFilters('{"vision":true,"free":"yes"}'), { ...NO_FILTERS, vision: true });
  assert.deepEqual(parseFilters("not json"), NO_FILTERS);
  assert.deepEqual(parseFilters(null), NO_FILTERS);
});

test("slash menu: only text starting with / opens it", () => {
  assert.equal(slashQuery("hello /web"), null);
  assert.equal(slashQuery("/Web"), "web");
  assert.deepEqual(slashItems("hello", [skill("x")]), []);
});

test("slash menu: commands come before skills, and the list is what the cursor walks", () => {
  const skills = Array.from({ length: 10 }, (_, i) => skill(`goal-${i}`));
  const items = slashItems("/goal", skills);
  assert.equal(items[0].kind, "command");
  assert.equal(items[0].kind === "command" && items[0].value.id, "goal");
  assert.equal(items.filter((i) => i.kind === "skill").length, MAX_SKILL_ITEMS);
  const everything = slashItems("/", []);
  assert.equal(everything.length, SLASH_COMMANDS.length, "a bare / lists every command");
});

test("slash menu: skills match by name, description or tag", () => {
  const s = [skill("review", { tags: ["code"] }), skill("haiku", { description: "Write a short poem" })];
  assert.deepEqual(slashItems("/code", s).filter((i) => i.kind === "skill").map((i) => i.kind === "skill" && i.value.name), ["review"]);
  assert.deepEqual(slashItems("/poem", s).map((i) => i.kind === "skill" && i.value.name), ["haiku"]);
});

test("sizes use one unit, GB", () => {
  assert.equal(formatGb(3052 * 1024 * 1024), "3.0 GB");
  assert.equal(formatGb(140 * 1024 * 1024), "0.14 GB");
  assert.equal(formatGb(0), "0 GB");
});

test("read aloud: formatting, code and citation marks are not spoken", () => {
  const text = speakableText("## Title\n\nUse **bold** and `code` [1].\n\n```js\nlet x = 1;\n```\n- item one\n[a link](https://example.com)");
  assert.equal(text, "Title Use bold and code. Code block. item one a link");
});
