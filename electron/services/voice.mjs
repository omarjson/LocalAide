// Voice input: transcription models, local and online. Local models run in
// the sherpa-onnx engine on this computer and never use the network. Online
// models use the user's own key; "Local only" in Settings blocks them
// entirely, and every online call lands in the cloud audit (what left this
// computer). Ported from services/voice/{mod,local,online,live}.rs.

import crypto from "node:crypto";
import os from "node:os";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { appDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { loadConfig } from "../core/config.mjs";
import { auditCloud } from "./cloud.mjs";
import { getEntry, isSafeModelId, allEntries } from "./models.mjs";
import { sherpaBinary, ensureSherpa, sherpaStatus } from "./download.mjs";
import { localEndpoint } from "./engine.mjs";
import { complete, buildBody } from "./stream.mjs";

// ---- The WAV the window records ----------------------------------------

/** Seconds of audio in a 16 kHz mono 16-bit recording of this many bytes. */
export const pcmSeconds = (bytes) => bytes / 32_000;

/**
 * Check a WAV recording from the window: RIFF/WAVE, 16-bit PCM, mono,
 * 16 kHz, at most 15 minutes. Returns the PCM samples' byte range.
 */
export function checkWav(wav) {
  if (wav.length < 44 || wav.subarray(0, 4).toString("latin1") !== "RIFF" || wav.subarray(8, 12).toString("latin1") !== "WAVE") {
    throw AppError.invalidInput("The recording is not a WAV file.");
  }
  let pos = 12;
  let fmtOk = false;
  while (pos + 8 <= wav.length) {
    const id = wav.subarray(pos, pos + 4).toString("latin1");
    const len = wav.readUInt32LE(pos + 4);
    const body = pos + 8;
    if (id === "fmt " && body + 16 <= wav.length) {
      const format = wav.readUInt16LE(body);
      const channels = wav.readUInt16LE(body + 2);
      const rate = wav.readUInt32LE(body + 4);
      const bits = wav.readUInt16LE(body + 14);
      if (format !== 1 || channels !== 1 || rate !== 16_000 || bits !== 16) {
        throw AppError.invalidInput("The recording must be 16 kHz mono 16-bit PCM.");
      }
      fmtOk = true;
    } else if (id === "data") {
      if (!fmtOk) throw AppError.invalidInput("The recording has no format block.");
      const end = Math.min(body + len, wav.length);
      if (end - body > 16_000 * 2 * 60 * 15) {
        throw AppError.invalidInput("Recordings can be up to 15 minutes long.");
      }
      return [body, end];
    }
    pos = body + len + (len & 1);
  }
  throw AppError.invalidInput("The recording has no audio.");
}

// ---- The online provider table ------------------------------------------

/** Model ids checked against each provider's docs in November 2026. */
export const ONLINE = [
  {
    id: "gemini",
    label: "Google Gemini",
    key_url: "https://aistudio.google.com/apikey",
    host: "generativelanguage.googleapis.com",
    models: [
      { id: "gemini-3.5-transcribe-live", label: "Gemini 3.5 Transcribe Live", delivery: "live", note: "Streams while you speak; finds the language, Arabic included." },
      { id: "gemini-3.5-transcribe", label: "Gemini 3.5 Transcribe", delivery: "batch", note: "One request for the whole recording." },
    ],
  },
  {
    id: "deepgram",
    label: "Deepgram",
    key_url: "https://console.deepgram.com/",
    host: "api.deepgram.com",
    models: [
      { id: "nova-3", label: "Nova-3 (live)", delivery: "live", note: "Streams while you speak." },
      { id: "nova-3", label: "Nova-3", delivery: "batch", note: "One request for the whole recording." },
    ],
  },
  {
    id: "groq",
    label: "Groq",
    key_url: "https://console.groq.com/keys",
    host: "api.groq.com",
    models: [{ id: "whisper-large-v3-turbo", label: "Whisper large-v3-turbo", delivery: "batch", note: "Very fast. About 100 languages." }],
  },
  {
    id: "openai",
    label: "OpenAI",
    key_url: "https://platform.openai.com/api-keys",
    host: "api.openai.com",
    models: [
      { id: "gpt-transcribe", label: "GPT-Transcribe", delivery: "batch", note: "OpenAI's current model for recordings." },
      { id: "gpt-4o-transcribe", label: "GPT-4o Transcribe", delivery: "batch", note: "Retires on 26 February 2027." },
      { id: "whisper-1", label: "Whisper", delivery: "batch", note: "Retires on 26 February 2027." },
    ],
  },
  {
    id: "soniox",
    label: "Soniox",
    key_url: "https://console.soniox.com/",
    host: "stt-rt.soniox.com",
    models: [{ id: "stt-rt-v6", label: "Soniox v6 (live)", delivery: "live", note: "Streams while you speak; 60+ languages." }],
  },
];

/** A model choice as stored in settings: a local catalog id, or
 *  "online:<provider>:<model>:<live|batch>". */
export function parseChoice(value) {
  if (typeof value !== "string") return null;
  if (value.startsWith("online:")) {
    const [provider, model, delivery] = value.slice(7).split(":");
    const known = ONLINE.some(
      (p) => p.id === provider && p.models.some((m) => m.id === model && m.delivery === delivery),
    );
    return known ? { online: true, provider, model, delivery } : null;
  }
  if (value.startsWith("stt-") && isSafeModelId(value) && getEntry(value)?.kind === "stt") {
    return { online: false, id: value };
  }
  return null;
}

export const choiceKey = (provider, model, delivery) => `online:${provider}:${model}:${delivery}`;

// ---- Local transcription (sherpa-onnx) ----------------------------------

/** Which engine flags a model needs, from its catalog id. */
function family(id) {
  if (id.startsWith("stt-whisper-")) return "whisper";
  if (id === "stt-moonshine-base-ar") return "moonshine-v2";
  if (id.startsWith("stt-moonshine-")) return "moonshine";
  if (id.startsWith("stt-parakeet-")) return "transducer";
  return null;
}

/** The engine arguments for a model whose files sit flat in models/. */
function modelArgs(id, modelsDir) {
  const f = (part) => path.join(modelsDir, `${id}.${part}`);
  const tokens = `--tokens=${f("tokens.txt")}`;
  switch (family(id)) {
    case "whisper":
      return [`--whisper-encoder=${f("encoder.onnx")}`, `--whisper-decoder=${f("decoder.onnx")}`, tokens, "--model-type=whisper"];
    case "moonshine":
      return [`--moonshine-preprocessor=${f("preprocess.onnx")}`, `--moonshine-encoder=${f("encoder.onnx")}`, `--moonshine-uncached-decoder=${f("uncached-decoder.onnx")}`, `--moonshine-cached-decoder=${f("cached-decoder.onnx")}`, tokens];
    case "moonshine-v2":
      return [`--moonshine-encoder=${f("encoder.ort")}`, `--moonshine-merged-decoder=${f("merged-decoder.ort")}`, tokens];
    case "transducer":
      return [`--encoder=${f("encoder.onnx")}`, `--decoder=${f("decoder.onnx")}`, `--joiner=${f("joiner.onnx")}`, tokens, "--model-type=nemo_transducer"];
    default:
      return null;
  }
}

/** The text from the engine's output: the last JSON line with a "text" field. */
function parseOutput(stdout) {
  for (const line of stdout.split("\n").reverse()) {
    try {
      const value = JSON.parse(line.trim());
      const text = value?.text;
      if (typeof text === "string") {
        const lang = typeof value?.lang === "string" && value.lang ? value.lang : null;
        return [text.trim(), lang];
      }
    } catch {
      /* not a JSON line */
    }
  }
  return null;
}

function tempWav() {
  const dir = path.join(appDir(), "voice");
  fs.mkdirSync(dir, { recursive: true });
  return path.join(dir, `rec-${crypto.randomBytes(8).toString("hex")}.wav`);
}

function sherpaEnv(binary) {
  // The engine's shared libraries live in ../lib.
  const root = path.dirname(path.dirname(binary));
  const lib = path.join(root, "lib");
  if (process.platform === "linux") return { LD_LIBRARY_PATH: lib };
  if (process.platform === "darwin") return { DYLD_LIBRARY_PATH: lib };
  return {};
}

/** Transcribe a 16 kHz mono WAV with a downloaded model. No network. */
export async function transcribeLocal(id, wav) {
  const binary = sherpaBinary();
  if (!binary) {
    throw new AppError("VOICE_ENGINE_MISSING", "The voice engine is not downloaded yet. Get it in Settings → Voice.");
  }
  const args = modelArgs(id, path.join(appDir(), "models"));
  if (!args) throw AppError.invalidInput(`${id} is not a voice model.`);
  const file = tempWav();
  fs.writeFileSync(file, wav);
  const threads = Math.max(1, Math.min(4, os.availableParallelism?.() ?? 2));
  const started = Date.now();
  try {
    const result = await new Promise((resolve, reject) => {
      const child = spawn(binary, [`--num-threads=${threads}`, ...args, file], {
        env: { ...process.env, ...sherpaEnv(binary) },
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      });
      let out = "";
      child.stdout.on("data", (c) => (out += c));
      child.stderr.on("data", (c) => (out += c));
      const deadline = setTimeout(() => {
        child.kill("SIGKILL");
        reject(AppError.internal("Transcription took longer than 3 minutes and was stopped."));
      }, 180_000);
      child.on("close", () => {
        clearTimeout(deadline);
        resolve(out);
      });
      child.on("error", (e) => {
        clearTimeout(deadline);
        reject(AppError.internal(`The voice engine did not start: ${e}`));
      });
    });
    const parsed = parseOutput(result);
    if (!parsed) {
      const tail = result.split("\n").slice(-3).join(" ").trim();
      throw AppError.internal(`The voice engine gave no text. ${tail}`);
    }
    return { text: parsed[0], language: parsed[1], ms: Date.now() - started };
  } finally {
    fs.rmSync(file, { force: true });
  }
}



// ---- Online batch transcription ------------------------------------------

const failed = (label, status, body) =>
  AppError.internal(`${label} answered with an error (${status}). ${String(body).slice(0, 200)}`.trim());

const voiceKey = (provider) => String(loadConfig().voice?.keys?.[provider] ?? "").trim();

/** One provider's batch transcription, on the user's explicit recording. */
export async function transcribeOnline(provider, model, wav, seconds) {
  const key = voiceKey(provider);
  if (!key) {
    throw AppError.invalidInput("Add your key for this provider in Settings → Voice first.");
  }
  const what = `Recorded audio, ${seconds.toFixed(1)} s`;
  if (provider === "groq" || provider === "openai") {
    const url = provider === "groq"
      ? "https://api.groq.com/openai/v1/audio/transcriptions"
      : "https://api.openai.com/v1/audio/transcriptions";
    const label = provider === "groq" ? "Groq" : "OpenAI";
    const form = new FormData();
    form.append("model", model);
    form.append("response_format", "json");
    form.append("file", new Blob([wav], { type: "audio/wav" }), "speech.wav");
    return requestJson(label, url, what, model, { Authorization: `Bearer ${key}` }, form, (body) => String(body?.text ?? "").trim());
  }
  if (provider === "deepgram") {
    const url = `https://api.deepgram.com/v1/listen?model=${model}&smart_format=true&language=multi`;
    return requestJson(
      "Deepgram",
      url,
      what,
      model,
      { Authorization: `Token ${key}`, "Content-Type": "audio/wav" },
      wav,
      (body) => String(body?.results?.channels?.[0]?.alternatives?.[0]?.transcript ?? "").trim(),
    );
  }
  if (provider === "gemini") {
    const url = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent`;
    const body = JSON.stringify({
      contents: [{ role: "user", parts: [{ inline_data: { mime_type: "audio/wav", data: wav.toString("base64") } }] }],
    });
    return requestJson("Google Gemini", url, what, model, { "x-goog-api-key": key }, body, geminiText);
  }
  throw AppError.invalidInput("That voice provider is not supported.");
}

async function requestJson(label, url, what, model, headers, body, readText) {
  const started = Date.now();
  let response;
  try {
    response = await fetch(url, { method: "POST", headers, body, signal: AbortSignal.timeout(120_000) });
  } catch (e) {
    auditCloud(label, url, model, "voice", what, bodyLength(body), false);
    throw AppError.internal(`Could not reach ${label}: ${e}`);
  }
  const text = await response.text();
  auditCloud(label, url, model, "voice", what, bodyLength(body), response.ok);
  if (!response.ok) throw failed(label, response.status, text);
  let json;
  try {
    json = JSON.parse(text);
  } catch {
    throw AppError.internal(`${label} sent an answer LocalAide could not read.`);
  }
  void started;
  return readText(json);
}

const bodyLength = (body) => (typeof body === "string" ? body.length : body.byteLength ?? body.size ?? 0);

/** The transcript in a generateContent reply: every text part, joined. */
function geminiText(v) {
  const parts = v?.candidates?.[0]?.content?.parts;
  return Array.isArray(parts)
    ? parts.filter((p) => typeof p?.text === "string").map((p) => p.text).join("").trim()
    : "";
}

// ---- Live transcription (WebSocket) --------------------------------------

/** LiveEvent shapes: open, interim, final, done, error. */

const liveSessions = new Map();

function livePlan(provider, model, key) {
  switch (provider) {
    case "deepgram":
      return {
        url: `wss://api.deepgram.com/v1/listen?model=${model}&encoding=linear16&sample_rate=16000&channels=1&interim_results=true&smart_format=true&language=multi`,
        headers: { Authorization: `Token ${key}` },
        hello: null,
        label: "Deepgram",
      };
    case "soniox":
      return {
        url: "wss://stt-rt.soniox.com/transcribe-websocket",
        headers: { Authorization: `Bearer ${key}` },
        hello: JSON.stringify({
          api_key: key,
          model,
          audio_format: "pcm_s16le",
          sample_rate: 16000,
          num_channels: 1,
          language_hints: ["en", "ar"],
        }),
        label: "Soniox",
      };
    case "gemini":
      return {
        url: "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent",
        headers: { "x-goog-api-key": key },
        hello: JSON.stringify({
          setup: {
            model: `models/${model}`,
            generationConfig: { responseModalities: ["TEXT"] },
            inputAudioTranscription: {},
          },
        }),
        label: "Google Gemini",
      };
    default:
      throw AppError.invalidInput("That provider has no live transcription.");
  }
}

