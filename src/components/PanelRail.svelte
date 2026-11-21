<script lang="ts">
  // The chat's side rail: live cards for what the answer used, what it is
  // doing, and what it can do. Every card is active — sources ask back,
  // skills drop into the composer — none of them are just decoration.
  import { fly } from "svelte/transition";
  import ContextMeter from "./ContextMeter.svelte";
  import Icon from "./Icon.svelte";
  import WorkStatus from "./WorkStatus.svelte";
  import type { Citation, MessagePart, Skill, TodoItem } from "../lib/api/bindings";
  import type { ContextUsage } from "../lib/contextUsage";
  import { dur, motion } from "../lib/motion";

  type ToolPart = Extract<MessagePart, { type: "tool" }>;

  interface Props {
    sources: Citation[];
    skills: Skill[];
    todos: TodoItem[] | null;
    subagents: ToolPart[];
    startedAt: number | null;
    toolRounds: number;
    live: boolean;
    usage: ContextUsage;
    autoCompactAt: number;
    onAsk: (text: string) => void;
    onSkill: (name: string) => void;
    onCompact: () => void;
    onClose: () => void;
  }

  let { sources, skills, todos, subagents, startedAt, toolRounds, live, usage, autoCompactAt, onAsk, onSkill, onCompact, onClose }: Props = $props();

  const busy = $derived(live || (todos?.length ?? 0) > 0 || subagents.length > 0 || startedAt !== null);
</script>

<aside
  aria-label="Chat panels"
  class="absolute bottom-0 right-0 top-14 z-10 hidden w-72 flex-col border-l border-edge bg-ink-950/95 lg:flex"
  in:fly={{ x: 24, duration: dur(motion.base), easing: motion.enter }}
>
  <div class="flex shrink-0 items-center justify-between border-b border-ink-800 px-4 py-2.5">
    <h2 class="text-sm font-semibold text-ink-100">Chat panels</h2>
    <button type="button" aria-label="Hide the panels" onclick={onClose} class="chat-icon-btn"><Icon name="close" class="h-4 w-4" /></button>
  </div>

  <div class="min-h-0 flex-1 space-y-3 overflow-y-auto p-3">
    {#if busy}
      <section class="rounded-xl border border-ink-700 bg-ink-900 p-3" aria-label="Work in progress">
        <WorkStatus {todos} {subagents} {startedAt} {toolRounds} {live} />
      </section>
    {/if}

    {#if todos && todos.length > 0}
      <!-- The model's plan, item by item: done ticks ember, the step in
           progress carries the spinner, the rest wait in line. -->
      <section class="rounded-xl border border-ink-700 bg-ink-900 p-3" aria-label="Plan">
        <div class="mb-2 flex items-center justify-between">
          <h3 class="flex items-center gap-1.5 text-sm font-semibold text-ink-100"><Icon name="list-check" class="h-3.5 w-3.5 text-ink-400" />Plan</h3>
          <span class="text-xs tabular-nums text-ink-500">{todos.filter((t) => t.status === "done").length}/{todos.length}</span>
        </div>
        <ul class="space-y-1.5" aria-label="Todo list">
          {#each todos as todo, i (i)}
            <li class="flex items-start gap-2 text-sm" aria-label={`Todo ${i + 1}: ${todo.text}`}>
              {#if todo.status === "done"}
                <span class="mt-0.5 grid h-4 w-4 shrink-0 place-items-center rounded-full bg-ember-500/20 text-ember-300"><Icon name="check" class="h-2.5 w-2.5" /></span>
              {:else if todo.status === "in_progress"}
                <span class="mt-0.5 h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-ink-700 border-t-ember-400" aria-label="In progress"></span>
              {:else}
                <span class="mt-0.5 h-4 w-4 shrink-0 rounded-full border border-ink-600" aria-label="Pending"></span>
              {/if}
              <span
                class={`min-w-0 flex-1 ${todo.status === "done" ? "text-ink-500 line-through" : todo.status === "in_progress" ? "font-medium text-ink-50" : "text-ink-300"}`}
              >{todo.text}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <section class="rounded-xl border border-ink-700 bg-ink-900 p-3" aria-label="Context window">
      <div class="mb-2 flex items-center justify-between">
        <h3 class="flex items-center gap-1.5 text-sm font-semibold text-ink-100"><Icon name="database" class="h-3.5 w-3.5 text-ink-400" />Context</h3>
        {#if usage.limit > 0}
          <button type="button" class="rounded-md px-2 py-0.5 text-xs text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-100" onclick={onCompact} title="Compact: drop older turns to free room">Compact</button>
        {/if}
      </div>
      <ContextMeter {usage} streaming={live} {autoCompactAt} compacted={false} onCompact={onCompact} />
      {#if usage.limit > 0}
        <p class="mt-2 text-xs text-ink-500">{usage.used.toLocaleString()} of {usage.limit.toLocaleString()} tokens used{usage.overflow ? " — the next answer will not fit" : ""}</p>
      {/if}
    </section>

    {#if sources.length > 0}
      <section class="rounded-xl border border-ink-700 bg-ink-900 p-3" aria-label="Sources of the last answer">
        <h3 class="mb-2 flex items-center gap-1.5 text-sm font-semibold text-ink-100"><Icon name="file" class="h-3.5 w-3.5 text-ink-400" />Sources</h3>
        <div class="space-y-1.5">
          {#each sources as source (source.index)}
            <div class="group/source flex min-w-0 items-center gap-1.5 rounded-lg px-1.5 py-1 transition-colors hover:bg-veil/5">
              <span class="shrink-0 text-xs tabular-nums text-ink-500">[{source.index}]</span>
              <span class="min-w-0 flex-1 truncate text-sm text-ink-200" title={source.title}>{source.title}</span>
              <button
                type="button"
                class="shrink-0 rounded-md px-1.5 py-0.5 text-xs text-ink-500 opacity-0 transition-[opacity,color] group-hover/source:opacity-100 hover:text-ember-300 focus-visible:opacity-100"
                title={`Ask what this source says`}
                onclick={() => onAsk(`What does "${source.title}" say about it? Keep it short.`)}
              >Ask</button>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    {#if skills.length > 0}
      <section class="rounded-xl border border-ink-700 bg-ink-900 p-3" aria-label="Your skills">
        <h3 class="mb-2 flex items-center gap-1.5 text-sm font-semibold text-ink-100"><Icon name="skills" class="h-3.5 w-3.5 text-ink-400" />Skills</h3>
        <div class="space-y-1">
          {#each skills.slice(0, 8) as skill (skill.name)}
            <button
              type="button"
              class="flex w-full min-w-0 items-center gap-2 rounded-lg px-1.5 py-1 text-left transition-colors hover:bg-veil/5"
              title={`Type /${skill.name} in the composer`}
              onclick={() => onSkill(skill.name)}
            >
              <span class="shrink-0 rounded-md bg-ink-800 px-1.5 py-0.5 text-xs text-ember-300">/{skill.name}</span>
              <span class="min-w-0 flex-1 truncate text-xs text-ink-400">{skill.description}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</aside>
