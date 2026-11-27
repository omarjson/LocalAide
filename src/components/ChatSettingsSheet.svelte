<script lang="ts">
  // The chat's settings sheet (Reading, Response style, While it works,
  // How LocalAide answers, Sampling, Context window, System prompt).
  // Moved out of ChatScreen.svelte unchanged; the state stays in the chat
  // and is bound here.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import Slider from "./Slider.svelte";
  import { dur, motion } from "../lib/motion";
  import { chatModes, flag, moduleOn } from "../lib/dev.svelte";
  import { MODE_ICONS } from "../lib/chatModes";
  import type { ChatModeId, ChatModeInfo } from "../lib/api/bindings";

  type Width = "cozy" | "wide" | "full";

  interface Props {
    open: boolean;
    chatWidthChoice: Width;
    followStreaming: boolean;
    collapseQuestions: boolean;
    answerStats: boolean;
    systemPresets: { label: string; prompt: string }[];
    systemPrompt: string;
    persona: string;
    learnSkills: boolean;
    suggestNext: boolean;
    visuals: boolean;
    useSubagents: boolean;
    mode: ChatModeId;
    currentModeInfo: ChatModeInfo | undefined;
    temperature: number;
    maxTokens: number;
    autoCompactAt: number;
    setChatWidth: (choice: Width) => void;
    toggleFollowStreaming: () => void;
    toggleCollapseQuestions: () => void;
    toggleAnswerStats: () => void;
    changeMode: (mode: ChatModeId) => void;
    savePrefs: () => void | Promise<void>;
  }

  let {
    open = $bindable(),
    chatWidthChoice,
    followStreaming,
    collapseQuestions,
    answerStats,
    systemPresets,
    systemPrompt = $bindable(),
    persona = $bindable(),
    learnSkills = $bindable(),
    suggestNext = $bindable(),
    visuals = $bindable(),
    useSubagents = $bindable(),
    mode,
    currentModeInfo,
    temperature = $bindable(),
    maxTokens = $bindable(),
    autoCompactAt = $bindable(),
    setChatWidth,
    toggleFollowStreaming,
    toggleCollapseQuestions,
    toggleAnswerStats,
    changeMode,
    savePrefs,
  }: Props = $props();
</script>

