// The chat loop: send, stream, run tools, ask for approval, repeat.
// Emits exactly the ChatStreamEvent shapes the Svelte UI already renders.

import crypto from "node:crypto";
import { channel } from "../core/channels.mjs";
import { loadConfig, onlineAllowed } from "../core/config.mjs";
import { AppError } from "../core/errors.mjs";
import { log } from "../core/log.mjs";
import { runTool, looksReadOnly } from "./tools.mjs";
import { schemasFor, toolsFor, TOOLS } from "./toolSpecs.mjs";
import { buildBody, complete, effortOf, readStream, thinkingAllowance, ToolCallAccumulator } from "./stream.mjs";
import { localEndpoint } from "./engine.mjs";
import { recordUsage } from "./usage.mjs";
import { appendMessage, ensureThread, readThread } from "./threads.mjs";
import { memoryContext } from "./memory.mjs";
import { modeOf, composeSystemPrompt, composeCompactSystemPrompt, toolGuidance } from "./modes.mjs";
import { learnSkill } from "./skills.mjs";
import { specsForChat, runChatTool } from "./mcp.mjs";

/** Running jobs, so a cancel or a steer can reach them. */
const jobs = new Map();

export const cancelJob = (jobId) => {
  const job = jobs.get(jobId);
  if (!job) return false;
  job.controller.abort();
  return true;
};

export const steerJob = (jobId, text) => {
  const job = jobs.get(jobId);
  if (!job || job.done) return false;
  job.steers.push(String(text ?? ""));
  return true;
};

/** Approval decisions the UI sent back, keyed by approval id. */
const approvals = new Map();

export function decideTool(approvalId, decision) {
  const pending = approvals.get(approvalId);
  if (!pending) return false;
  approvals.delete(approvalId);
  pending.resolve(String(decision ?? "deny"));
  return true;
}

/**
 * Asks the UI and waits for the answer. `timer` seconds (0 = wait) is the
 * approval_timers countdown: a terminal command runs, or a plan accepts,
 * after that long unless the user declines first. Nothing waits forever.
 */
function askApproval(emit, { id, title, detail, preview, timer = 0 }) {
  return new Promise((resolve) => {
    const approvalId = crypto.randomUUID();
    approvals.set(approvalId, { resolve });
    emit({
      type: "tool_approval",
      payload: { approval_id: approvalId, id, title, detail, preview },
    });
    const finish = (decision) => {
      if (approvals.delete(approvalId)) resolve(decision);
    };
    if (timer > 0) {
      setTimeout(() => finish("approve"), Math.min(600, Math.round(timer)) * 1000);
    } else {
      setTimeout(() => finish("timeout"), 5 * 60_000);
    }
  });
}

/** Which provider a chat request goes to. */
export function resolveTarget(req = {}) {
  const cfg = loadConfig();
  const target = req.target ?? null;
  const providerId = target?.provider_id ?? (req.target === undefined && cfg.cloud_llm_enabled && onlineAllowed(cfg) ? cfg.active_cloud_provider : null);
  if (providerId) {
    if (!onlineAllowed(cfg)) {
      throw new AppError("CLOUD_OFF", "Online providers are switched off. Turn them on in Settings → Online providers.");
    }
    const provider = cfg.cloud_providers.find((p) => p.id === providerId);
    if (!provider) throw AppError.notFound("That provider is no longer saved.");
    if (!provider.enabled) throw new AppError("PROVIDER_OFF", `${provider.label} is switched off.`);
    const base = provider.base_url.replace(/\/+$/, "");
    const model =
      target?.model ||
      (cfg.active_cloud_provider === provider.id ? cfg.active_cloud_model : null) ||
      provider.model ||
      "";
    return {
      url: `${base}/chat/completions`,
      bearer: provider.api_key || null,
      label: provider.label,
      model,
      local: false,
      providerId: provider.id,
    };
  }
  const local = localEndpoint();
  if (!local) {
    throw new AppError("NO_MODEL", "No model is loaded. Pick one in the model menu, or add an online provider.");
  }
  return { url: local.url, bearer: local.bearer, label: "This computer", model: "", local: true, providerId: null };
}

