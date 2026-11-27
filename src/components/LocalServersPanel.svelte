<script lang="ts">
  // Servers already running on this computer. One probe of the usual ports
  // finds them, and adding one is a click: name, address and model list fill
  // themselves in, so nothing has to be typed or looked up.
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { CloudProviderDto, FoundServer } from "../lib/api/bindings";

  interface Props {
    providers: CloudProviderDto[];
    /** Adds the found server through the provider form, prefilled. */
    onAdd: (server: FoundServer) => void;
  }
  let { providers, onAdd }: Props = $props();

  let found = $state<FoundServer[]>([]);
  let scanning = $state(false);
  let scanned = $state(false);
  let error = $state<string | null>(null);

  const scan = async () => {
    scanning = true;
    error = null;
    try {
      found = await api.discoverLocalServers();
      scanned = true;
    } catch (e: unknown) {
      error = errorMessage(e);
      found = [];
    } finally {
      scanning = false;
    }
  };

  const saved = (server: FoundServer) =>
    providers.some((p) => p.base_url.replace(/\/+$/, "") === server.base_url);
</script>

<div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div class="min-w-0">
      <h3 class="flex items-center gap-2 text-[15px] font-semibold text-ink-50">
        <Icon name="computer" class="h-4 w-4 text-ink-400" />
        Local servers
        {#if found.length > 0}<span class="rounded-full bg-ink-800 px-2 py-0.5 text-xs font-medium tabular-nums text-ink-300">{found.length}</span>{/if}
      </h3>
      <p class="mt-1 text-sm leading-6 text-ink-400">
        Ollama, LM Studio, llama.cpp, MLX, vLLM, LiteLLM and more. Look for what is already running on the usual ports, then add it in one click. Nothing leaves this computer.
      </p>
    </div>
    <Button variant="secondary" disabled={scanning} onclick={() => void scan()}>
      <Icon name="refresh" class={`h-4 w-4 ${scanning ? "animate-spin" : ""}`} />
      {scanning ? "Looking…" : scanned ? "Look again" : "Find servers"}
    </Button>
  </div>

  {#if error}
    <p class="mt-3 flex items-center gap-2 text-sm text-rose-300" role="status"><Icon name="alert" class="h-4 w-4 shrink-0" />{error}</p>
  {:else if scanned && found.length === 0}
    <p class="mt-3 text-sm text-ink-400">Nothing answered on the usual ports. Start your server, then look again.</p>
  {:else if found.length > 0}
    <ul class="mt-3 grid gap-1.5 sm:grid-cols-2">
      {#each found as server (server.base_url)}
        <li class="flex items-center gap-2 rounded-md border border-ink-800 bg-ink-950 px-2.5 py-2">
          <span class="grid h-7 w-7 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-300"><Icon name="models" class="h-3.5 w-3.5" /></span>
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-ink-100">{server.label}</p>
            <p class="truncate font-mono text-xs text-ink-500">{server.base_url}</p>
          </div>
          {#if saved(server)}
            <StatusBadge label="Added" tone="ok" dot />
          {:else}
            <Button variant="subtle" onclick={() => onAdd(server)}>Add</Button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

