// The setup self-test: find a model, check the engine, load, one-word reply.
// Used by the Settings screen (over a Channel) and the shared link (as
// NDJSON lines) — `send` gets each step object.

import { loadConfig } from "../core/config.mjs";
import { engineStatus, engineInfo } from "./engine.mjs";
import { getEntry } from "./models.mjs";
import { complete, buildBody } from "./stream.mjs";

export async function runSelfTest(send) {
  const started = Date.now();
  const steps = [];
  const push = (step, label, status, ms, detail, percent = null) => {
    const entry = { step, label, status, ms, detail, percent };
    steps.push(entry);
    send(entry);
    return entry;
  };

  let t0 = Date.now();
  const entry = getEntry((loadConfig().selected_models ?? [])[0] ?? "");
  push("model", "Find a model", entry ? "pass" : "fail", Date.now() - t0, entry ? entry.name : "No model is selected or installed.", 25);

  t0 = Date.now();
  const installed = engineInfo().installed;
  push("engine", "Check the model engine", installed ? "pass" : "fail", Date.now() - t0, installed ? "llama-server is on this computer." : "The engine is not downloaded yet.", 50);

  const loaded = engineStatus().loaded;
  t0 = Date.now();
  push("load", "Load the model", loaded ? "pass" : "skip", Date.now() - t0, loaded ? engineStatus().model : "Nothing is loaded; start a chat to load one.", 75);

  t0 = Date.now();
  let reply = "skip";
  let detail = "No model is loaded.";
  if (loaded) {
    try {
      const target = {
        url: `http://127.0.0.1:${engineStatus().port}/v1/chat/completions`,
        bearer: null,
      };
      const result = await complete(target.url, {
        bearer: target.bearer,
        body: buildBody({
          model: "",
          messages: [{ role: "user", content: "Reply with the word: ready" }],
          maxTokens: 16,
          effort: { kind: "off", level: null },
          tools: [],
        }),
        timeoutMs: 30_000,
      });
      reply = result.content.trim() ? "pass" : "fail";
      detail = result.content.trim() ? "The model answered." : "The model gave no answer.";
    } catch (e) {
      reply = "fail";
      detail = String(e?.message ?? e).slice(0, 200);
    }
  }
  push("reply", "Ask for a one-word reply", reply, Date.now() - t0, detail, 100);

  return {
    passed: steps.every((s) => s.status !== "fail"),
    model_id: entry?.id ?? "",
    steps,
    ms: Date.now() - started,
  };
}
