<script lang="ts">
  // Work status above the chat box: the model's to-do list ("Completed
  // 2/4") and the subagents of this chat, each with its current step, so
  // one that stalled or went the wrong way shows without opening its chat.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import TodoList from "./TodoList.svelte";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import type { MessagePart, TodoItem } from "../lib/api/bindings";

  type ToolPart = Extract<MessagePart, { type: "tool" }>;

  let { todos = null, subagents = [] }: { todos?: TodoItem[] | null; subagents?: ToolPart[] } = $props();

  let openPanel = $state<"todos" | "agents" | null>(null);
  let expanded = $state<string | null>(null);

  const done = $derived(todos ? todos.filter((t) => t.status === "done").length : 0);
  const running = $derived(subagents.filter((s) => s.status === "running").length);
  const failed = $derived(subagents.filter((s) => s.status === "failed").length);
  const stepOf = (s: ToolPart) => s.steps?.at(-1) ?? (s.status === "running" ? "Starting" : "");
</script>

{#if (todos && todos.length > 0) || subagents.length > 0}
  <div class="relative mb-2 flex flex-wrap items-center justify-center gap-2" use:popover={{ open: openPanel !== null, onClose: () => (openPanel = null) }}>
    {#if todos && todos.length > 0}
      <button type="button" class="ws-chip" aria-expanded={openPanel === "todos"} onclick={() => (openPanel = openPanel === "todos" ? null : "todos")}>
        {#if done === todos.length}
          <Icon name="check" class="h-3.5 w-3.5 text-emerald-400" />Completed
        {:else}
          <span class="h-3 w-3 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>In progress
        {/if}
        <span class="tabular-nums text-ink-400">{done}/{todos.length}</span>
      </button>
    {/if}
    {#if subagents.length > 0}
      <button type="button" class="ws-chip" aria-expanded={openPanel === "agents"} onclick={() => (openPanel = openPanel === "agents" ? null : "agents")} aria-label={`Subagents: ${running} running, ${subagents.length} in this chat`}>
        <span class="flex -space-x-1.5" aria-hidden="true">
          {#each subagents.slice(-3) as s (s.id)}
            <span class={`ws-avatar ${s.status === "running" ? "ws-avatar-live" : s.status === "failed" ? "ws-avatar-bad" : ""}`}><Icon name={s.status === "done" ? "check" : s.status === "failed" ? "alert" : "users"} class="h-2.5 w-2.5" /></span>
          {/each}
        </span>
        {running > 0 ? "Running" : "Completed"}
        <span class="tabular-nums text-ink-400">{running > 0 ? running : subagents.length}</span>
        {#if failed > 0}<span class="text-rose-300">· {failed} failed</span>{/if}
      </button>
    {/if}

    {#if openPanel}
      <div class="ws-panel" role="dialog" aria-label={openPanel === "todos" ? "Plan" : "Subagents"} in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
        {#if openPanel === "todos" && todos}
          <div class="mb-2 flex items-center gap-2 text-sm font-semibold text-ink-100"><Icon name="list-check" class="h-4 w-4 text-ink-400" />Plan<span class="text-xs font-normal tabular-nums text-ink-500">{done} of {todos.length} done</span></div>
          <TodoList items={todos} compact />
        {:else}
          <div class="mb-2 flex items-center gap-2 text-sm font-semibold text-ink-100"><Icon name="users" class="h-4 w-4 text-ink-400" />Subagents<span class="text-xs font-normal text-ink-500">They search and read; they cannot change anything.</span></div>
          <ul class="max-h-80 space-y-1 overflow-y-auto">
            {#each [...subagents].reverse() as s (s.id)}
              <li class="rounded-lg border border-ink-800 bg-ink-900/60">
                <button type="button" class="flex w-full items-start gap-2.5 px-2.5 py-2 text-left" aria-expanded={expanded === s.id} onclick={() => (expanded = expanded === s.id ? null : s.id)}>
                  <span class="mt-0.5 shrink-0">
                    {#if s.status === "running"}<span class="block h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500"></span>
                    {:else if s.status === "done"}<Icon name="check" class="h-3.5 w-3.5 text-emerald-400" />
                    {:else}<Icon name="alert" class="h-3.5 w-3.5 text-rose-300" />{/if}
                  </span>
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-sm font-medium text-ink-100">{s.detail || "Subagent"}</span>
                    <span class={`block truncate text-xs ${s.status === "failed" ? "text-rose-300" : "text-ink-500"}`}>{s.status === "failed" ? s.summary || "Failed" : stepOf(s)}</span>
                  </span>
                  <Icon name="chevron-down" class={`mt-1 h-3.5 w-3.5 shrink-0 text-ink-500 transition-transform duration-200 ${expanded === s.id ? "rotate-180" : ""}`} />
                </button>
                {#if expanded === s.id}
                  <div class="space-y-2 border-t border-ink-800 px-2.5 pb-2.5 pt-2">
                    {#if s.steps?.length}
                      <ol class="space-y-0.5 text-xs text-ink-400">{#each s.steps as step, i (i)}<li class="truncate">· {step}</li>{/each}</ol>
                    {/if}
                    {#if s.output.trim()}<pre class="max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-ink-950 p-2 text-xs leading-5 text-ink-300">{s.output}</pre>{/if}
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .ws-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    height: 1.9rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
    padding: 0 0.75rem;
    font-size: 0.8125rem;
    color: var(--color-ink-100);
    transition: border-color 150ms ease-out, background-color 150ms ease-out;
  }
  .ws-chip:hover,
  .ws-chip[aria-expanded="true"] {
    border-color: var(--color-edge);
    background: var(--color-ink-800);
  }
  .ws-avatar {
    display: grid;
    height: 1.1rem;
    width: 1.1rem;
    place-items: center;
    border-radius: 9999px;
    border: 1.5px solid var(--color-ink-900);
    background: #3b82f6;
    color: white;
  }
  .ws-avatar-live {
    background: var(--color-ember-500);
  }
  .ws-avatar-bad {
    background: #e11d48;
  }
  .ws-panel {
    position: absolute;
    bottom: calc(100% + 0.5rem);
    left: 50%;
    z-index: 35;
    width: min(26rem, calc(100vw - 2rem));
    transform: translateX(-50%);
    border-radius: 0.85rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-950);
    padding: 0.75rem;
    box-shadow: 0 24px 48px -20px rgb(0 0 0 / 0.6);
  }
</style>