/** Text from one provider message: (new final text, interim text, flags). */
export function parseLive(provider, v) {
  const out = { finalText: "", interim: null, ready: false, finished: false, error: null };
  if (provider === "deepgram") {
    if (v?.type === "Results") {
      const t = v?.channel?.alternatives?.[0]?.transcript ?? "";
      if (v.is_final === true) out.finalText = t;
      else out.interim = t;
    } else if (v?.type === "Metadata") {
      out.finished = true;
    } else {
      out.error = v?.err_msg ?? v?.description ?? null;
    }
  } else if (provider === "soniox") {
    out.error = v?.error_message ?? null;
    let interim = "";
    for (const token of v?.tokens ?? []) {
      const t = String(token?.text ?? "");
      if (t.startsWith("<") && t.endsWith(">")) continue; // markers such as <end>
      if (token?.is_final === true) out.finalText += t;
      else interim += t;
    }
    if (interim) out.interim = interim;
    out.finished = v?.finished === true;
  } else if (provider === "gemini") {
    if (v?.setupComplete !== undefined) out.ready = true;
    if (v?.serverContent) {
      const finalText = v.serverContent?.inputTranscription?.text;
      if (typeof finalText === "string") out.finalText = finalText;
      const interim = v.serverContent?.interimInputTranscription?.text;
      if (typeof interim === "string") out.interim = interim;
    }
    out.error = v?.error?.message ?? null;
  }
  return out;
}

