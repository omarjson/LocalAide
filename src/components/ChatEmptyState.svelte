<script lang="ts">
  // The empty-chat view: logo, greeting, the model status line, the
  // local-index stats, the starter prompts, and the skills chips.
  // Moved out of ChatScreen unchanged; the screen owns the data and
  // the send actions, this is only the markup.
  import { fade, fly } from "svelte/transition";
  import { dur, motion, stagger } from "../lib/motion";
  import { MODE_ICONS } from "../lib/chatModes";
  import type { ChatModeId, ModelChoice, ModelChoices, Skill } from "../lib/api/bindings";
  import LogoMark from "./LogoMark.svelte";
  import Button from "./Button.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  interface Props {
    chatWidth: string;
    modelsChecked: boolean;
    activeChoice: ModelChoice | null;
    choices: ModelChoices | null;
    onOpenModels?: () => void;
    moduleOn: (id: string) => boolean;
    onOpenAddProvider: () => void;
    greeting: string;
    modelNotice: string | null;
    onlineChoice: ModelChoice | null;
    engineLoading: boolean;
    pickerBusy: boolean;
    engineLoaded: boolean;
    useWeb: boolean;
    webAvailable: boolean;
    mode: ChatModeId;
    /** The current mode's label, or null for the standard mode. */
    modeLabel: string | null;
    localDocs: number | null;
    localNotes: number | null;
    localBudgetGb: number | null;
    starterPrompts: { title: string; prompt: string; icon: IconName; learned: boolean }[];
    skills: Skill[];
    onPickStarter: (prompt: string) => void;
    onPickSkill: (body: string) => void;
  }
  let {
    chatWidth,
    modelsChecked,
    activeChoice,
    choices,
    onOpenModels,
    moduleOn,
    onOpenAddProvider,
    greeting,
    modelNotice,
    onlineChoice,
    engineLoading,
    pickerBusy,
    engineLoaded,
    useWeb,
    webAvailable,
    mode,
    modeLabel,
    localDocs,
    localNotes,
    localBudgetGb,
    starterPrompts,
    skills,
    onPickStarter,
    onPickSkill,
  }: Props = $props();
</script>