/** Turns the saved history into wire messages. */
function historyToWire(history = [], systemPrompt) {
  const messages = [];
  if (systemPrompt?.trim()) messages.push({ role: "system", content: systemPrompt.trim() });
  for (const msg of history) {
    if (msg.steer) continue; // a steer is never sent back
    if (msg.error) continue;
    const role = msg.role === "assistant" ? "assistant" : "user";
    const content = String(msg.content ?? "");
    if (!content.trim() && !msg.images?.length) continue;
    if (role === "user" && msg.images?.length) {
      messages.push({
        role,
        content: [
          ...(content.trim() ? [{ type: "text", text: content }] : []),
          ...msg.images.map((url) => ({ type: "image_url", image_url: { url } })),
        ],
      });
    } else {
      messages.push({ role, content });
    }
  }
  return messages;
}

/** Whether an approval mode asks before this call, from the UI's modes. */
function needsApprovalFor(spec, call, args, mode, allowedTools, timers) {
  if (allowedTools.includes(call.name)) return false; // "Allow for this chat"
  // Only tools that change something can ask; MCP servers set risky from
  // their auto_approve switch.
  if (!spec.risky) return false;
  if (mode === "auto") return false;
  // smart asks only when there is risk: read-only shell work runs by itself.
  if (mode === "smart" && call.name === "run_command" && looksReadOnly(args.command)) return false;
  // plan_auto_accept: a plan shown for approval accepts after N seconds;
  // terminal commands run after the terminal countdown, unless 0 = wait.
  const timer =
    mode === "plan"
      ? timers.plan_auto_accept_secs
      : call.name === "run_command"
        ? timers.terminal_approval_secs
        : 0;
  return { ask: true, timer };
}

/**
 * Runs one chat turn. Returns the job id immediately; every event streams
 * through the channel.
 */
export function startChat(req, channelId) {
  const jobId = crypto.randomUUID();
  const emit = (event) => channel(channelId).send(event);
  const controller = new AbortController();
  const job = { controller, steers: [], done: false, id: jobId };
  jobs.set(jobId, job);

  // Runs in the background: the IPC call returns the job id at once.
  void (async () => {
    try {
      await runChat(req, { job, emit, controller });
    } catch (e) {
      const message = e instanceof AppError ? e.message : "LocalAide could not answer that. Try again.";
      log("error", "chat failed", String(e?.stack ?? e));
      emit({ type: "error", payload: { error: message } });
    } finally {
      job.done = true;
      jobs.delete(jobId);
    }
  })();

  return jobId;
}

