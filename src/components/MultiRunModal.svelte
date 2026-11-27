<script lang="ts">
  // Compare models: send one prompt to up to three OpenAI-compatible
  // endpoints at once. Every connection detail comes from this form; nothing
  // is read from saved settings or sent anywhere the user did not name.
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { RunAnswer, RunTarget } from "../lib/api/bindings";

  let { onClose }: { onClose: () => void } = $props();

  type Draft = { id: number; endpoint: string; model: string; apiKey: string };
  let nextId = 1;
  const blank = (): Draft => ({ id: nextId++, endpoint: "", model: "", apiKey: "" });

  let prompt = $state("");
  let drafts = $state<Draft[]>([blank()]);
  let answers = $state<RunAnswer[]>([]);
  let busy = $state(false);
  let ran = $state(false);
  let error = $state<string | null>(null);

  const addTarget = () => {
    if (drafts.length < 3) drafts = [...drafts, blank()];
  };
  const removeTarget = (id: number) => {
    if (drafts.length > 1) drafts = drafts.filter((d) => d.id !== id);
  };

  const run = async () => {
    if (!prompt.trim() || busy) return;
    const targets: RunTarget[] = drafts
      .filter((d) => d.endpoint.trim() && d.model.trim())
      .map((d) => ({
        label: d.model.trim(),
        endpoint: d.endpoint.trim(),
        model: d.model.trim(),
        api_key: d.apiKey.trim() || null,
        max_tokens: null,
      }));
    if (targets.length === 0) {
      error = "Add at least one target with a base URL and a model.";
      return;
    }
    busy = true;
    error = null;
    answers = [];
    try {
      answers = await api.multiRunChat(targets, prompt.trim());
      ran = true;
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };
</script>

<Modal title="Compare models" onClose={onClose} size="lg">
  <form onsubmit={(e) => { e.preventDefault(); void run(); }}>
    <label class="mb-1 block text-sm text-ink-400" for="compare-prompt">Prompt</label>
    <textarea id="compare-prompt" bind:value={prompt} rows="3" class="min-h-20 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-2 py-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="Ask the same question of each model…"></textarea>

    <div class="mt-4 flex items-center justify-between">
      <h3 class="flex items-center gap-2 text-sm font-semibold text-ink-100"><Icon name="target" class="h-4 w-4 text-ink-400" />Targets ({drafts.length}/3)</h3>
      {#if drafts.length < 3}<Button variant="subtle" onclick={addTarget}><Icon name="plus" class="h-3.5 w-3.5" />Add target</Button>{/if}
    </div>
    <p class="mt-1 text-xs text-ink-500">A base URL such as <code>https://api.example.com/v1</code>. Keys stay in this form and are used only for this run.</p>
    <div class="mt-2 space-y-2">
      {#each drafts as draft, i (draft.id)}
        <div class="rounded-lg border border-ink-700 bg-ink-950 p-3">
          <div class="mb-2 flex items-center justify-between">
            <span class="text-sm font-medium text-ink-200">Target {i + 1}</span>
            {#if drafts.length > 1}<Button variant="subtle" onclick={() => removeTarget(draft.id)}>Remove</Button>{/if}
          </div>
          <div class="grid gap-2 sm:grid-cols-3">
            <input bind:value={draft.endpoint} class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="Base URL" aria-label={`Target ${i + 1} base URL`} />
            <input bind:value={draft.model} class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="Model" aria-label={`Target ${i + 1} model`} />
            <input bind:value={draft.apiKey} type="password" class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="Key (optional)" aria-label={`Target ${i + 1} API key`} autocomplete="off" />
          </div>
        </div>
      {/each}
    </div>

    {#if error}<p class="mt-3 text-sm text-rose-300" role="alert">{error}</p>{/if}
    <div class="mt-4 flex justify-end"><Button type="submit" variant="primary" disabled={busy || !prompt.trim()}><Icon name="compare" class="h-4 w-4" />{busy ? "Running…" : "Run comparison"}</Button></div>
  </form>

  {#if ran}
    <div class="mt-5 grid gap-3 md:grid-cols-2" aria-live="polite">
      {#each answers as answer (answer.label + answer.text)}
        <div class="rounded-lg border border-ink-700 bg-ink-950 p-3">
          <h4 class="flex items-center gap-2 font-medium text-ink-100"><Icon name="sparkle" class="h-4 w-4 text-ink-400" />{answer.label}</h4>
          {#if answer.error}
            <p class="mt-2 text-sm text-rose-300">{answer.error}</p>
          {:else}
            <pre class="mt-2 max-h-72 overflow-auto whitespace-pre-wrap text-sm leading-6 text-ink-300">{answer.text || "(empty answer)"}</pre>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</Modal>
