<script lang="ts">
  // Local HTML panels contributed by plugins. The `plugin_list` command is
  // added by another stream; builds without it simply report "unavailable"
  // rather than inventing panels. Each panel runs in an iframe sandboxed to
  // allow-scripts only — no same-origin, no app access.
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { PluginDescriptor } from "../lib/api/bindings";

  let plugins = $state<PluginDescriptor[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let unavailable = $state(false);

  const load = async () => {
    loading = true;
    error = null;
    unavailable = false;
    try {
      plugins = await api.pluginList();
    } catch (e: unknown) {
      // A build without the command has nothing to show; say so plainly.
      plugins = [];
      unavailable = true;
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  };

  const labelOf = (p: PluginDescriptor) => p.label ?? p.name ?? p.id;
  const localHtml = (p: PluginDescriptor) => /\.html?$/i.test(p.path);

  onMount(() => { void load(); });
</script>

<section class="mt-9" aria-labelledby="plugin-panels-title">
  <h3 id="plugin-panels-title" class="mb-2 flex items-center gap-2 text-[15px] font-semibold text-ink-50">
    <Icon name="plug" class="h-4 w-4 text-ember-300" />Plugin panels
  </h3>
  <p class="mb-3 text-sm text-ink-400">
    HTML panels listed by plugins installed on this computer. Each one runs in a sandboxed frame:
    scripts may run, but the page cannot read LocalAide's data or reach the network through the app.
  </p>

  {#if error}
    <p class="mb-3 text-sm text-rose-300" role="status">
      {unavailable ? "Plugin panels are not available in this build." : error}
    </p>
  {/if}

  <div class="mb-3 flex justify-end"><Button variant="subtle" disabled={loading} onclick={() => void load()}>{loading ? "Refreshing…" : "Refresh"}</Button></div>

  {#if plugins.length === 0}
    <div class="rounded-xl border border-dashed border-ink-700 p-6 text-center">
      <p class="text-sm text-ink-300">{unavailable ? "No plugin list command is registered in this build." : loading ? "Looking for plugin panels…" : "No plugin panels are installed."}</p>
      <p class="mt-1 text-xs text-ink-500">Panels are local HTML files; nothing is downloaded here.</p>
    </div>
  {:else}
    <div class="grid gap-3 lg:grid-cols-2">
      {#each plugins as plugin (plugin.id)}
        <div class="overflow-hidden rounded-lg border border-ink-700 bg-ink-900">
          <div class="flex items-center gap-2 border-b border-ink-800 px-3 py-2 text-sm text-ink-200">
            <Icon name="plug" class="h-4 w-4 text-ember-300" />
            <span class="truncate font-semibold">{labelOf(plugin)}</span>
            <span class="ml-auto text-xs text-ink-500">Sandboxed</span>
          </div>
          {#if localHtml(plugin)}
            <iframe
              src={convertFileSrc(plugin.path)}
              title={`Plugin panel: ${labelOf(plugin)}`}
              sandbox="allow-scripts"
              referrerpolicy="no-referrer"
              class="block h-72 w-full bg-white"
            ></iframe>
          {:else}
            <p class="p-4 text-sm text-ink-400">Skipped: {plugin.path} is not an HTML file.</p>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>
