<script lang="ts">
  // Settings → Web search: providers, optional keys, the default, and an
  // optional self-hosted Firecrawl server. Moved out of SettingsScreen.svelte
  // unchanged; the list now loads when this section opens.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { dur } from "../lib/motion";
  import type { WebProviderStatus } from "../lib/api/bindings";

  let webProviders = $state<WebProviderStatus[]>([]);
  let webKeyDraft = $state<Record<string, string>>({});
  let webBusy = $state(false);
  let webNotice = $state<string | null>(null);

  // Web search: a Firecrawl server on this computer
  let firecrawlUrl = $state("");
  const saveFirecrawlUrl = async () => {
    webBusy = true;
    webNotice = null;
    try {
      const value = firecrawlUrl.trim();
      await api.webSetFirecrawlUrl(value || null);
      await loadWebProviders();
      webNotice = value ? `Firecrawl now uses your server at ${value}.` : "Firecrawl uses its own service again.";
    } catch (e: unknown) {
      webNotice = errorMessage(e);
    } finally {
      webBusy = false;
    }
  };

  const loadWebProviders = async () => {
    try {
      webProviders = await api.webProviders();
      firecrawlUrl = webProviders.find((p) => p.id === "firecrawl")?.base_url ?? "";
    } catch (e: unknown) {
      console.error(e);
      webProviders = [];
    }
  };

  const labelOf = (id: string) => webProviders.find((provider) => provider.id === id)?.label ?? id;

  const saveWebKey = async (id: string) => {
    webBusy = true;
    webNotice = null;
    try {
      const value = webKeyDraft[id]?.trim() ?? "";
      await api.webSetKey(id, value ? value : null);
      webKeyDraft = { ...webKeyDraft, [id]: "" };
      await loadWebProviders();
      webNotice = `${labelOf(id)} key ${value ? "saved" : "cleared"}.`;
    } catch (e: unknown) {
      webNotice = errorMessage(e);
    } finally {
      webBusy = false;
    }
  };

  const setWebDefault = async (id: string) => {
    webBusy = true;
    webNotice = null;
    try {
      await api.webSetDefault(id);
      await loadWebProviders();
      webNotice = `${labelOf(id)} is now the default search provider.`;
    } catch (e: unknown) {
      webNotice = errorMessage(e);
    } finally {
      webBusy = false;
    }
  };

  onMount(() => {
    void loadWebProviders();
  });
</script>

  <div>
    <h2 class="text-xl font-semibold text-ink-50">Web search</h2>
    <p class="mt-1 text-sm text-ink-400">Every provider works without a key. LocalAide searches only when you press Search, or when you turn on Web in the chat box.</p>
    {#if webNotice}<div class="mt-4 rounded-md border border-ink-700 bg-ink-900 p-3 text-sm text-ink-300" in:slide={{ duration: dur(150) }}>{webNotice}</div>{/if}
    <div class="mt-6 space-y-2">
      {#each webProviders as provider (provider.id)}
<div class={`rounded-lg border p-4 transition-colors ${provider.is_default ? "border-ink-600 bg-veil/5" : "border-ink-700 bg-ink-900"}`}>
          <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-semibold text-ink-50">{provider.label}</span>
                {#if provider.is_default}<StatusBadge label="Default" tone="ok" dot />{/if}
                <span class="rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs font-semibold text-emerald-300">{provider.key_set ? "Your key" : "No key needed"}</span>
                <span class="text-xs text-ink-500">{provider.supports === "crawl" ? "Reads pages" : provider.supports === "search" ? "Search" : "Search and reads pages"}</span>
              </div>
              <p class="mt-1 text-sm text-ink-400">{provider.note}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              {#if provider.takes_key}
                <input type="password" bind:value={webKeyDraft[provider.id]} placeholder={provider.key_set ? "Replace key" : "Optional key"} autocomplete="off" class="h-9 w-44 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" />
                <Button variant="secondary" disabled={webBusy} onclick={() => void saveWebKey(provider.id)}>{webKeyDraft[provider.id]?.trim() || !provider.key_set ? "Save key" : "Remove key"}</Button>
              {/if}
              {#if provider.supports.includes("search") && !provider.is_default}<Button variant="subtle" disabled={webBusy} onclick={() => void setWebDefault(provider.id)}>Make default</Button>{/if}
            </div>
          </div>
          {#if provider.id === "firecrawl"}
            <div class="mt-3 border-t border-ink-800 pt-3">
              <label class="block text-sm text-ink-400" for="firecrawl-url">Your own Firecrawl server (optional)</label>
              <div class="mt-1 flex flex-col gap-2 sm:flex-row">
                <input id="firecrawl-url" bind:value={firecrawlUrl} placeholder="http://localhost:3002" class="h-9 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
                <Button variant="secondary" disabled={webBusy} onclick={() => void saveFirecrawlUrl()}>{firecrawlUrl.trim() ? "Use this server" : "Use Firecrawl's service"}</Button>
              </div>
              <p class="mt-1.5 text-xs text-ink-500">Run Firecrawl yourself (for example with its Docker setup) and nothing goes to Firecrawl's servers. Leave empty to use their keyless free tier.</p>
            </div>
          {/if}
        </div>
      {:else}
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-5 text-sm text-ink-400">The provider list is not ready. Restart the app and try again.</div>
      {/each}
    </div>
    <p class="mt-4 text-sm text-ink-400">Keys stay in this app's local settings and are sent only to their own provider. Without a key, free limits apply per computer. Backups leave keys out unless you choose to include them.</p>
  </div>
