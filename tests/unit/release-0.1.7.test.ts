import assert from "node:assert/strict";
import { test } from "node:test";
import type { ChatThread, MessagePart, ModelChoice, ModelChoices } from "../../src/lib/api/bindings.ts";
import { parseChart } from "../../src/lib/charts.ts";
import { groupChats, folderName } from "../../src/lib/chatGroups.ts";
import { addSubagentStep, questionItems, setSkillChange, setTodos, startTool } from "../../src/lib/chatParts.ts";
import { providerList, providerModels } from "../../src/lib/modelPicker.ts";

test("charts accept the common shapes and reject the rest", () => {
  const bar = parseChart('{"type":"bar","title":"Sales","labels":["Q1","Q2"],"series":[{"name":"2026","values":[3,"4.5"]}]}');
  assert.equal(bar?.type, "bar");
  assert.deepEqual(bar?.series[0].values, [3, 4.5]);
  const simple = parseChart('{"type":"pie","labels":["A","B","C"],"values":[1,2,3]}');
  assert.equal(simple?.series[0].values.length, 3);
  const rows = parseChart('{"type":"line","data":[{"label":"Mon","value":2},{"label":"Tue","value":5}]}');
  assert.deepEqual(rows?.labels, ["Mon", "Tue"]);
  assert.equal(parseChart("not json"), null);
  assert.equal(parseChart('{"type":"bar"}'), null);
  // Missing labels are numbered; unknown types draw as bars.
  assert.deepEqual(parseChart('{"type":"radar","values":[1,2]}')?.labels, ["1", "2"]);
});

const thread = (id: string, at: number, folder: string | null = null): ChatThread => ({ id, title: id, created_at: at, updated_at: at, message_count: 2, folder });

test("chats group by project folder, newest first", () => {
  const groups = groupChats(
    [thread("a", 10, "/work/site"), thread("b", 30), thread("c", 20, "/work/site"), thread("d", 40, "C:\\code\\app")],
    ["/new/empty"],
  );
  assert.deepEqual(groups.folders.map((f) => f.name), ["app", "site", "empty"]);
  assert.deepEqual(groups.folders[1].threads.map((t) => t.id), ["c", "a"]);
  assert.deepEqual(groups.plain.map((t) => t.id), ["b"]);
  assert.equal(folderName("/work/site/"), "site");
  // A search hides folders without a match.
  assert.equal(groupChats([thread("x", 1, "/f")], ["/g"], "zzz").folders.length, 0);
});

test("question parts read old and new shapes", () => {
  const old: Extract<MessagePart, { type: "question" }> = { type: "question", id: "q", question: "Which?", options: ["A", "B"], multiple: false };
  assert.deepEqual(questionItems(old)[0].options.map((o) => o.label), ["A", "B"]);
  const fresh = { ...old, items: [{ header: "Clarify", question: "What?", options: [{ label: "X", description: "x" }], multiple: false }] };
  assert.equal(questionItems(fresh)[0].header, "Clarify");
});

test("todos, subagent steps and learned skills update the answer", () => {
  const parts: MessagePart[] = [];
  setTodos(parts, [{ text: "one", status: "pending" }]);
  setTodos(parts, [{ text: "one", status: "done" }]);
  assert.equal(parts.filter((p) => p.type === "todos").length, 1);
  startTool(parts, { id: "s1", name: "run_subagent", title: "Subagent", detail: "Check tests", group: "subagent", arguments: "{}" }, 0);
  addSubagentStep(parts, "s1", "Read a file: a.rs");
  addSubagentStep(parts, "s1", "Read a file: a.rs");
  addSubagentStep(parts, "s1", "Writing its report");
  const tool = parts.find((p) => p.type === "tool");
  assert.deepEqual(tool?.type === "tool" ? tool.steps : null, ["Read a file: a.rs", "Writing its report"]);
  startTool(parts, { id: "k1", name: "save_skill", title: "Learned a skill", detail: "Report", group: "skills", arguments: "{}" }, 0);
  setSkillChange(parts, { id: "k1", change_id: "1-report", name: "Report", action: "created" });
  const skill = parts.find((p) => p.type === "tool" && p.id === "k1");
  assert.equal(skill?.type === "tool" ? skill.skill?.change_id : null, "1-report");
});

const model = (over: Partial<ModelChoice>): ModelChoice => ({
  key: "k", provider_id: null, provider_label: "This computer", model: "m", name: "M", local: true, reasoning: false, vision: false, tools: true,
  context: 8192, efforts: [], free: true, favorite: false, active: false, size_bytes: null, ...over,
});

test("the model picker lists providers and their models", () => {
  const choices: ModelChoices = {
    models: [
      model({ key: "l1", name: "Qwen" }),
      model({ key: "z1", provider_id: "zai", provider_label: "Start Plan", name: "GLM-5.3-Flash", local: false, vision: true }),
      model({ key: "z2", provider_id: "zai", provider_label: "Start Plan", name: "GLM-5.3", local: false, favorite: true }),
    ],
    groups: [{ provider_id: "zai", label: "Start Plan", enabled: true, local_router: false, needs_refresh: false }],
    active_key: "z1",
    local_loaded: false,
    online_allowed: true,
  };
  const providers = providerList(choices);
  assert.deepEqual(providers.map((p) => p.label), ["This computer", "Start Plan"]);
  assert.equal(providers[1].badge, "Free");
  assert.deepEqual(providerModels(choices, "zai", "").map((m) => m.key), ["z2", "z1"], "starred first");
  assert.deepEqual(providerModels(choices, "zai", "flash").map((m) => m.key), ["z1"]);
  assert.equal(providerList({ ...choices, online_allowed: false }).length, 1);
});
