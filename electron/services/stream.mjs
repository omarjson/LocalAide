// The OpenAI-compatible streaming client: one SSE reader that every
// provider (local llama-server, Ollama, LM Studio, LiteLLM, OpenRouter,
// OpenAI, Groq, anything else) goes through. Ported from openai_stream.rs.

import { AppError } from "../core/errors.mjs";

/** How hard the model thinks. */
export const effortOf = (value) => {
  const v = (value ?? "").trim().toLowerCase();
  if (!v || v === "auto" || v === "default") return { kind: "auto", level: null };
  if (v === "off" || v === "none") return { kind: "off", level: null };
  return { kind: "level", level: v };
};

export const thinkingAllowance = (effort) => {
  if (effort.kind === "off") return 0;
  if (effort.kind === "auto") return 4096;
  return { minimal: 1024, low: 2048, medium: 4096, high: 8192 }[effort.level] ?? 12288;
};

/** The request body for one round. */
export function buildBody({ model, messages, maxTokens, temperature, tools, effort, usage, forbidTools, stream = true }) {
  const body = { model: model || undefined, messages, stream };
  if (Number.isFinite(maxTokens) && maxTokens > 0) body.max_tokens = Math.round(maxTokens);
  if (Number.isFinite(temperature)) body.temperature = temperature;
  if (tools?.length && !forbidTools) body.tools = tools;
  if (effort.kind === "level") body.reasoning_effort = effort.level;
  if (effort.kind === "off") body.reasoning_effort = "none";
  if (usage) body.stream_options = { include_usage: true };
  for (const key of Object.keys(body)) if (body[key] === undefined) delete body[key];
  return body;
}

/**
 * Reads one SSE response and yields deltas as they arrive.
 * Handles: data: {...} frames, [DONE], a clean close without [DONE], and
 * providers that send a whole JSON object instead of a stream.
 */
export async function* readStream(response, { signal, requireDone = false } = {}) {
  if (!response.body) throw AppError.internal("The provider sent no answer body.");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let sawDone = false;

  try {
    while (true) {
      if (signal?.aborted) return;
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });

      let index;
      while ((index = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, index).trim();
        buffer = buffer.slice(index + 1);
        if (!line || line.startsWith(":")) continue;
        if (line === "data: [DONE]") {
          sawDone = true;
          yield { kind: "done" };
          return;
        }
        if (!line.startsWith("data:")) continue;
        const payload = line.slice(5).trim();
        if (!payload) continue;
        let json;
        try {
          json = JSON.parse(payload);
        } catch {
          continue;
        }
        yield* framesOf(json);
      }
    }
    if (buffer.trim().startsWith("data:")) {
      const payload = buffer.trim().slice(5).trim();
      if (payload === "[DONE]") sawDone = true;
      else {
        try {
          yield* framesOf(JSON.parse(payload));
        } catch {
          /* a partial last frame is dropped, like the Rust client */
        }
      }
    }
  } finally {
    reader.cancel().catch(() => {});
  }

  yield { kind: "end", sawDone, requireDone };
}

/** Turns one wire chunk into deltas. */
function* framesOf(json) {
  if (json?.error) {
    yield { kind: "error", message: String(json.error.message ?? json.error) };
    return;
  }
  const choice = json?.choices?.[0];
  const delta = choice?.delta ?? choice?.message ?? {};
  if (typeof delta.content === "string" && delta.content) {
    yield { kind: "content", text: delta.content };
  }
  // DeepSeek, vLLM and others put thinking in reasoning_content.
  const reasoning = delta.reasoning_content ?? delta.reasoning;
  if (typeof reasoning === "string" && reasoning) {
    yield { kind: "reasoning", text: reasoning };
  }
  for (const call of delta.tool_calls ?? []) {
    yield {
      kind: "tool",
      index: call.index ?? 0,
      id: call.id ?? null,
      name: call.function?.name ?? null,
      arguments: call.function?.arguments ?? "",
    };
  }
  if (choice?.finish_reason) yield { kind: "finish", reason: choice.finish_reason };
  if (json?.usage) {
    yield {
      kind: "usage",
      prompt: json.usage.prompt_tokens ?? null,
      completion: json.usage.completion_tokens ?? null,
      cached: json.usage.prompt_tokens_details?.cached_tokens ?? json.usage.cache_read_input_tokens ?? null,
    };
  }
}

/** Collects tool-call fragments into complete calls. */
export class ToolCallAccumulator {
  constructor() {
    this.calls = new Map();
  }
  push(frame) {
    const current = this.calls.get(frame.index) ?? { id: null, name: "", arguments: "" };
    if (frame.id) current.id = frame.id;
    if (frame.name) current.name = frame.name;
    current.arguments += frame.arguments ?? "";
    this.calls.set(frame.index, current);
  }
  list() {
    return [...this.calls.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([, call], i) => ({
        id: call.id || `call_${i}`,
        name: call.name,
        arguments: call.arguments || "{}",
      }))
      .filter((call) => call.name);
  }
}

/** One non-streamed request, for titles, summaries and chores. */
export async function complete(url, { bearer, body, signal, timeoutMs = 120_000 }) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const onAbort = () => controller.abort();
  signal?.addEventListener("abort", onAbort, { once: true });
  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        ...(bearer ? { Authorization: `Bearer ${bearer}` } : {}),
      },
      body: JSON.stringify({ ...body, stream: false }),
      signal: controller.signal,
    });
    if (!response.ok) {
      const text = await response.text().catch(() => "");
      throw new AppError("PROVIDER_ERROR", `The provider answered ${response.status}.`, {
        detail: text.slice(0, 500),
        retryable: response.status >= 500 || response.status === 429,
      });
    }
    const json = await response.json();
    const message = json?.choices?.[0]?.message ?? {};
    return {
      content: String(message.content ?? ""),
      reasoning: String(message.reasoning_content ?? message.reasoning ?? ""),
      toolCalls: (message.tool_calls ?? []).map((call, i) => ({
        id: call.id ?? `call_${i}`,
        name: call.function?.name ?? "",
        arguments: call.function?.arguments ?? "{}",
      })),
      finishReason: json?.choices?.[0]?.finish_reason ?? null,
      usage: {
        prompt: json?.usage?.prompt_tokens ?? null,
        completion: json?.usage?.completion_tokens ?? null,
        cached: json?.usage?.prompt_tokens_details?.cached_tokens ?? null,
      },
    };
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", onAbort);
  }
}