{#if open}
  <div
    role="dialog"
    aria-label="Chat settings"
    tabindex="-1"
    onkeydown={(e) => { if (e.key === "Escape") open = false; }}
    class="absolute bottom-0 right-0 top-14 z-30 flex w-96 max-w-[90vw] flex-col border-l border-ink-800 bg-ink-950 shadow-2xl"
    transition:fly={{ x: 24, duration: dur(motion.base), easing: motion.enter }}
  >
    <div class="flex items-center justify-between border-b border-ink-800 px-4 py-3">
      <h2 class="font-semibold text-ink-50">Chat settings</h2>
      <button type="button" aria-label="Close chat settings" onclick={() => open = false} class="chat-icon-btn"><Icon name="close" class="h-4 w-4" /></button>
    </div>
    <div class="flex-1 space-y-6 overflow-y-auto p-4">
      <section>
        <h3 class="mb-2 text-[13px] font-semibold text-ink-300">Reading</h3>
        <div class="mb-3">
          <span class="mb-1.5 block text-sm text-ink-100">Chat width</span>
          <div class="grid grid-cols-3 gap-1 rounded-lg border border-ink-700 bg-ink-900 p-1" role="group" aria-label="Chat width">
            {#each [["cozy", "Comfortable"], ["wide", "Wider"], ["full", "Full"]] as [value, label] (value)}
<button type="button" onclick={() => setChatWidth(value as "cozy" | "wide" | "full")} aria-pressed={chatWidthChoice === value} class={`rounded-md px-2 py-1.5 text-[13px] transition-colors ${chatWidthChoice === value ? "bg-veil/10 font-semibold text-ink-50" : "text-ink-300 hover:bg-veil/5"}`}>{label}</button>
            {/each}
          </div>
        </div>
        <div class="space-y-1">
          {#each [
            { label: "Follow answers while run.streaming", hint: "Keep the newest text in view instead of fighting the scrollbar.", on: followStreaming, toggle: toggleFollowStreaming },
            { label: "Collapse long questions", hint: "Show the first lines of a long prompt; click to expand.", on: collapseQuestions, toggle: toggleCollapseQuestions },
          ] as o (o.label)}
            <button type="button" role="switch" aria-checked={o.on} onclick={o.toggle} class="flex w-full items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-veil/5">
              <span class="min-w-0 flex-1"><span class="block text-sm text-ink-100">{o.label}</span><span class="block text-xs leading-5 text-ink-400">{o.hint}</span></span>
              <span class={`sheet-switch ${o.on ? "sheet-switch-on" : ""}`} aria-hidden="true"><span></span></span>
            </button>
          {/each}
        </div>
      </section>

      <section>
        <h3 class="mb-2 text-[13px] font-semibold text-ink-300">Response style</h3>
        <div class="grid grid-cols-2 gap-2">
          {#each systemPresets as p (p.label)}
<button type="button" onclick={() => { systemPrompt = p.prompt; persona = p.label; void savePrefs(); }} aria-pressed={systemPrompt === p.prompt} class={`rounded-lg border px-3 py-2 text-left text-sm transition-colors ${systemPrompt === p.prompt ? "border-ink-600 bg-veil/8 text-ink-50" : "border-ink-700 text-ink-300 hover:border-edge hover:text-ink-50"}`}>{p.label}</button>
          {/each}
        </div>
      </section>

      <section>
        <h3 class="mb-2 text-[13px] font-semibold text-ink-300">While it works</h3>
        <div class="space-y-1">
          {#each [
            { label: "Learn skills from chats", hint: "Saves and improves skills on its own, with a note and Undo.", on: learnSkills, toggle: () => (learnSkills = !learnSkills), show: moduleOn("skills") },
            { label: "Suggest a next prompt", hint: "One short suggestion after each answer. Tab takes it.", on: suggestNext, toggle: () => (suggestNext = !suggestNext), show: true },
            { label: "Answer statistics", hint: "Tokens, time and speed under each answer.", on: answerStats, toggle: toggleAnswerStats, show: true },
            { label: "Charts, diagrams and mockups", hint: "Draws them in the chat, sandboxed from your files.", on: visuals, toggle: () => (visuals = !visuals), show: true },
            { label: "Subagents", hint: "Hands parts of a task to helpers that only read.", on: useSubagents, toggle: () => (useSubagents = !useSubagents), show: true },
          ].filter((o) => o.show) as o (o.label)}
            <button type="button" role="switch" aria-checked={o.on} onclick={o.toggle} class="flex w-full items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-veil/5">
              <span class="min-w-0 flex-1"><span class="block text-sm text-ink-100">{o.label}</span><span class="block text-xs leading-5 text-ink-400">{o.hint}</span></span>
              <span class={`sheet-switch ${o.on ? "sheet-switch-on" : ""}`} aria-hidden="true"><span></span></span>
            </button>
          {/each}
        </div>
      </section>

      <section>
        <h3 class="mb-2 text-[13px] font-semibold text-ink-300">How LocalAide answers</h3>
        <div class="space-y-1.5">
          {#each chatModes() as m (m.id)}
<button type="button" onclick={() => changeMode(m.id)} aria-pressed={mode === m.id} class={`flex w-full items-start gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors ${mode === m.id ? "border-ink-600 bg-veil/8" : "border-ink-700 hover:border-edge"}`}>
              <Icon name={MODE_ICONS[m.id]} class={`mt-0.5 h-4 w-4 shrink-0 ${mode === m.id ? "text-ink-400" : "text-ink-400"}`} />
              <span><span class="block text-sm font-semibold text-ink-100">{m.label}</span><span class="block text-xs leading-5 text-ink-400">{m.description}</span></span>
            </button>
          {/each}
        </div>
      </section>

      {#if flag("sampling_controls")}
        <section>
          <h3 class="mb-2 flex items-center gap-1.5 text-[13px] font-semibold text-ink-300"><Icon name="flask" class="h-3.5 w-3.5" />Sampling</h3>
          <label class="block text-sm text-ink-300">
            <span class="flex justify-between"><span>Creativity (temperature)</span><span class="font-mono text-ink-100">{temperature.toFixed(1)}{mode === "strict" ? " · capped at 0.3" : ""}</span></span>
            <Slider min={0} max={2} step={0.1} bind:value={temperature} onchange={savePrefs} class="mt-1 w-full" label="Temperature" />
          </label>
          <label class="mt-3 block text-sm text-ink-300">
            <span class="flex justify-between"><span>Longest answer</span><span class="font-mono text-ink-100">{maxTokens} tokens</span></span>
            <Slider min={64} max={8192} step={64} bind:value={maxTokens} onchange={savePrefs} class="mt-1 w-full" label="Longest answer in tokens" />
          </label>
        </section>
      {/if}

      <section>
        <h3 class="mb-2 flex items-center gap-1.5 text-[13px] font-semibold text-ink-300"><Icon name="layers" class="h-3.5 w-3.5" />Context window</h3>
        <label class="block text-sm text-ink-300">
          <span class="flex justify-between"><span>Auto-compact at</span><span class="font-mono text-ink-100">{autoCompactAt}%</span></span>
          <Slider min={50} max={95} step={5} bind:value={autoCompactAt} class="mt-1 w-full" label="Auto-compact threshold" />
        </label>
        <p class="mt-2 text-xs leading-5 text-ink-500">Older turns are shortened before a new answer when the context meter reaches this level. Use the circle beside the model selector to see estimated input and output usage.</p>
      </section>

      {#if flag("system_prompt_editor")}
        <section>
          <textarea bind:value={systemPrompt} rows={6} onblur={savePrefs} aria-label="System prompt" class="w-full resize-y rounded-lg border border-ink-700 bg-ink-900 p-3 font-mono text-[13px] leading-6 text-ink-100"></textarea>
          {#if currentModeInfo?.instruction}
            <p class="mb-1 mt-3 text-xs font-semibold text-ink-400">{currentModeInfo.label} mode adds:</p>
            <p class="rounded-lg border border-ink-800 bg-ink-900/60 p-3 font-mono text-[12px] leading-5 text-ink-400">{currentModeInfo.instruction}</p>
          {/if}
        </section>
      {:else}
        <p class="text-xs leading-5 text-ink-500">Turn on developer mode in Settings to write your own system prompt and adjust sampling.</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .chat-icon-btn {
    display: grid;
    height: 2.25rem;
    width: 2.25rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .chat-icon-btn:hover {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .sheet-switch {
    position: relative;
    margin-top: 0.2rem;
    height: 1.25rem;
    width: 2.25rem;
    flex-shrink: 0;
    border-radius: 9999px;
    background: var(--color-ink-700);
    transition: background-color 150ms ease-out;
  }
  .sheet-switch span {
    position: absolute;
    left: 0.15rem;
    top: 0.15rem;
    height: 0.95rem;
    width: 0.95rem;
    border-radius: 9999px;
    background: white;
    transition: transform 160ms cubic-bezier(0.23, 1, 0.32, 1);
  }
  .sheet-switch-on {
    background: var(--color-ink-100);
  }
  .sheet-switch-on span {
    transform: translateX(1rem);
    background: var(--color-ink-950);
  }
</style>
