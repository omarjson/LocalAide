<script lang="ts">
  // One online provider in Settings. Every provider uses this card, so the
  // name, status, where data goes, its models and its actions are always in
  // the same place.
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import Switch from "./Switch.svelte";
  import type { CloudProviderDto } from "../lib/api/bindings";

  interface Props {
    provider: CloudProviderDto;
    /** Online models are switched on in Settings. */
    available: boolean;
    /** The provider needs a key (from its preset) and none is saved. */
    needsKey?: boolean;
    busy?: boolean;
    testing?: boolean;
    onToggle: (on: boolean) => void;
    onTest: () => void;
    onEdit: () => void;
    onRemove: () => void;
  }
  let { provider, available, needsKey = false, busy = false, testing = false, onToggle, onTest, onEdit, onRemove }: Props = $props();

  const host = $derived.by(() => {
    try {
      return new URL(provider.base_url).host;
    } catch {
      return provider.base_url;
    }
  });
  const local = $derived(/^(localhost|127\.0\.0\.1|\[::1\])(:\d+)?$/.test(host));
  const ago = (secs: number) => {
    const s = Math.max(0, Date.now() / 1000 - secs);
    if (s < 60) return "just now";
    if (s < 3600) return `${Math.round(s / 60)} min ago`;
    if (s < 86400) return `${Math.round(s / 3600)} h ago`;
    return new Date(secs * 1000).toLocaleDateString();
  };
  const status = $derived.by(() => {
    if (testing) return { label: "Checking…", tone: "idle" as const };
    if (provider.check?.ok) return { label: "Connected", tone: "ok" as const };
    if (provider.check && !provider.check.ok) return { label: "Not reachable", tone: "bad" as const };
    if (needsKey) return { label: "Needs a key", tone: "warn" as const };
    return { label: "Not checked", tone: "idle" as const };
  });
  const extra = $derived(Math.max(0, provider.model_count - provider.sample_models.length));
</script>

<article class={`card ${available ? "" : "card-off"}`} aria-label={provider.label}>
  <header class="flex items-start gap-3">
    <span class="mark" aria-hidden="true"><Icon name={local ? "computer" : "cloud"} class="h-4 w-4" /></span>
    <div class="min-w-0 flex-1">
      <div class="flex flex-wrap items-center gap-x-2.5 gap-y-1">
        <h4 class="truncate text-[15px] font-semibold text-ink-50">{provider.label}</h4>
        {#if available}
          <span class={`status status-${status.tone}`}><span class="dot" aria-hidden="true"></span>{status.label}</span>
        {:else}
          <span class="status status-idle">Not available</span>
        {/if}
      </div>
      <p class="mt-0.5 flex flex-wrap gap-x-3 text-[13px] text-ink-400">
        <span>{local ? "Runs on this computer at" : "Sends to"} <span class="text-ink-200">{host}</span></span>
        <span>{provider.api_key_hint ? `Key ${provider.api_key_hint}` : needsKey ? "No key saved" : "No key"}</span>
        {#if provider.check}<span title={new Date(provider.check.at * 1000).toLocaleString()}>Checked {ago(provider.check.at)}</span>{/if}
      </p>
    </div>
  </header>

  {#if provider.check && !provider.check.ok && provider.check.error && available}
    <p class="mt-3 rounded-md bg-rose-500/10 px-3 py-2 text-[13px] leading-5 text-rose-300">{provider.check.error}</p>
  {/if}

  <div class="mt-3">
    {#if provider.sample_models.length > 0}
      <ul class="flex flex-wrap gap-1.5" aria-label={`Models from ${provider.label}`}>
        {#each provider.sample_models as m (m)}
          <li class={`chip ${m === provider.model ? "chip-default" : ""}`} title={m === provider.model ? "Default model" : m}>{m}</li>
        {/each}
        {#if extra > 0}<li class="chip chip-more">{extra.toLocaleString()} more</li>{/if}
      </ul>
    {:else}
      <p class="text-[13px] text-ink-500">Default model <span class="font-mono text-ink-300">{provider.model}</span>. Test the connection to list the other models.</p>
    {/if}
  </div>

  <footer class="mt-4 flex flex-wrap items-center gap-2 border-t border-ink-800 pt-3">
    <span class="flex items-center gap-2.5 text-sm text-ink-300" title="Show this provider's models in the chat's model menu">
      <Switch checked={available && provider.enabled} label={`Show ${provider.label} in the model menu`} disabled={busy || !available} onchange={onToggle} />
      In the model menu
    </span>
    <span class="flex-1"></span>
    <Button variant="secondary" disabled={busy || testing || !available} onclick={onTest}>
      <Icon name="refresh" class={`h-4 w-4 ${testing ? "animate-spin" : ""}`} />{testing ? "Testing…" : "Test connection"}
    </Button>
    <Button variant="subtle" disabled={busy} onclick={onEdit}>Edit</Button>
    <Button variant="danger" disabled={busy} onclick={onRemove}>Remove</Button>
  </footer>
</article>

<style>
  .card {
    border-radius: 12px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
    padding: 16px;
    transition: opacity 150ms ease-out;
  }
  .card-off .mark,
  .card-off header,
  .card-off ul {
    opacity: 0.5;
    filter: grayscale(1);
  }
  .mark {
    display: grid;
    height: 32px;
    width: 32px;
    flex-shrink: 0;
    place-items: center;
    border-radius: 8px;
    background: var(--color-ink-800);
    color: var(--color-ink-200);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 9999px;
    padding: 1px 8px;
    font-size: 12px;
    font-weight: 600;
    line-height: 18px;
    background: var(--color-ink-800);
    color: var(--color-ink-300);
  }
  .status .dot {
    height: 6px;
    width: 6px;
    border-radius: 9999px;
    background: currentColor;
  }
  .status-ok {
    background: color-mix(in oklab, #10b981 14%, transparent);
    color: #34d399;
  }
  .status-warn {
    background: color-mix(in oklab, #f59e0b 14%, transparent);
    color: var(--color-amber-300);
  }
  .status-bad {
    background: color-mix(in oklab, #f43f5e 14%, transparent);
    color: #fda4af;
  }
  :global([data-theme="light"]) .status-ok {
    color: #047857;
  }
  :global([data-theme="light"]) .status-bad {
    color: #be123c;
  }
  .chip {
    max-width: 16rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-radius: 6px;
    background: var(--color-ink-800);
    padding: 2px 8px;
    font-size: 12px;
    line-height: 20px;
    color: var(--color-ink-200);
  }
  .chip-default {
    box-shadow: inset 0 0 0 1px var(--color-ink-600);
  }
  .chip-more {
    background: transparent;
    color: var(--color-ink-500);
  }
</style>