<div class={`ember-glow mx-auto flex min-h-full ${chatWidth} flex-col items-center justify-center gap-8 px-5 py-10 text-center`}>
  <div class="flex max-w-lg flex-col items-center gap-2" in:fly={{ y: 10, duration: dur(360), easing: motion.enter }}>
    <div class="relative mb-3">
      <div class="absolute inset-0 -z-10 scale-[1.8] rounded-full bg-ember-500/20 blur-3xl" aria-hidden="true"></div>
      <LogoMark class="h-16 w-16 drop-shadow-xl" />
    </div>
    {#if !modelsChecked}
      <h1 class="text-2xl font-semibold tracking-tight text-ink-50">Getting ready…</h1>
    {:else if !activeChoice}
      <h1 class="text-2xl font-semibold tracking-tight text-ink-50">Pick a model to start</h1>
      <p class="text-[15px] leading-6 text-ink-400">{choices && choices.models.length > 0 ? "Choose one in the model menu under the message box." : "Models run on this computer. Download one, or connect a free online provider such as Kilo Code in Settings."}</p>
      <div class="mt-3 flex flex-wrap justify-center gap-2">
        {#if onOpenModels}<Button variant="primary" class="h-9 px-4" onclick={onOpenModels}><Icon name="models" class="h-4 w-4" />Open Models</Button>{/if}
        {#if moduleOn("online")}<Button variant="secondary" class="h-9 px-4" onclick={onOpenAddProvider}><Icon name="cloud" class="h-4 w-4" />Use a free provider</Button>{/if}
      </div>
    {:else}
      <h1 class="text-balance text-[30px] font-semibold tracking-tight text-ink-50">{greeting} What can I help with?</h1>
      {#if modelNotice}
        <p class="text-[15px] text-ink-400">{modelNotice}</p>
      {/if}
      {#if onlineChoice}
        <p class="text-[15px] text-ink-400">Answers come from <span class="font-semibold text-ink-200">{onlineChoice.name}</span>, through {onlineChoice.provider_label}. Messages are sent to that provider.</p>
      {:else if engineLoading || pickerBusy}
        <p class="flex items-center gap-2 text-[15px] text-ink-400" role="status"><span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-edge border-t-ember-500"></span>Loading {activeChoice.name}. This can take a minute.</p>
      {:else if !engineLoaded}
        <p class="text-[15px] text-ink-400">{activeChoice.name} is not loaded yet. Choose it in the model menu under the message box.</p>
      {:else}
        <p class="text-[15px] text-ink-400">Running <span class="font-semibold text-ink-200">{activeChoice.name}</span> on this computer. Nothing you type leaves it{useWeb && webAvailable ? ", except web searches the model makes" : ""}.</p>
      {/if}
      {#if mode !== "standard" && modeLabel}
        <span class="mt-2 inline-flex items-center gap-1.5 rounded-full border border-ember-500/40 bg-ember-500/10 px-3 py-1 text-xs font-medium text-ember-200"><Icon name={MODE_ICONS[mode]} class="h-3.5 w-3.5" />{modeLabel} mode</span>
      {/if}
    {/if}
  </div>

  {#if activeChoice && (localDocs !== null || localNotes !== null || localBudgetGb !== null)}
    <div class="flex max-w-lg flex-wrap items-center justify-center gap-x-3.5 gap-y-1 text-xs text-ink-500" in:fade={{ duration: dur(300), delay: dur(120) }}>
      {#if localDocs !== null}<span><span class="font-semibold tabular-nums text-ink-300">{localDocs}</span> documents indexed</span>{/if}
      {#if localDocs !== null && localNotes !== null}<span class="h-3 w-px bg-ink-700" aria-hidden="true"></span>{/if}
      {#if localNotes !== null}<span><span class="font-semibold tabular-nums text-ink-300">{localNotes}</span> memory notes</span>{/if}
      {#if (localDocs !== null || localNotes !== null) && localBudgetGb !== null}<span class="h-3 w-px bg-ink-700" aria-hidden="true"></span>{/if}
      {#if localBudgetGb !== null}<span><span class="font-semibold tabular-nums text-ink-300">{localBudgetGb} GB</span> of model memory</span>{/if}
    </div>
  {/if}

  {#if activeChoice}
    <div class={`grid w-full ${chatWidth} grid-cols-1 gap-2.5 sm:grid-cols-2`}>
      {#each starterPrompts as sp, i (sp.prompt)}
        <button type="button" onclick={() => onPickStarter(sp.prompt)} in:fly={stagger(i + 2)} title={sp.learned ? "You often start chats like this" : undefined} class="lift group flex items-start gap-3 rounded-2xl border border-ink-700 bg-veil/5 p-3.5 text-left hover:border-ink-700 hover:bg-ink-800/60">
          <span class="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-ember-500/12 text-ember-300 transition-colors group-hover:bg-ember-500/20"><Icon name={sp.icon} class="h-[17px] w-[17px]" /></span>
          <span class="min-w-0">
            <span class="block truncate text-sm font-semibold text-ink-100">{sp.title}</span>
            <span class="line-clamp-2 block text-[13px] leading-5 text-ink-400">{sp.prompt}</span>
          </span>
        </button>
      {/each}
    </div>
    {#if skills.length > 0 && moduleOn("skills")}
      <div class={`flex w-full ${chatWidth} flex-wrap items-center justify-center gap-2`} in:fade={{ duration: dur(300), delay: dur(200) }}>
        <span class="text-xs text-ink-500">Your skills</span>
        {#each skills.slice(0, 6) as skill (skill.path)}
          <button type="button" onclick={() => onPickSkill(skill.body)} class="rounded-full border border-ink-700 bg-veil/5 px-3 py-1 font-mono text-xs text-ink-300 transition-colors hover:border-ember-500/50 hover:text-ember-200">/{skill.name}</button>
        {/each}
      </div>
    {/if}
  {/if}
</div>