async function runChat(req, { job, emit, controller }) {
  const started = Date.now();
  const target = resolveTarget(req);
  const effort = effortOf(req.effort);
  const cfg = loadConfig();
  const mode = modeOf(req.mode);
  const approvalMode = req.approval_mode ?? "ask";
  const timers = {
    plan_auto_accept_secs: Math.min(600, Math.max(0, cfg.plan_auto_accept_secs ?? 0)),
    terminal_approval_secs: Math.min(600, Math.max(0, cfg.terminal_approval_secs ?? 60)),
  };

  // Autonomous gathers sources on its own; the other modes follow the switches.
  const useWeb = req.use_web === true || mode.gathers_web === true;
  const useRecall = req.use_recall !== false && (cfg.developer?.modules?.memory ?? true);
  const useFiles = req.use_files !== false;
  const useSkills = req.use_skills !== false && (cfg.developer?.modules?.skills ?? true);
  const useShell = req.use_shell === true;
  const useSubagents = req.use_subagents !== false;
  const allowedTools = Array.isArray(req.allowed_tools) ? req.allowed_tools : [];

  const toolNames = toolsFor({ useWeb, useRecall, useFiles, useSkills, useShell, useSubagents });
  const tools = schemasFor(toolNames);

  // The user's MCP servers add their tools to the offer. A server that
  // cannot be reached is skipped; the model just doesn't see its tools.
  const mcpRequested = Array.isArray(req.mcp_servers) ? req.mcp_servers : null;
  const mcpSpecs = (cfg.mcp_servers ?? []).length > 0 ? await specsForChat(mcpRequested ?? []) : [];
  for (const spec of mcpSpecs) {
    if (!tools.some((t) => t.function.name === spec.name)) {
      tools.push({
        type: "function",
        function: { name: spec.name, description: spec.description, parameters: spec.parameters },
      });
    }
  }

  // The notes and documents worth showing the model before it answers.
  const recall = useRecall ? await memoryContext(String(req.message ?? "")) : "";

  const userPrompt = String(req.system_prompt ?? cfg.chat_system_prompt ?? "").trim();
  let context = recall;
  if (req.now) context = `${context ? `${context}\n\n` : ""}[Today is ${req.now}.]`;
  const compact = cfg.use_compact_prompt === true && target.local;
  let systemPrompt = (compact ? composeCompactSystemPrompt : composeSystemPrompt)(userPrompt, mode, context);
  if (!systemPrompt) systemPrompt = "You are LocalAide, a private assistant running on the user's own computer.";

  // Tool guidance rides along when any tool group is on.
  const groups = [useWeb && "web", useRecall && "notes", useFiles && "files", useSkills && "skills", useShell && "shell", mcpSpecs.length > 0 && "mcp"].filter(Boolean);
  const guidance = (compact ? "" : toolGuidance(groups, mode.id)) || "";
  if (guidance) systemPrompt = `${systemPrompt}\n\n${guidance}`;

  const history = req.resume === true ? (req.history ?? []) : (req.history ?? []);
  const messages = historyToWire(history, systemPrompt);
  const userContent = String(req.message ?? "");
  const images = Array.isArray(req.images) ? req.images : [];
  if (req.resume !== true) {
    messages.push(
      images.length
        ? {
            role: "user",
            content: [
              ...(userContent.trim() ? [{ type: "text", text: userContent }] : []),
              ...images.map((url) => ({ type: "image_url", image_url: { url } })),
            ],
          }
        : { role: "user", content: userContent },
    );
  }

  // Attached text files ride along as part of the message.
  for (const file of req.attachments ?? []) {
    if (file?.text) {
      messages.push({
        role: "user",
        content: `[Attached file: ${file.name}]\n${String(file.text).slice(0, 60_000)}`,
      });
    }
  }

  const maxTokens = Number(req.max_tokens) > 0 ? Number(req.max_tokens) : 4096;
  // Strict keeps the model precise: its temperature is capped at 0.3.
  const requestedTemp = Number(req.temperature);
  let temperature = Number.isFinite(requestedTemp) ? requestedTemp : undefined;
  if (mode.temperature_cap !== null && temperature !== undefined) {
    temperature = Math.min(temperature, mode.temperature_cap);
  }
  const allowTools = mode.id !== "strict";
  // A chat that works in a project folder jails its file tools there.
  const cwdRoot = req.folder && String(req.folder).trim() ? String(req.folder).trim() : null;

  let round = 0;
  let totalPrompt = null;
  let totalCompletion = 0;
  let firstPrompt = null;
  let firstTokenAt = null;
  let toolCallsCount = 0;
  const MAX_ROUNDS = mode.max_rounds;

  while (round < MAX_ROUNDS) {
    round += 1;
    // A steer typed while the answer runs joins the next round.
    while (job.steers.length > 0) {
      const note = job.steers.shift();
      messages.push({ role: "user", content: `[The user added while you were answering: ${note}]` });
      emit({ type: "steered", payload: { text: note } });
    }
    if (controller.signal.aborted) {
      finishRun(emit, req, target, {
        finish: "stopped", totalPrompt, totalCompletion, firstPrompt, systemPrompt, tools,
        started, firstTokenAt, toolCallsCount, cancelled: true,
      });
      return;
    }

    const body = buildBody({
      model: target.model,
      messages,
      maxTokens: maxTokens + thinkingAllowance(effort),
      temperature,
      tools: allowTools ? tools : [],
      effort,
      usage: true,
      forbidTools: false,
    });

    emit({ type: "status", payload: { label: round === 1 ? "Thinking" : "Reading the results" } });

    const response = await fetch(target.url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "text/event-stream",
        ...(target.bearer ? { Authorization: `Bearer ${target.bearer}` } : {}),
      },
      body: JSON.stringify(body),
      signal: controller.signal,
    }).catch((e) => {
      if (e?.name === "AbortError") return null;
      throw new AppError("PROVIDER_UNREACHABLE", "Could not reach the model. Check that it is running.", {
        detail: String(e),
        retryable: true,
      });
    });

    if (!response) {
      finishRun(emit, req, target, {
        finish: "stopped", totalPrompt, totalCompletion, firstPrompt, systemPrompt, tools,
        started, firstTokenAt, toolCallsCount, cancelled: true,
      });
      return;
    }
    if (!response.ok) {
      const text = await response.text().catch(() => "");
      throw new AppError(
        response.status === 401 || response.status === 403 ? "PROVIDER_AUTH" : "PROVIDER_ERROR",
        response.status === 401 || response.status === 403
          ? `${target.label} did not accept the key.`
          : `${target.label} answered with an error (${response.status}).`,
        { detail: text.slice(0, 800), retryable: response.status >= 500 || response.status === 429 },
      );
    }

    const calls = new ToolCallAccumulator();
    let content = "";
    let finish = null;
    let roundPrompt = null;
    let roundCompletion = null;

    for await (const frame of readStream(response, { signal: controller.signal })) {
      if (frame.kind === "content") {
        if (firstTokenAt === null) firstTokenAt = Date.now();
        content += frame.text;
        emit({ type: "token", payload: { token: frame.text } });
      } else if (frame.kind === "reasoning") {
        emit({ type: "reasoning", payload: { text: frame.text } });
      } else if (frame.kind === "tool") {
        calls.push(frame);
      } else if (frame.kind === "finish") {
        finish = frame.reason;
      } else if (frame.kind === "usage") {
        roundPrompt = frame.prompt;
        roundCompletion = frame.completion;
      } else if (frame.kind === "error") {
        throw new AppError("PROVIDER_ERROR", frame.message);
      } else if (frame.kind === "end" && controller.signal.aborted) {
        finishRun(emit, req, target, {
          finish: "stopped", totalPrompt, totalCompletion, firstPrompt, systemPrompt, tools,
          started, firstTokenAt, toolCallsCount, cancelled: true,
        });
        return;
      }
    }

    if (roundPrompt !== null) {
      totalPrompt = roundPrompt;
      if (firstPrompt === null) firstPrompt = roundPrompt;
    }
    totalCompletion += roundCompletion ?? 0;

    const toolCalls = calls.list();
    if (toolCalls.length === 0 || !allowTools) {
      // Announce tool calls the model made but could not run.
      if (content) {
        appendMessage(req.thread_id, { role: "assistant", content, ts: Date.now() });
      }
      finishRun(emit, req, target, {
        finish, totalPrompt, totalCompletion, firstPrompt, systemPrompt, tools,
        started, firstTokenAt, toolCallsCount, cancelled: false,
      });
      return;
    }

    // Announce each call, then run it (asking first when it changes something).
    messages.push({
      role: "assistant",
      content: content || null,
      tool_calls: toolCalls.map((call) => ({
        id: call.id,
        type: "function",
        function: { name: call.name, arguments: call.arguments },
      })),
    });

    for (const call of toolCalls) {
      const spec = TOOLS[call.name] ?? mcpSpecs.find((s) => s.name === call.name);
      if (!spec) continue;
      let args = {};
      try {
        args = JSON.parse(call.arguments || "{}");
      } catch {
        args = {};
      }
      const isMcp = Boolean(spec.serverId);

      // ask_user and the plan tools talk to the UI, not the disk.
      if (call.name === "ask_user") {
        const questions = Array.isArray(args.questions) ? args.questions : [];
        const first = questions[0];
        if (first) {
          emit({
            type: "question",
            payload: {
              id: call.id,
              question: String(first.question ?? ""),
              options: (first.options ?? []).map((o) => (typeof o === "string" ? o : o.label)),
              multiple: first.multiple === true,
              items: questions.map((q) => ({
                id: String(q.id ?? ""),
                header: String(q.header ?? ""),
                question: String(q.question ?? ""),
                options: (q.options ?? []).map((o) => ({
                  label: String(o?.label ?? o ?? ""),
                  description: o?.description ?? null,
                  recommended: o?.recommended === true,
                })),
                multiple: q.multiple === true,
              })),
            },
          });
        }
        messages.push({ role: "tool", tool_call_id: call.id, content: "The question is shown to the user." });
        continue;
      }
      if (call.name === "todo_write") {
        emit({ type: "todos", payload: { items: Array.isArray(args.items) ? args.items : [] } });
        messages.push({ role: "tool", tool_call_id: call.id, content: "The plan is shown." });
        continue;
      }

      const title = spec.title ?? call.name;
      emit({
        type: "tool_call",
        payload: {
          id: call.id,
          name: call.name,
          title,
          detail: String(args.path ?? args.query ?? args.command ?? args.url ?? (isMcp ? JSON.stringify(args).slice(0, 120) : "")),
          group: spec.group ?? "chat",
          arguments: call.arguments,
        },
      });

      // Plan mode refuses anything that changes something, outright.
      const writes = new Set(["write_file", "run_command", "save_note", "learn_skill"]);
      if (approvalMode === "plan" && writes.has(call.name)) {
        const summary = "Plan mode is on, so nothing was changed. Describe the change and ask first.";
        emit({ type: "tool_result", payload: { id: call.id, ok: false, summary, output: "" } });
        messages.push({ role: "tool", tool_call_id: call.id, content: summary });
        continue;
      }

      // Approval: anything that changes something asks, unless the mode or
      // an explicit allow for this chat says otherwise.
      const approval = needsApprovalFor(spec, call, args, approvalMode, allowedTools, timers);
      let decision = "approve";
      if (approval) {
        decision = await askApproval(emit, {
          id: call.id,
          title,
          detail: String(args.path ?? args.command ?? args.query ?? ""),
          preview: call.arguments.slice(0, 600),
          timer: typeof approval === "object" ? approval.timer : 0,
        });
      }

      if (decision !== "approve") {
        const summary = decision === "timeout" ? "No answer, so it was skipped." : "You said no.";
        emit({ type: "tool_result", payload: { id: call.id, ok: false, summary, output: "" } });
        messages.push({ role: "tool", tool_call_id: call.id, content: summary });
        continue;
      }

      toolCallsCount += 1;

      // An MCP tool runs on its server; the server's auto_approve decides
      // whether the call asked first (it did, unless the server is trusted).
      if (isMcp) {
        const result = await runChatTool(call.name, args);
        const output = String(result.text ?? "").slice(0, 60_000);
        const summary = result.ok ? (output.split("\n")[0] ?? "Done").slice(0, 120) || "Done" : result.text.slice(0, 200) || "The tool failed.";
        emit({
          type: "tool_result",
          payload: { id: call.id, ok: result.ok, summary, output },
        });
        messages.push({ role: "tool", tool_call_id: call.id, content: output || summary });
        continue;
      }

      // learn_skill talks to the skills service and reports its change, so
      // the Skills screen's Undo can revert it.
      if (call.name === "learn_skill") {
        try {
          const learned = learnSkill(args.name, args.description, args.instructions);
          emit({
            type: "skill_change",
            payload: {
              id: call.id,
              change_id: learned.changeId,
              name: learned.skill.name,
              action: learned.created ? "created" : "updated",
            },
          });
          emit({
            type: "tool_result",
            payload: {
              id: call.id,
              ok: true,
              summary: learned.created ? `Saved skill ${learned.skill.name}` : `Updated skill ${learned.skill.name}`,
              output: learned.skill.body.slice(0, 2000),
            },
          });
          messages.push({ role: "tool", tool_call_id: call.id, content: `Saved skill "${learned.skill.name}".` });
        } catch (e) {
          const summary = e?.message ?? "The skill could not be saved.";
          emit({ type: "tool_result", payload: { id: call.id, ok: false, summary, output: "" } });
          messages.push({ role: "tool", tool_call_id: call.id, content: summary });
        }
        continue;
      }

      // A subagent works on one self-contained job and reports back.
      if (call.name === "subagent") {
        const result = await runSubagent(target, call, args, emit, controller.signal);
        emit({
          type: "tool_result",
          payload: { id: call.id, ok: result.ok, summary: result.summary, output: result.output },
        });
        messages.push({ role: "tool", tool_call_id: call.id, content: result.output || result.summary });
        continue;
      }

      const result = await runTool(call.name, args, {
        approved: true,
        signal: controller.signal,
        cwdRoot,
      });
      emit({
        type: "tool_result",
        payload: { id: call.id, ok: result.ok, summary: result.summary, output: result.output },
      });
      // The answer can show where its facts came from.
      for (const citation of result.citations ?? []) {
        emit({ type: "citation", payload: citation });
      }
      messages.push({ role: "tool", tool_call_id: call.id, content: result.output || result.summary });
    }
  }

  emit({ type: "error", payload: { error: "That took too many steps. Try a smaller request." } });
}

