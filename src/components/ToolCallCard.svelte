<script lang="ts">
  import { slide } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import type { ToolStatus } from "../lib/api/bindings";

  interface Props {
    title: string;
    detail: string;
    group: string;
    status: ToolStatus;
    summary: string;
    arguments: string;
    output: string;
    /** Shown while the tool waits for the user. */
    approval?: { preview: string } | null;
    onDecide?: (decision: "allow" | "allow_chat" | "deny") => void;
  }

  let {
    title,
    detail,
    group,
    status,
    summary,
    arguments: args,
    output,
    approval = null,
    onDecide,
  }: Props = $props();

  let open = $state(false);
  let allowEl = $state<HTMLButtonElement>();

  const ICONS: Record<string, IconName> = {
    web: "globe",
    notes: "memory",
    skills: "skills",
    files: "folder",
    mcp: "plug",
  };
  const icon = $derived(ICONS[group] ?? "wrench");
  const waiting = $derived(status === "waiting" && approval !== null);
  const hasDetails = $derived((args.trim() && args.trim() !== "{}") || output.trim().length > 0);

  // A question for the user takes the keyboard, so Enter answers it.
  $effect(() => {
    if (waiting) {
      allowEl?.scrollIntoView({ block: "nearest" });
      allowEl?.focus();
    }
  });
</script>

<div class={`tool ${waiting ? "tool-waiting" : ""}`}>
  <button
    type="button"
    class="tool-head"
    aria-expanded={hasDetails ? open : undefined}
    disabled={!hasDetails}
    onclick={() => (open = !open)}
  >
    <span class={`tool-icon ${status === "running" ? "tool-icon-live" : ""}`}><Icon name={icon} class="h-3.5 w-3.5" /></span>
    <span class="min-w-0 flex-1 truncate text-left">
      <span class="font-medium text-ink-100">{title}</span>
      {#if detail}<span class="ml-1.5 text-ink-400">{detail}</span>{/if}
    </span>
    <span class="flex shrink-0 items-center gap-1.5 text-xs">
      {#if status === "running"}
        <span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
        <span class="text-ink-400">Working</span>
      {:else if status === "waiting"}
        <span class="rounded-full bg-amber-500/15 px-2 py-0.5 font-medium text-amber-200">Needs your OK</span>
      {:else if status === "done"}
        <span class="max-w-[12rem] truncate text-ink-500">{summary}</span>
        <Icon name="check" class="h-3.5 w-3.5 text-emerald-400" />
      {:else if status === "denied"}
        <span class="text-ink-500">Not allowed</span>
        <Icon name="x-circle" class="h-3.5 w-3.5 text-ink-500" />
      {:else}
        <span class="max-w-[14rem] truncate text-rose-300" title={summary}>{summary || "Failed"}</span>
        <Icon name="alert" class="h-3.5 w-3.5 text-rose-300" />
      {/if}
      {#if hasDetails}
        <Icon name="chevron-down" class={`h-3.5 w-3.5 text-ink-500 transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
      {/if}
    </span>
  </button>

  {#if waiting && approval}
    <div class="border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      <p class="text-sm text-ink-300">LocalAide wants to do this. Nothing happens until you choose.</p>
      {#if approval.preview.trim()}
        <pre class="tool-pre mt-2 max-h-48">{approval.preview}</pre>
      {/if}
      <div class="mt-3 flex flex-wrap gap-2">
        <button bind:this={allowEl} type="button" onclick={() => onDecide?.("allow")} class="rounded-lg bg-ember-600 px-3 py-1.5 text-sm font-semibold text-on-ember transition-colors hover:bg-ember-500">Allow once</button>
        <button type="button" onclick={() => onDecide?.("allow_chat")} class="rounded-lg border border-edge px-3 py-1.5 text-sm text-ink-100 transition-colors hover:bg-veil/5">Allow for this chat</button>
        <button type="button" onclick={() => onDecide?.("deny")} class="rounded-lg px-3 py-1.5 text-sm text-ink-300 transition-colors hover:bg-veil/5 hover:text-ink-100">Don't allow</button>
      </div>
    </div>
  {/if}

  {#if open && hasDetails}
    <div class="space-y-2 border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      {#if args.trim() && args.trim() !== "{}"}
        <div>
          <div class="mb-1 text-xs font-semibold text-ink-500">Input</div>
          <pre class="tool-pre max-h-40">{args}</pre>
        </div>
      {/if}
      {#if output.trim()}
        <div>
          <div class="mb-1 text-xs font-semibold text-ink-500">Result</div>
          <pre class="tool-pre max-h-64">{output}</pre>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool {
    overflow: hidden;
    border-radius: 0.6rem;
    border: 1px solid var(--color-ink-800);
    background: color-mix(in oklab, var(--color-ink-950) 70%, transparent);
    transition: border-color 150ms ease-out;
  }
  .tool-waiting {
    border-color: color-mix(in oklab, var(--color-amber-400, #fbbf24) 45%, transparent);
  }
  .tool-head {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    padding: 0.45rem 0.65rem;
    font-size: 0.875rem;
    transition: background-color 150ms ease-out;
  }
  .tool-head:not(:disabled):hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .tool-head:disabled {
    cursor: default;
  }
  .tool-icon {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    flex-shrink: 0;
    place-items: center;
    border-radius: 0.4rem;
    background: var(--color-ink-800);
    color: var(--color-ink-300);
  }
  .tool-icon-live {
    color: var(--color-ember-300);
  }
  .tool-pre {
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    border-radius: 0.5rem;
    border: 1px solid var(--color-ink-800);
    background: var(--color-ink-950);
    padding: 0.5rem 0.65rem;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12.5px;
    line-height: 1.25rem;
    color: var(--color-ink-300);
  }
</style>
