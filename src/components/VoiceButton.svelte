<script lang="ts">
  // Push to talk: hold to record, release to transcribe. Live models show
  // text while you speak; local and batch models transcribe on release.
  import { onDestroy } from "svelte";
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { Recorder, bytesToBase64 } from "../lib/audio";
  import { dur, motion } from "../lib/motion";
  import { deliveryOf, voice } from "../lib/voice.svelte";

  interface Props {
    /** The transcript, after cleanup when it is on. */
    onText: (text: string, info: { online: boolean; model: string }) => void;
    disabled?: boolean;
    /** Where the recording panel opens. */
    placement?: "above" | "below";
  }
  let { onText, disabled = false, placement = "above" }: Props = $props();

  type Phase = "idle" | "starting" | "recording" | "transcribing" | "tidying";
  let phase = $state<Phase>("idle");
  let levels = $state<number[]>(Array(32).fill(0));
  let started = 0;
  let elapsed = $state(0);
  let interim = $state("");
  let error = $state<string | null>(null);
  let recorder: Recorder | null = null;
  let session: string | null = null;
  let liveDone: ((text: string) => void) | null = null;
  let liveText = "";
  let tick: ReturnType<typeof setInterval> | null = null;
  let errorTimer: ReturnType<typeof setTimeout> | null = null;

  const model = $derived(voice.settings?.model ?? "stt-whisper-tiny");
  const delivery = $derived(deliveryOf(model));

  const fail = (message: string) => {
    error = message;
    phase = "idle";
    if (errorTimer) clearTimeout(errorTimer);
    errorTimer = setTimeout(() => (error = null), 6000);
  };

  const pushLevel = (v: number) => {
    levels = [...levels.slice(1), v];
  };

  const begin = async () => {
    if (disabled || phase !== "idle") return;
    error = null;
    interim = "";
    liveText = "";
    phase = "starting";
    try {
      recorder = new Recorder();
      if (delivery === "live") {
        session = await api.voiceLiveStart((e) => {
          if (e.type === "interim" || e.type === "final") interim = e.text;
          if (e.type === "final") liveText = e.text;
          if (e.type === "done") liveDone?.(e.text || liveText);
          if (e.type === "error") {
            liveDone?.("");
            fail(e.message);
          }
        });
        const s = session;
        await recorder.start(pushLevel, (pcm) => void api.voiceLivePush(s, bytesToBase64(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength))).catch(() => {}));
      } else {
        await recorder.start(pushLevel);
      }
      started = performance.now();
      elapsed = 0;
      tick = setInterval(() => (elapsed = (performance.now() - started) / 1000), 100);
      phase = "recording";
    } catch (e: unknown) {
      recorder = null;
      const name = (e as { name?: string })?.name;
      fail(name === "NotAllowedError" ? "LocalAide may not use the microphone. Allow it in your system settings, then try again." : name === "NotFoundError" ? "No microphone was found." : errorMessage(e));
      if (session) void api.voiceLiveStop(session);
      session = null;
    }
  };

  const end = async () => {
    if (phase === "starting") {
      // Released before the microphone opened: stop as soon as it is ready.
      const wait = setInterval(() => {
        if (phase !== "starting") {
          clearInterval(wait);
          if (phase === "recording") void end();
        }
      }, 50);
      return;
    }
    if (phase !== "recording" || !recorder) return;
    if (tick) clearInterval(tick);
    const r = recorder;
    recorder = null;
    phase = "transcribing";
    try {
      let text = "";
      let online = delivery !== "local";
      if (delivery === "live" && session) {
        const s = session;
        session = null;
        const done = new Promise<string>((resolve) => {
          liveDone = resolve;
          setTimeout(() => resolve(liveText), 8000);
        });
        await r.stop((pcm) => void api.voiceLivePush(s, bytesToBase64(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength))).catch(() => {}));
        await api.voiceLiveStop(s);
        text = (await done).trim();
        liveDone = null;
      } else {
        const rec = await r.stop();
        if (rec.seconds < 0.3) {
          fail("That was too short. Hold the button while you speak.");
          return;
        }
        const t = await api.transcribe(bytesToBase64(rec.wav));
        text = t.text.trim();
        online = t.online;
      }
      if (!text) {
        fail("No speech was heard. Try again a little closer to the microphone.");
        return;
      }
      if (voice.settings?.cleanup) {
        phase = "tidying";
        const c = await api.voiceCleanup(text);
        text = c.text;
      }
      phase = "idle";
      onText(text, { online, model });
    } catch (e: unknown) {
      fail(errorMessage(e));
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if ((e.key === " " || e.key === "Enter") && !e.repeat) {
      e.preventDefault();
      void begin();
    }
  };
  const onKeyUp = (e: KeyboardEvent) => {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      void end();
    }
  };

  onDestroy(() => {
    if (tick) clearInterval(tick);
    recorder?.cancel();
    if (session) void api.voiceLiveStop(session);
  });

  const busy = $derived(phase === "transcribing" || phase === "tidying");
  const label = $derived(
    phase === "recording" ? "Release to send" : phase === "transcribing" ? "Transcribing…" : phase === "tidying" ? "Tidying the text…" : phase === "starting" ? "Opening the microphone…" : "Hold to talk",
  );