/** The done event plus the usage record, from one place. */
function finishRun(
  emit,
  req,
  target,
  { finish, totalPrompt, totalCompletion, firstPrompt, systemPrompt, tools, started, firstTokenAt, toolCallsCount, cancelled },
) {
  if (totalPrompt !== null) {
    emit({
      type: "done",
      payload: {
        finish_reason: finish ?? "stop",
        total_tokens: totalCompletion,
        prompt_tokens: totalPrompt,
        cache_tokens: null,
        first_prompt_tokens: firstPrompt,
        system_tokens: Math.ceil(systemPrompt.length / 4),
        tool_tokens: Math.ceil(JSON.stringify(tools).length / 4),
      },
    });
  } else {
    emit({ type: "done", payload: { finish_reason: finish ?? "stop", total_tokens: totalCompletion } });
  }
  const ms = Math.max(0, Date.now() - started);
  const tokS = ms > 0 && totalCompletion > 0 ? totalCompletion / (ms / 1000) : null;
  recordUsage({
    model: target.model || target.label,
    provider: target.providerId ? target.label : "This computer",
    local: !target.providerId,
    input_tokens: totalPrompt ?? 0,
    output_tokens: totalCompletion,
    ms,
    ttft_ms: firstTokenAt !== null ? firstTokenAt - started : null,
    tok_s: tokS,
    cancelled,
    tool_calls: toolCallsCount,
    source: "chat",
    folder: req.folder ?? null,
    session: req.thread_id ?? null,
  });
}

