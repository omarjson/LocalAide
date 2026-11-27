<script lang="ts">
  // A subagent's work, read-only, in a side panel: what the main assistant
  // asked it to do, what it did step by step, what it used, and the report
  // it sent back. There is no composer and no steer here on purpose: a
  // subagent cannot be talked to, and its run is already over (or it is
  // still running and this view follows it). The full turn-by-turn
  // transcript is not kept — the backend only returns the report — so the
  // step log is the record of what happened.
  import { fade, fly } from "svelte/transition";
  import FocusTrap from "./FocusTrap.svelte";
  import Icon from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import type { MessagePart } from "../lib/api/bindings";

  type ToolPart = Extract<MessagePart, { type: "tool" }>;

  let { agent, onClose }: { agent: ToolPart | null; onClose: () => void } = $props();

  // The task the main assistant sent, from the tool call's arguments.
  const task = $derived.by(() => {
    if (!agent) return "";
    try {
      const args = JSON.parse(agent.arguments) as { task?: unknown; title?: unknown };
      return typeof args.task === "string" ? args.task : "";
    } catch {
      return "";
    }
  });
  const tokenLabel = $derived(agent?.tokens !== null && agent?.tokens !== undefined ? `${agent.tokens.toLocaleString("en-US")} tokens` : null);
  const statusLabel = $derived(!agent ? "" : agent.status === "running" ? "Running" : agent.status === "done" ? "Done" : agent.status === "failed" ? "Failed" : agent.status === "waiting" ? "Waiting" : "Not allowed");
</script>

{#if agent}
  {@const a = agent}
  <div
    role="presentation"
    in:fade={{ duration: dur(motion.fast) }}
    out:fade={{ duration: dur(motion.fast) }}
    class="fixed inset-0 z-[70] bg-black/60 backdrop-blur-sm"
    onclick={onClose}
    onkeydown={(e) => { if (e.key === "Escape") onClose(); }}
  >
    <div
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label={`Subagent: ${a.detail || "Subagent"}`}
      in:fly={{ x: 24, duration: dur(motion.base), easing: motion.enter }}
      out:fly={{ x: 24, duration: dur(motion.fast) }}
      class="absolute bottom-0 right-0 top-0 flex w-full max-w-md flex-col overflow-hidden border-l border-ink-800 bg-ink-900 shadow-2xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => { if (e.key === "Escape") onClose(); e.stopPropagation(); }}
    >
      <FocusTrap>
        <header class="flex shrink-0 items-start justify-between gap-3 border-b border-ink-800 px-5 py-3.5">
          <div class="min-w-0">
            <div class="flex items-center gap-2 text-xs font-medium text-ink-400"><Icon name="users" class="h-3.5 w-3.5" />Subagent</div>
            <h2 class="mt-1 truncate font-semibold text-ink-50">{a.detail || "Subagent"}</h2>
            <p class="mt-0.5 text-xs text-ink-500">{statusLabel}{#if tokenLabel} · {tokenLabel}{/if}</p>
          </div>
          <button type="button" aria-label="Close" onclick={onClose} class="grid h-8 w-8 shrink-0 place-items-center rounded-md text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-50">
            <Icon name="close" class="h-4 w-4" />
          </button>
        </header>
        <div class="min-h-0 flex-1 space-y-5 overflow-y-auto px-5 py-4">
          {#if task}
            <section>
              <h3 class="mb-1.5 text-xs font-semibold uppercase tracking-wide text-ink-500">Asked to do</h3>
              <p class="whitespace-pre-wrap break-words text-sm leading-6 text-ink-100">{task}</p>
            </section>
          {/if}
          {#if (a.steps?.length ?? 0) > 0}
            <section>
              <h3 class="mb-1.5 text-xs font-semibold uppercase tracking-wide text-ink-500">What it did</h3>
              <ol class="space-y-1.5">
                {#each a.steps ?? [] as step, i (i)}
                  <li class="flex items-start gap-2 text-sm leading-6 text-ink-300"><span class="mt-[9px] h-1.5 w-1.5 shrink-0 rounded-full bg-ink-600"></span><span class="min-w-0 break-words">{step}</span></li>
                {/each}
              </ol>
            </section>
          {/if}
          {#if a.output.trim()}
            <section>
              <h3 class="mb-1.5 text-xs font-semibold uppercase tracking-wide text-ink-500">Report</h3>
              <pre class="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-ink-800 bg-ink-950 p-3 font-mono text-[13px] leading-6 text-ink-200">{a.output}</pre>
            </section>
          {:else if a.status === "running"}
            <p class="text-sm text-ink-400">Still working — its report appears here when it finishes.</p>
          {:else if a.status === "failed"}
            <p class="text-sm text-rose-300">{a.summary || "It failed without a report."}</p>
          {/if}
        </div>
        <footer class="shrink-0 border-t border-ink-800 px-5 py-3">
          <p class="text-xs leading-5 text-ink-500">Read-only. Subagents can't be talked to or steered — this is the task, the work log and the report the main assistant got back.</p>
        </footer>
      </FocusTrap>
    </div>
  </div>
{/if}
