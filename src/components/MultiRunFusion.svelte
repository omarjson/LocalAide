<script lang="ts">
  // Compare models with a result choice: keep one answer or fuse all of them.
  // Fusion is local string joining — no backend change and no extra network
  // call beyond the compare run itself, whose endpoints the user typed in.
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { fuseRunAnswers } from "../lib/notifications.svelte";
  import type { RunAnswer, RunTarget } from "../lib/api/bindings";

  let { onClose }: { onClose: () => void } = $props();

  type Draft = { id: number; endpoint: string; model: string; apiKey: *** };
  let nextId = 1;
  const blank = (): Draft => ({ id: nextId++, endpoint: "", model: "", apiKey: "" });

  let prompt = $state("");
  let drafts = $state<Draft[]>([blank()]);
  let answers = $state<RunAnswer[]>([]);
  let busy = $state(false);
  let ran = $state(false);
  let error = $state<string | null>(null);
  let choice = $state<"keep" | "fuse">("fuse");
  let keptLabel = $state<string | null>(null);
  let copied = $state(false);

  const addTarget = () => {
    if (drafts.length < 3) drafts = [...drafts, blank()];
  };
  const removeTarget = (id: number) => {
    if (drafts.length > 1) drafts = drafts.filter((d) => d.id !== id);
  };

  const usable = $derived(answers.filter((a) => !a.error && a.text.trim()));
  const best = $derived(
    [...usable].sort((a, b) => b.text.trim().length - a.text.trim().length)[0] ?? null,
  );
  const kept = $derived(
    usable.find((a) => a.label === keptLabel) ?? best,
  );
  const fused = $derived(fuseRunAnswers(answers));
  const result = $derived(choice === "fuse" ? fused : (kept?.text.trim() ?? ""));

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
    copied = false;
    answers = [];
    keptLabel = null;
    try {
      answers = await api.multiRunChat(targets, prompt.trim());
      ran = true;
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const copy = async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result);
      copied = true;
    } catch {
      copied = false;
    }
  };
</script>

<Modal title="Compare & fuse models" onClose={onClose} size="lg">
  <form onsubmit={(e) => { e.preventDefault(); void run(); }}>
    <label class="mb-1 block text-sm text-ink-400" for="fusion-prompt">Prompt</label>
    <textarea id="fusion-prompt" bind:value={prompt} rows="3" class="min-h-20 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-2 py-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="Ask the same question of each model…"></textarea>

    <div class="mt-4 flex items-center justify-between">
      <h3 class="flex items-center gap-2 text-sm font-semibold text-ink-100"><Icon name="target" class="h-4 w-4 text-ember-300" />Targets ({drafts.length}/3)</h3>
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
    <div class="mt-5" aria-live="polite">
      <h3 class="text-sm font-semibold text-ink-100">Answers</h3>
      <div class="mt-2 grid gap-3 md:grid-cols-2">
        {#each answers as answer (answer.label + answer.text)}
          <div class={`rounded-lg border p-3 ${answer.label === kept?.label ? "border-ember-500/50 bg-ember-500/5" : "border-ink-700 bg-ink-950"}`}>
            <div class="flex items-center justify-between gap-2">
              <h4 class="flex items-center gap-2 font-medium text-ink-100"><Icon name="sparkle" class="h-4 w-4 text-ember-300" />{answer.label}</h4>
              {#if !answer.error && answer.text.trim()}
                <Button variant="subtle" onclick={() => { keptLabel = answer.label; choice = "keep"; }}>Keep this</Button>
              {/if}
            </div>
            {#if answer.error}
              <p class="mt-2 text-sm text-rose-300">{answer.error}</p>
            {:else}
              <pre class="mt-2 max-h-56 overflow-auto whitespace-pre-wrap text-sm leading-6 text-ink-300">{answer.text || "(empty answer)"}</pre>
            {/if}
          </div>
        {/each}
      </div>

      <div class="mt-6 rounded-lg border border-ink-700 bg-ink-950 p-4">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <h3 class="flex items-center gap-2 text-sm font-semibold text-ink-100"><Icon name="merge" class="h-4 w-4 text-ember-300" />Result</h3>
          <div class="inline-flex rounded-md border border-ink-700 p-0.5" role="group" aria-label="Result mode">
            <button type="button" onclick={() => choice = "keep"} aria-pressed={choice === "keep"} class={`rounded px-3 py-1 text-xs font-medium transition-colors ${choice === "keep" ? "bg-ember-500/15 text-ember-200" : "text-ink-400 hover:text-ink-100"}`}>Keep best</button>
            <button type="button" onclick={() => choice = "fuse"} aria-pressed={choice === "fuse"} class={`rounded px-3 py-1 text-xs font-medium transition-colors ${choice === "fuse" ? "bg-ember-500/15 text-ember-200" : "text-ink-400 hover:text-ink-100"}`}>Fuse answers</button>
          </div>
        </div>
        <p class="mt-2 text-xs text-ink-500">
          {#if choice === "fuse"}
            All answers joined under their model names, separated by a rule. Errors are left out.
          {:else}
            One answer kept — {kept?.label ?? "none"} — the longest valid answer unless you picked one above.
          {/if}
        </p>
        {#if result}
          <pre class="mt-3 max-h-72 overflow-auto whitespace-pre-wrap rounded-md border border-ink-800 bg-ink-900 p-3 text-sm leading-6 text-ink-200">{result}</pre>
          <div class="mt-2 flex justify-end">
            <Button variant="subtle" onclick={() => void copy()}>{copied ? "Copied" : "Copy result"}</Button>
          </div>
        {:else}
          <p class="mt-3 text-sm text-ink-500">No answer to show yet.</p>
        {/if}
      </div>
    </div>
  {/if}
</Modal>