/** Hands one self-contained job to a helper that works on its own. */
async function runSubagent(target, call, args, emit, signal) {
  const step = (text, tokens) =>
    emit({ type: "subagent_step", payload: { id: call.id, step: text, ...(tokens !== undefined ? { tokens } : {}) } });
  step(`Starting: ${String(args.title ?? "a subtask")}`);
  const result = await complete(target.url, {
    bearer: target.bearer,
    body: buildBody({
      model: target.model,
      messages: [
        {
          role: "system",
          content:
            "You are a read-only helper inside LocalAide. Complete the task you are given and report the result as compact text. You cannot change files or settings; say so if the task needs that.",
        },
        { role: "user", content: String(args.task ?? "") },
      ],
      maxTokens: 2048,
      effort: { kind: "off", level: null },
      tools: [],
    }),
    signal,
  });
  const tokens = (result.usage.prompt ?? 0) + (result.usage.completion ?? 0);
  step("Reporting back", tokens || undefined);
  return {
    ok: true,
    summary: String(args.title ?? "Subagent finished"),
    output: result.content.trim() || "The helper had nothing to report.",
  };
}

/** Titles and one-line helpers, on whatever model is loaded. */
export async function quickAnswer(prompt, { maxTokens = 200, system, target } = {}) {
  const resolved = resolveTarget(target ? { target } : {});
  const result = await complete(resolved.url, {
    bearer: resolved.bearer,
    body: buildBody({
      model: resolved.model,
      messages: [
        ...(system ? [{ role: "system", content: system }] : []),
        { role: "user", content: prompt },
      ],
      maxTokens,
      effort: { kind: "off", level: null },
      tools: [],
    }),
  });
  return result.content.trim();
}

/** One summarizing pass, for the Summarize button. */
export async function summarize(text, maxWords = 256) {
  const resolved = resolveTarget({});
  const result = await complete(resolved.url, {
    bearer: resolved.bearer,
    body: buildBody({
      model: resolved.model,
      messages: [
        { role: "system", content: "You are a concise summarizer. Summarize the following text in a few clear sentences:" },
        { role: "user", content: String(text ?? "") },
      ],
      maxTokens: Math.max(64, Math.min(2048, Math.round(maxWords) * 4)),
      temperature: 0.3,
      effort: { kind: "off", level: null },
      tools: [],
    }),
  });
  return result.content.trim();
}

/** A thread exists before its first message is appended. */
export function ensureThreadFor(threadId) {
  if (!threadId) return;
  ensureThread(threadId);
}

/** Whether a thread has any messages (the resume check). */
export const threadHasMessages = (threadId) => (readThread(threadId)?.messages?.length ?? 0) > 0;