</script>

<div class="relative">
  <button
    type="button"
    class={`mic ${phase === "recording" ? "mic-on" : ""}`}
    aria-label="Hold to talk"
    aria-pressed={phase === "recording"}
    title={delivery === "local" ? "Hold to talk (transcribed on this computer)" : "Hold to talk (sent to an online model)"}
    disabled={disabled || busy}
    onpointerdown={(e) => { (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); void begin(); }}
    onpointerup={() => void end()}
    onpointercancel={() => void end()}
    onkeydown={onKeyDown}
    onkeyup={onKeyUp}
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#if busy}<span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" aria-hidden="true"></span>{:else}<Icon name="mic" class="h-4 w-4" />{/if}
  </button>

  {#if phase !== "idle" || error}
    <div class={`panel ${placement === "above" ? "bottom-full mb-2" : "top-full mt-2"}`} role="status" aria-live="polite" in:fly={{ y: placement === "above" ? 6 : -6, duration: dur(motion.base), easing: motion.enter }}>
      {#if error}
        <p class="flex items-start gap-2 text-sm text-rose-300"><Icon name="alert" class="mt-0.5 h-4 w-4 shrink-0" />{error}</p>
      {:else}
        <div class="flex items-center gap-3">
          <span class={`dot ${phase === "recording" ? "dot-on" : ""}`} aria-hidden="true"></span>
          <div class="wave" aria-hidden="true">
            {#each levels as l, i (i)}<span style={`height:${Math.max(8, Math.round(l * 100))}%`}></span>{/each}
          </div>
          <span class="w-12 text-right font-mono text-xs tabular-nums text-ink-300">{elapsed.toFixed(1)} s</span>
        </div>
        <div class="mt-2 flex items-center justify-between gap-3 text-xs">
          <span class="font-medium text-ink-100">{label}</span>
          <span class="text-ink-500">{delivery === "local" ? "On this computer" : delivery === "live" ? "Live · online" : "Online after you release"}</span>
        </div>
        {#if delivery === "live" && interim}
          <p class="mt-2 max-h-24 overflow-y-auto text-sm leading-6 text-ink-200" dir="auto">{interim}</p>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .mic {
    display: grid;
    height: 2rem;
    width: 2rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    touch-action: none;
    user-select: none;
    transition: background-color 150ms ease-out, color 150ms ease-out, transform 120ms ease-out;
  }
  .mic:hover:not(:disabled) {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .mic:disabled {
    opacity: 0.5;
  }
  .mic-on {
    background: var(--color-ember-600);
    color: var(--color-on-ember);
    transform: scale(1.06);
  }
  .mic-on:hover:not(:disabled) {
    background: var(--color-ember-500);
    color: var(--color-on-ember);
  }
  .panel {
    position: absolute;
    left: 0;
    z-index: 40;
    width: min(22rem, 80vw);
    border-radius: 0.75rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-950);
    padding: 0.7rem 0.8rem;
    box-shadow: 0 18px 40px -16px rgb(0 0 0 / 0.55);
  }
  .wave {
    display: flex;
    flex: 1;
    height: 28px;
    align-items: center;
    gap: 2px;
  }
  .wave span {
    flex: 1;
    min-height: 2px;
    border-radius: 9999px;
    background: var(--color-ember-500);
    opacity: 0.85;
    transition: height 60ms linear;
  }
  .dot {
    height: 8px;
    width: 8px;
    flex-shrink: 0;
    border-radius: 9999px;
    background: var(--color-ink-500);
  }
  .dot-on {
    background: #ef4444;
    animation: blink 1s steps(2, start) infinite;
  }
  @keyframes blink {
    to { visibility: hidden; }
  }
  :global([data-motion="off"]) .dot-on {
    animation: none;
  }
</style>