const join = (acc, piece) => {
  const trimmed = piece.trim();
  if (!acc) return trimmed;
  if (!trimmed) return acc;
  return `${acc.trimEnd()} ${trimmed}`;
};

const sendLiveAudio = (provider, ws, pcm) => {
  if (provider === "gemini") {
    ws.send(JSON.stringify({ realtimeInput: { audio: { data: pcm.toString("base64"), mimeType: "audio/pcm;rate=16000" } } }));
  } else {
    ws.send(pcm);
  }
};

const sendLiveEnd = (provider, ws) => {
  if (provider === "deepgram") ws.send(JSON.stringify({ type: "CloseStream" }));
  else if (provider === "soniox") ws.send(new Uint8Array(0));
  else ws.send(JSON.stringify({ realtimeInput: { audioStreamEnd: true } }));
};

/** Starts one live session; events ride the channel. Returns its id. */
export function liveStart(provider, model, channelId, send) {
  const key = voiceKey(provider);
  if (!key) {
    throw AppError.invalidInput("Add your key for this provider in Settings → Voice first.");
  }
  const plan = livePlan(provider, model, key);
  const id = crypto.randomBytes(8).toString("hex");
  liveSessions.set(id, null); // reserve the id so push() finds it

  void (async () => {
    let ws;
    try {
      ws = new WebSocket(plan.url, { headers: plan.headers });
    } catch (e) {
      liveSessions.delete(id);
      send({ type: "error", message: `Could not reach ${plan.label}: ${e}` });
      return;
    }
    liveSessions.set(id, ws);
    let ready = provider !== "gemini";
    let waiting = [];
    let finals = "";
    let bytes = 0;
    let stopAt = null;
    let finished = false;
    let failure = null;
    const started = Date.now();
    const lastMessage = { at: Date.now() };

    const handle = (raw) => {
      let v;
      try {
        v = JSON.parse(raw);
      } catch {
        return;
      }
      lastMessage.at = Date.now();
      const p = parseLive(provider, v);
      if (p.error) {
        failure = p.error;
        return;
      }
      if (p.ready && !ready) {
        ready = true;
        send({ type: "open" });
        for (const pcm of waiting) sendLiveAudio(provider, ws, pcm);
        waiting = [];
      }
      if (p.finalText.trim()) {
        finals = provider === "soniox" ? `${finals}${p.finalText}` : join(finals, p.finalText);
        send({ type: "final", text: finals.trim() });
      }
      if (p.interim !== null) {
        send({ type: "interim", text: join(finals, p.interim) });
      }
      if (p.finished) finished = true;
    };

    ws.onopen = () => {
      if (plan.hello) ws.send(plan.hello);
      if (ready) send({ type: "open" });
    };
    ws.onmessage = (event) => {
      const text = typeof event.data === "string" ? event.data : Buffer.from(event.data).toString("utf8");
      handle(text);
    };
    ws.onerror = () => {
      if (!stopAt) failure = failure ?? `${plan.label} refused the connection. Check the key.`;
    };
    ws.onclose = (event) => {
      if (!finals && !stopAt && event?.reason) {
        failure = failure ?? `${plan.label} closed the connection: ${event.reason}`;
      }
    };

    // Stop ends the session: flush, tell the provider, wait briefly for the
    // last words, then wrap up — exactly the Rust timing.
    const finish = () => {
      if (!stopAt) {
        for (const pcm of waiting) sendLiveAudio(provider, ws, pcm);
        waiting = [];
        sendLiveEnd(provider, ws);
        stopAt = Date.now();
      }
    };
    liveSessions.set(id, { ws, finish });

    const poll = setInterval(() => {
      if (stopAt && (finished || Date.now() - stopAt > 6000 || (Date.now() - stopAt > 1500 && Date.now() - lastMessage.at > 1500))) {
        end();
        return;
      }
      if (Date.now() - started > 15 * 60_000) end();
      if (ws.readyState === 3 && !stopAt) end(); // closed mid-session
    }, 150);

    const end = () => {
      clearInterval(poll);
      liveSessions.delete(id);
      try {
        ws.close();
      } catch {
        /* already closed */
      }
      const secs = pcmSeconds(bytes);
      if (failure) {
        auditCloud(plan.label, plan.url, model, "voice (live)", "Live microphone audio", 0, false);
        send({ type: "error", message: failure });
      } else {
        auditCloud(plan.label, plan.url, model, "voice (live)", "Live microphone audio", 0, true);
        void secs;
        send({ type: "done", text: finals.trim() });
      }
    };
  })();

  return id;
}

