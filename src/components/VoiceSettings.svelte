<script lang="ts">
  // Settings → Voice: one picker for local and online transcription models,
  // Local only, cleanup, and read aloud.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import Slider from "./Slider.svelte";
  import Switch from "./Switch.svelte";
  import VoiceButton from "./VoiceButton.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import SettingsRow from "./SettingsRow.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { formatGb } from "../lib/format";
  import { openInBrowser } from "../lib/links";
  import { dur } from "../lib/motion";
  import { speak, speechAvailable, stopSpeaking, systemVoices } from "../lib/speech";
  import { updateVoice, voice } from "../lib/voice.svelte";
  import type { VoiceModels, VoiceUpdate } from "../lib/api/bindings";

  let models = $state<VoiceModels | null>(null);
  let error = $state<string | null>(null);
  let downloading = $state<Record<string, number>>({});
  let engineBusy = $state<number | null>(null);
  let keyDraft = $state<Record<string, string>>({});
  let openProvider = $state<string | null>(null);
  let tryText = $state<string | null>(null);
  let voices = $state<SpeechSynthesisVoice[]>([]);
  let rate = $state(1);

  const s = $derived(voice.settings);

  const load = async () => {
    try {
      models = await api.voiceModels();
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };
  onMount(() => {
    void load();
    rate = voice.settings?.rate ?? 1;
    const fill = () => (voices = systemVoices());
    fill();
    if (speechAvailable()) window.speechSynthesis.onvoiceschanged = fill;
  });

  const set = async (u: VoiceUpdate) => {
    error = null;
    try {
      await updateVoice(u);
      await load();
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const fetchEngine = async () => {
    engineBusy = 0;
    error = null;
    try {
      await api.fetchVoiceEngine((e) => (engineBusy = Math.round(e.percent)));
      await load();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      engineBusy = null;
    }
  };

  const download = async (id: string) => {
    downloading = { ...downloading, [id]: 0 };
    error = null;
    try {
      await api.downloadModel(id, (e) => {
        if (e.status === "downloading") downloading = { ...downloading, [id]: Math.round(e.percent) };
      });
      await load();
      if (voice.settings?.model !== id && !models?.local.some((m) => m.id === voice.settings?.model && m.installed)) await set({ model: id });
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      const { [id]: _, ...rest } = downloading;
      downloading = rest;
    }
  };

  const saveKey = async (provider: string) => {
    error = null;
    try {
      voice.settings = await api.setVoiceKey(provider, keyDraft[provider] ?? "");
      keyDraft = { ...keyDraft, [provider]: "" };
      await load();
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const providers = $derived.by(() => {
    const out: { id: string; label: string; host: string; key_url: string; has_key: boolean; models: NonNullable<typeof models>["online"] }[] = [];
    for (const m of models?.online ?? []) {
      let p = out.find((x) => x.id === m.provider);
      if (!p) {
        p = { id: m.provider, label: m.provider_label, host: m.host, key_url: m.key_url, has_key: m.has_key, models: [] };
        out.push(p);
      }
      p.models.push(m);
    }
    return out;
  });
  const engineReady = $derived(!!models?.engine.installed);
  const selectedLocal = $derived(models?.local.find((m) => m.id === s?.model) ?? null);
</script>

<div>
  <h2 class="text-xl font-semibold text-ink-50">Voice</h2>
  <p class="mt-1 max-w-[65ch] text-sm leading-6 text-ink-400">Talk to the assistant: hold the microphone button in the chat box, speak, and let go. Local models turn speech into text on this computer; online models use your own key.</p>

  {#if !s}
    <div class="skeleton mt-6 h-24"></div>
  {:else}
    <div class="mt-6"></div>
    <SettingsGroup>
      <SettingsRow icon="chat" title="Voice input" description="Shows the microphone button in the chat box. Off by default.">
        {#snippet control()}<Switch checked={s.enabled} label="Voice input" onchange={(on) => void set({ enabled: on })} />{/snippet}
      </SettingsRow>
      <SettingsRow icon="lock" title="Local only" description={s.local_only ? "On. Voice never uses the network; online models are blocked." : "Off. Online models can be used; each call is in the online log."} class="border-t border-ink-800">
        {#snippet control()}<Switch checked={s.local_only} label="Local only" onchange={(on) => void set({ local_only: on })} />{/snippet}
      </SettingsRow>
      <SettingsRow icon="arrow-up" title="Send when I let go" description="Sends the message as soon as it is transcribed. Off puts the text in the chat box to edit first." class="border-t border-ink-800">
        {#snippet control()}<Switch checked={s.auto_send} label="Send when I let go" onchange={(on) => void set({ auto_send: on })} />{/snippet}
      </SettingsRow>
      <SettingsRow icon="sparkle" title="Tidy the text" description="The model loaded on this computer removes filler words (um, uh) and fixes punctuation before sending. No audio goes to it." class="border-t border-ink-800">
        {#snippet control()}<Switch checked={s.cleanup} label="Tidy the text" onchange={(on) => void set({ cleanup: on })} />{/snippet}
      </SettingsRow>
    </SettingsGroup>

    <div class="mb-2 mt-8 flex flex-wrap items-baseline justify-between gap-2">
      <h3 class="text-[15px] font-semibold text-ink-50">On this computer</h3>
      <span class="text-sm text-ink-500">Download once, then they work offline.</span>
    </div>
    {#if models && !engineReady}
      <div class="mb-3 flex flex-col gap-3 rounded-xl border border-ember-500/30 bg-ember-500/[0.06] p-3 sm:flex-row sm:items-center">
        <Icon name="download" class="hidden h-5 w-5 shrink-0 text-ember-300 sm:block" />
        <p class="flex-1 text-sm leading-6 text-ink-200">Local models run in the sherpa-onnx speech engine (version {models.engine.version}, {formatGb(models.engine.download_bytes ?? 0)}). It is downloaded once from its GitHub release and checked against a pinned checksum.</p>
        {#if models.engine.available}
          <Button variant="primary" disabled={engineBusy !== null} onclick={() => void fetchEngine()}>{engineBusy !== null ? `Downloading ${engineBusy}%` : "Download the engine"}</Button>
        {:else}
          <span class="text-sm text-ink-400">Not available for this computer.</span>
        {/if}
      </div>
    {/if}
    <div class="models" role="radiogroup" aria-label="Local voice models">
      {#each models?.local ?? [] as m (m.id)}
        {@const chosen = s.model === m.id}
        <div class={`model ${chosen ? "model-on" : ""}`}>
          <button type="button" role="radio" aria-checked={chosen} disabled={!m.installed} onclick={() => void set({ model: m.id })} class="pick" aria-label={`Use ${m.name}`}>
            <span class="radio" aria-hidden="true"></span>
          </button>
          <div class="min-w-0 flex-1">
            <p class="flex flex-wrap items-baseline gap-x-2">
              <span class="font-medium text-ink-50">{m.name}</span>
              <span class="text-xs text-ink-500">{m.params} · {formatGb(m.size_bytes)}</span>
              {#if m.id === "stt-whisper-tiny"}<span class="tag">Default</span>{/if}
            </p>
            <p class="mt-0.5 text-[13px] leading-5 text-ink-400">{m.description}</p>
            {#if downloading[m.id] !== undefined}
              <ProgressBar value={downloading[m.id]} label={`Downloading ${m.name}`} class="mt-2 h-1.5 max-w-xs" />
            {/if}
          </div>
          {#if m.installed}
            <span class="text-xs font-medium text-emerald-400">Downloaded</span>
          {:else if downloading[m.id] !== undefined}
            <span class="w-14 text-right text-xs tabular-nums text-ink-300">{downloading[m.id]}%</span>
          {:else}
            <Button variant="secondary" onclick={() => void download(m.id)}><Icon name="download" class="h-4 w-4" />Download</Button>
          {/if}
        </div>
      {:else}
        <div class="skeleton h-16"></div>
      {/each}
    </div>

    <div class="mb-2 mt-8 flex flex-wrap items-baseline justify-between gap-2">
      <h3 class="text-[15px] font-semibold text-ink-50">Online</h3>
      <span class="text-sm text-ink-500"><b class="font-semibold text-ink-300">Live</b> shows text while you speak. <b class="font-semibold text-ink-300">Batch</b> sends the recording when you let go.</span>
    </div>
    <div class={`space-y-2 ${s.local_only ? "opacity-50" : ""}`} aria-disabled={s.local_only}>
      {#if s.local_only}<p class="text-sm text-ink-400">Turn off Local only to use these.</p>{/if}
      {#each providers as p (p.id)}
        <div class="prov">
          <button type="button" class="flex w-full items-center gap-3 text-left" aria-expanded={openProvider === p.id} onclick={() => (openProvider = openProvider === p.id ? null : p.id)}>
            <span class="min-w-0 flex-1">
              <span class="block font-medium text-ink-50">{p.label}</span>
              <span class="block text-[13px] text-ink-400">Sends to {p.host} · {p.has_key ? `Key ${s.key_hints[p.id] ?? "saved"}` : "No key yet"}</span>
            </span>
            <span class="flex flex-wrap justify-end gap-1">
              {#each p.models as m (m.key)}<span class={`badge ${m.delivery === "live" ? "badge-live" : ""}`}>{m.delivery === "live" ? "Live" : "Batch"}</span>{/each}
            </span>
            <Icon name="chevron-down" class={`h-4 w-4 shrink-0 text-ink-500 transition-transform ${openProvider === p.id ? "rotate-180" : ""}`} />
          </button>
          {#if openProvider === p.id}
            <div class="mt-3 space-y-3 border-t border-ink-800 pt-3" transition:slide={{ duration: dur(150) }}>
              <div class="flex flex-wrap items-center gap-2">
                <input type="password" autocomplete="off" spellcheck="false" placeholder={p.has_key ? `Saved: ${s.key_hints[p.id]}` : "Paste your key"} aria-label={`${p.label} key`} bind:value={keyDraft[p.id]} class="h-9 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-[13px] text-ink-100 placeholder:text-ink-500" />
                <Button variant="secondary" disabled={!keyDraft[p.id]?.trim()} onclick={() => void saveKey(p.id)}>Save key</Button>
                {#if p.has_key}<Button variant="subtle" onclick={() => { keyDraft = { ...keyDraft, [p.id]: "" }; void saveKey(p.id); }}>Remove key</Button>{/if}
                <a href={p.key_url} rel="noreferrer" class="inline-flex items-center gap-1 text-sm text-ember-300 hover:underline" onclick={(e) => openInBrowser(e, p.key_url)}>Get a key<Icon name="external" class="h-3.5 w-3.5" /></a>
              </div>
              <p class="text-xs text-ink-500">Saved in this app's settings file on this computer, like your chat provider keys, and sent only to {p.host}.</p>
              <div role="radiogroup" aria-label={`${p.label} models`} class="space-y-1">
                {#each p.models as m (m.key)}
                  {@const chosen = s.model === m.key}
                  <button type="button" role="radio" aria-checked={chosen} disabled={s.local_only || !p.has_key} onclick={() => void set({ model: m.key })} class={`row ${chosen ? "row-on" : ""}`}>
                    <span class="radio" aria-hidden="true"></span>
                    <span class="min-w-0 flex-1"><span class="block text-sm text-ink-100">{m.label} <span class="font-mono text-xs text-ink-500">{m.model}</span></span><span class="block text-xs text-ink-400">{m.note}</span></span>
                    <span class={`badge ${m.delivery === "live" ? "badge-live" : ""}`}>{m.delivery === "live" ? "Live" : "Batch"}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <div class="mt-8 rounded-xl border border-ink-700 bg-ink-900 p-4">
      <div class="flex flex-wrap items-center gap-3">
        <div class="min-w-0 flex-1">
          <h3 class="text-[15px] font-semibold text-ink-50">Try it</h3>
          <p class="mt-0.5 text-sm text-ink-400">Hold the button and say something. Uses {selectedLocal?.name ?? s.model.split(":").slice(1, 3).join(" · ")}.</p>
        </div>
        <VoiceButton placement="below" disabled={!s.model.startsWith("online:") && (!engineReady || !selectedLocal?.installed)} onText={(t) => (tryText = t)} />
      </div>
      {#if tryText !== null}<p class="mt-3 rounded-lg bg-ink-950 px-3 py-2 text-[15px] leading-6 text-ink-100" dir="auto">{tryText}</p>{/if}
      {#if !s.model.startsWith("online:") && (!engineReady || !selectedLocal?.installed)}<p class="mt-2 text-xs text-ink-500">Download the engine and the chosen model first.</p>{/if}
    </div>

    <h3 class="mb-2 mt-8 text-[15px] font-semibold text-ink-50">Read answers aloud</h3>
    <SettingsGroup>
      <SettingsRow icon="volume" title="Read aloud" description={speechAvailable() ? "Adds a speaker button under each answer. Uses your system's voice; nothing leaves this computer." : "This system has no voice for reading aloud."}>
        {#snippet control()}<Switch checked={s.read_aloud} disabled={!speechAvailable()} label="Read answers aloud" onchange={(on) => void set({ read_aloud: on })} />{/snippet}
      </SettingsRow>
      {#if s.read_aloud && speechAvailable()}
        <SettingsRow icon="volume" title="Voice" description="The system voices on this computer." class="border-t border-ink-800">
          {#snippet control()}
            <select id="voice-name" class="h-9 max-w-56 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100" value={s.voice_name ?? ""} onchange={(e) => void set({ voice_name: (e.currentTarget as HTMLSelectElement).value })} aria-label="Voice">
              <option value="">System default</option>
              {#each voices as v (v.name)}<option value={v.name}>{v.name} ({v.lang})</option>{/each}
            </select>
          {/snippet}
        </SettingsRow>
        <SettingsRow icon="sliders" title="Speed" description="How fast answers are read." class="border-t border-ink-800">
          {#snippet control()}
            <Slider min={0.5} max={2} step={0.1} bind:value={rate} onchange={() => void set({ rate })} class="w-36" label="Reading speed" />
            <span class="w-10 text-right text-sm tabular-nums text-ink-200">{rate.toFixed(1)}×</span>
            <Button variant="subtle" onclick={() => { stopSpeaking(); speak("This is how answers will sound.", () => {}, { voiceName: s.voice_name, rate }); }}>Listen</Button>
          {/snippet}
        </SettingsRow>
      {/if}
    </SettingsGroup>
  {/if}

  {#if error}<p class="mt-4 rounded-md bg-rose-500/10 px-3 py-2 text-sm text-rose-300" role="alert">{error}</p>{/if}
</div>

<style>
  .models {
    overflow: hidden;
    border-radius: 12px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
  }
  .model {
    display: flex;
    align-items: center;
    gap: 12px;
    border-top: 1px solid var(--color-ink-800);
    padding: 12px 14px;
  }
  .model:first-child {
    border-top: 0;
  }
  .model-on {
    background: color-mix(in oklab, var(--color-ember-500) 6%, transparent);
  }
  .pick {
    display: grid;
    height: 28px;
    width: 28px;
    flex-shrink: 0;
    place-items: center;
    border-radius: 9999px;
  }
  .pick:disabled {
    cursor: default;
    opacity: 0.4;
  }
  .radio {
    display: block;
    height: 16px;
    width: 16px;
    flex-shrink: 0;
    border-radius: 9999px;
    border: 2px solid var(--color-ink-500);
  }
  .model-on .radio,
  .row-on .radio {
    border: 5px solid var(--color-ember-500);
  }
  .tag {
    border-radius: 6px;
    background: var(--color-ink-800);
    padding: 0 6px;
    font-size: 11px;
    font-weight: 600;
    line-height: 18px;
    color: var(--color-ink-300);
  }
  .prov {
    border-radius: 12px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
    padding: 12px 14px;
  }
  .badge {
    border-radius: 9999px;
    background: var(--color-ink-800);
    padding: 1px 8px;
    font-size: 11px;
    font-weight: 600;
    line-height: 18px;
    color: var(--color-ink-300);
  }
  .badge-live {
    background: color-mix(in oklab, var(--color-ember-500) 16%, transparent);
    color: var(--color-ember-200);
  }
  .row {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 10px;
    border-radius: 8px;
    padding: 8px 10px;
    text-align: left;
  }
  .row:hover:not(:disabled) {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .row:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .row-on {
    background: color-mix(in oklab, var(--color-ember-500) 10%, transparent);
  }
</style>
