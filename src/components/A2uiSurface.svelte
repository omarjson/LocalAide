<script lang="ts">
  // A ```a2ui block in an answer: the surfaces it created, drawn from the
  // chat's A2UI state, plus any problems in plain words and a way to ask
  // the model to fix them (sent as an A2UI `error` message).
  import A2uiNode from "./A2uiNode.svelte";
  import Icon from "./Icon.svelte";
  import { errorReport, rootScope } from "../lib/a2ui";
  import { useA2ui } from "../lib/a2ui.svelte";

  interface Props {
    source: string;
    /** Sends a message as the user's next one: an action or an error report. */
    onAction?: (message: string) => void;
  }
  let { source, onAction }: Props = $props();

  const chat = useA2ui();
  const result = $derived(chat?.results[source.trimEnd()] ?? null);
  const surfaces = $derived((result?.created ?? []).map((id) => chat?.surfaces[id]).filter((s) => !!s && !s.deleted));
  const problems = $derived([...(result?.problems ?? []), ...surfaces.flatMap((s) => s?.problems ?? []), ...surfaces.filter((s) => s && !s.components.root).map((s) => `Surface "${s?.id}" has no component with the id "root".`)]);
  let reported = $state(false);
</script>

{#if !chat}
  <p class="text-sm text-ink-400">An interactive form is in this answer. Open the chat to use it.</p>
{:else if !result}
  <div class="flex items-center gap-2 rounded-xl border border-ink-700 bg-ink-950 px-3 py-2.5 text-sm text-ink-400" role="status"><span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500"></span>Building the form…</div>
{:else}
  {#each surfaces as surface (surface?.id)}
    {#if surface && surface.components.root}
      <section class="a2ui-surface" aria-label="Interactive form">
        <A2uiNode {surface} id="root" scope={rootScope(surface.data)} {onAction} />
      </section>
    {/if}
  {/each}
  {#if result.created.length > 0 && surfaces.length === 0 && problems.length === 0}
    <p class="text-xs text-ink-500">This form was closed by a later answer.</p>
  {/if}
  {#if problems.length > 0}
    <div class="rounded-xl border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-200" role="alert">
      <div class="flex items-start gap-2">
        <Icon name="alert" class="mt-0.5 h-4 w-4 shrink-0" />
        <div class="min-w-0 flex-1">
          <p class="font-medium">Part of this form could not be drawn.</p>
          <ul class="mt-1 list-disc pl-4 text-xs leading-5 text-amber-200/90">
            {#each problems.slice(0, 6) as problem, i (i)}<li>{problem}</li>{/each}
          </ul>
        </div>
        {#if onAction && !reported}
          <button type="button" class="shrink-0 rounded-md border border-amber-500/40 px-2 py-1 text-xs hover:bg-amber-500/15" onclick={() => { reported = true; onAction?.(errorReport(result.created[0] ?? "unknown", problems)); }}>Ask to fix it</button>
        {/if}
      </div>
    </div>
  {/if}
{/if}

<style>
  .a2ui-surface {
    border: 1px solid var(--color-edge);
    border-radius: 0.9rem;
    background: var(--color-ink-950);
    padding: 0.5rem;
    font-size: 0.875rem;
    line-height: 1.5rem;
  }
</style>