/** Push a piece of 16 kHz mono PCM (at most 5 seconds) to a live session. */
export function livePush(id, pcm) {
  const session = liveSessions.get(id);
  if (!session || !session.ws) return false;
  if (session.ws.readyState !== 1) return true; // queued until open
  session.ws.send(pcm);
  return true;
}

/** Stop a live session: the final transcript arrives as a done event. */
export function liveStop(id) {
  const session = liveSessions.get(id);
  if (!session) return false;
  session.finish?.() ?? session.ws?.close();
  return true;
}

// ---- The commands' bodies ------------------------------------------------

/** Transcribe a finished recording with the chosen (local or batch) model. */
export async function transcribe(audioBase64) {
  const wav = Buffer.from(String(audioBase64 ?? "").trim(), "base64");
  const [start, end] = checkWav(wav);
  const pcm = wav.subarray(start, end);
  const seconds = pcmSeconds(pcm.length);
  if (seconds < 0.25) {
    throw AppError.invalidInput("The recording was too short. Hold the button while you speak.");
  }
  const cfg = loadConfig();
  const choice = parseChoice(cfg.voice?.model);
  if (!choice) {
    throw AppError.invalidInput("Choose a voice model in Settings → Voice.");
  }
  const started = Date.now();
  if (!choice.online) {
    const result = await transcribeLocal(choice.id, wav);
    return { text: result.text, model: choice.id, ms: result.ms, online: false, language: result.language };
  }
  if (cfg.voice?.local_only) {
    throw AppError.invalidInput("Local only is on, so online voice models are blocked.");
  }
  if (choice.delivery === "live") {
    throw AppError.invalidInput("This is a live model; it transcribes while you speak.");
  }
  const text = await transcribeOnline(choice.provider, choice.model, pcm, seconds);
  return { text, model: `${choice.provider}:${choice.model}`, ms: Date.now() - started, online: true, language: null };
}

