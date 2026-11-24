<script lang="ts">
  // Settings → Tools and MCP. Saved personas are plain markdown files managed
  // by the agents_list / agents_save commands; this card edits them locally.
  import { onMount } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { Agent } from "../lib/api/bindings";

  let agents = $state<Agent[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  let name = $state("");
  let model = $state("");
  let toolsText = $state("");
  let prompt = $state("");

  const load = async () => {
    error = null;
    try {
      agents = await api.agentsList();
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const save = async () => {
    if (!name.trim() || !prompt.trim() || busy) return;
    busy = true;
    error = null;
    notice = null;
    try {
      const tools = toolsText.split(",").map((t) => t.trim()).filter(Boolean);
      const saved = await api.agentsSave(name.trim(), model.trim() || null, tools, prompt.trim());
      name = "";
      model = "";
      toolsText = "";
      prompt = "";
      notice = `Saved agent ${saved.name}.`;
      await load();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  onMount(() => { void load(); });
</script>

<div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
  <h3 class="flex items-center gap-2 font-semibold text-ink-50"><Icon name="users" class="h-4 w-4 text-ember-300" />Agents</h3>
  <p class="mt-0.5 text-sm text-ink-400">Local personas saved on this computer. They are markdown files you can edit outside the app.</p>

  {#if error}<p class="mt-3 text-sm text-rose-300" role="alert">{error}</p>{/if}
  {#if notice}<p class="mt-3 text-sm text-emerald-300" role="status">{notice}</p>{/if}

  <div class="mt-3 rounded-lg border border-ink-700 bg-ink-950 p-3">
    {#if agents.length === 0}
      <p class="text-sm text-ink-400">No saved agents yet. Add one below.</p>
    {:else}
      <ul class="space-y-2" aria-label="Saved agents">
        {#each agents as agent (agent.path || agent.name)}
          <li class="flex flex-wrap items-start justify-between gap-2 border-b border-ink-800 pb-2 last:border-0 last:pb-0">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-medium text-ink-100">{agent.name}</span>
                {#if agent.model}<span class="rounded bg-veil/5 px-1.5 py-0.5 text-xs text-ink-400">{agent.model}</span>{/if}
              </div>
              {#if agent.tools.length}<p class="mt-0.5 text-xs text-ink-500">Tools: {agent.tools.join(", ")}</p>{/if}
              <p class="mt-0.5 line-clamp-2 text-sm text-ink-300">{agent.prompt}</p>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <form class="mt-3 grid gap-2" onsubmit={(e) => { e.preventDefault(); void save(); }}>
    <div class="grid gap-2 sm:grid-cols-2">
      <label class="block text-sm">
        <span class="mb-1 block text-ink-400">Name</span>
        <input bind:value={name} class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="Code reviewer" autocomplete="off" />
      </label>
      <label class="block text-sm">
        <span class="mb-1 block text-ink-400">Model (optional)</span>
        <input bind:value={model} class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="gpt-4o-mini" autocomplete="off" />
      </label>
    </div>
    <label class="block text-sm">
      <span class="mb-1 block text-ink-400">Tools (comma-separated)</span>
      <input bind:value={toolsText} class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="read_file, write_file" autocomplete="off" />
    </label>
    <label class="block text-sm">
      <span class="mb-1 block text-ink-400">Prompt</span>
      <textarea bind:value={prompt} rows="3" class="min-h-20 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-2 py-2 text-sm text-ink-100 placeholder:text-ink-400" placeholder="What should this agent do?"></textarea>
    </label>
    <div><Button type="submit" variant="primary" disabled={busy || !name.trim() || !prompt.trim()}><Icon name="plus" class="h-4 w-4" />{busy ? "Saving…" : "Save agent"}</Button></div>
  </form>
</div>