/** Tidy dictated text with the loaded local model; skip when none runs. */
export async function cleanup(text) {
  const original = String(text ?? "").slice(0, 8000);
  if (!original.trim()) return { text: original, changed: false, skipped: null };
  const cfg = loadConfig();
  const target = cfg.cloud_llm_enabled ? null : localEndpoint();
  if (!target) {
    return { text: original, changed: false, skipped: "No model is loaded on this computer, so the text was not tidied." };
  }
  const result = await complete(target.url, {
    bearer: target.bearer,
    body: buildBody({
      model: target.model,
      messages: [
        {
          role: "system",
          content:
            "You tidy dictated text. Remove filler words (um, uh, er, like, you know), stutters and repeated words; fix punctuation and capital letters. Keep the words, meaning and language. Do not answer, translate or add anything. Reply with the tidied text only.",
        },
        { role: "user", content: original },
      ],
      maxTokens: Math.min(2048, Math.round(original.length / 3) + 64),
      temperature: 0.1,
      effort: { kind: "off", level: null },
      tools: [],
    }),
    timeoutMs: 45_000,
  });
  const cleaned = result.content.trim();
  if (!cleaned) return { text: original, changed: false, skipped: null };
  return { text: cleaned, changed: cleaned !== original, skipped: null };
}


/** The Settings → Voice model list, with installed flags and key hints. */
export function voiceModels() {
  const cfg = loadConfig();
  const local = allEntries()
    .filter((entry) => entry.kind === "stt")
    .map((entry) => {
      const file = path.join(appDir(), "models", `${entry.id}.gguf`);
      const primary = fs.existsSync(file)
        ? file
        : path.join(appDir(), "models", entry.filename ?? "");
      return {
        id: entry.id,
        name: entry.name,
        params: entry.quant || "",
        description: entry.description ?? "",
        license: entry.license ?? "",
        size_bytes: (entry.companions ?? []).reduce((n, c) => n + (c.file_size ?? 0), entry.file_size ?? 0),
        installed: fs.existsSync(primary) || fs.existsSync(path.join(appDir(), "models", `${entry.id}.encoder.onnx`)),
      };
    })
    .sort((a, b) => a.size_bytes - b.size_bytes);
  const online = ONLINE.flatMap((provider) =>
    provider.models.map((model) => ({
      key: choiceKey(provider.id, model.id, model.delivery),
      provider: provider.id,
      provider_label: provider.label,
      host: provider.host,
      key_url: provider.key_url,
      model: model.id,
      label: model.label,
      delivery: model.delivery,
      note: model.note,
      has_key: Boolean(voiceKey(provider.id)),
    })),
  );
  return { local, online, engine: sherpaStatus() };
}

/** Downloads the speech engine for this computer. */
export async function engineFetch(onProgress) {
  await ensureSherpa({ onProgress });
  return sherpaStatus();
}
