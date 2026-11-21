<script lang="ts">
  // Settings → Share my assistant: start and stop a public link, with the
  // two permission links shown once.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { dur } from "../lib/motion";
  import { formatBytes } from "../lib/format";
  import { refreshShare, setShare, share } from "../lib/share.svelte";
  import type { PairInfo, ShareStarted } from "../lib/api/bindings";

  let links = $state<ShareStarted | null>(null);
  let pair = $state<PairInfo | null>(null);
  let busy = $state<"start" | "local" | "stop" | "fetch" | null>(null);
  let pct = $state<number | null>(null);
  let error = $state<string | null>(null);
  let copied = $state<string | null>(null);

  onMount(() => {
    void refreshShare();
    // The QR and pairing code are cheap and local; refresh whenever this
    // screen opens, so a session started elsewhere still shows them.
    void loadPair();
  });

  const loadPair = async () => {
    try {
      pair = await api.sharePair();
    } catch {
      pair = null;
    }
  };

  const status = $derived(share.status);
  const tool = $derived(status?.cloudflared);

  const start = async (tunnel: boolean) => {
    error = null;
    busy = tunnel ? "start" : "local";
    try {
      links = await api.shareStart(tunnel);
      setShare(links.status);
      void loadPair();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = null;
    }
  };

  const stop = async () => {
    busy = "stop";
    try {
      setShare(await api.shareStop());
      links = null;
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = null;
    }
  };

  const fetchTool = async () => {
    error = null;
    busy = "fetch";
    pct = 0;
    try {
      await api.fetchCloudflared((e) => (pct = Math.round(e.percent)));
      await refreshShare();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = null;
      pct = null;
    }
  };

  const copy = async (what: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      copied = what;
      setTimeout(() => (copied = null), 1600);
    } catch {
      error = "Copy failed. Select the link and copy it.";
    }
  };
  const since = (secs: number | null | undefined) => (secs ? new Date(secs * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "");
</script>

<div>
  <h2 class="text-xl font-semibold text-ink-50">Share my assistant</h2>
  <p class="mt-1 max-w-[65ch] text-sm leading-6 text-ink-400">Open your assistant from another computer or show it to someone, through a link. It stays off until you start it, and the link stops working when you stop sharing.</p>

  <div class="mt-6 grid gap-3 sm:grid-cols-2">
    <div class="perm">
      <span class="perm-icon"><Icon name="eye" class="h-4 w-4" /></span>
      <div><p class="font-semibold text-ink-50">Viewer</p><p class="text-[13px] leading-5 text-ink-400">Reads your saved conversations. Cannot chat or run anything.</p></div>
    </div>
    <div class="perm">
      <span class="perm-icon"><Icon name="chat" class="h-4 w-4" /></span>
      <div><p class="font-semibold text-ink-50">Remote user</p><p class="text-[13px] leading-5 text-ink-400">Also chats and runs Check my setup. Commands and changes ask first, every time, and are logged here.</p></div>
    </div>
  </div>

  {#if status?.on}
    <section class="on mt-6" aria-live="polite">
      <div class="flex flex-wrap items-center gap-3">
        <span class="pulse" aria-hidden="true"></span>
        <h3 class="text-[15px] font-semibold text-ink-50">Sharing is on</h3>
        <span class="text-sm text-ink-400">since {since(status.started_at)}</span>
        <span class="flex-1"></span>
        <Button variant="danger" disabled={busy !== null} onclick={() => void stop()}>{busy === "stop" ? "Stopping…" : "Stop sharing"}</Button>
      </div>
      <p class="mt-2 break-all font-mono text-[13px] text-ink-200">{status.url}</p>
      <dl class="mt-3 flex flex-wrap gap-x-6 gap-y-1 text-sm">
        <div class="flex gap-1.5"><dt class="text-ink-500">Requests</dt><dd class="tabular-nums text-ink-100">{status.requests.toLocaleString()}</dd></div>
        <div class="flex gap-1.5"><dt class="text-ink-500">Refused</dt><dd class={`tabular-nums ${status.refused > 0 ? "text-ember-200" : "text-ink-100"}`}>{status.refused.toLocaleString()}</dd></div>
        <div class="flex gap-1.5"><dt class="text-ink-500">Last visit</dt><dd class="text-ink-100">{status.last_seen ? since(status.last_seen) : "none yet"}</dd></div>
      </dl>

      {#if links}
        <div class="mt-4 space-y-2" transition:slide={{ duration: dur(150) }}>
          <p class="flex items-center gap-2 text-sm font-medium text-ember-200"><Icon name="key" class="h-4 w-4" />Copy the links now. They are shown only once; to get new ones, stop and share again.</p>
          {#each [["Viewer", links.viewer_link], ["Remote user", links.remote_link]] as [label, link] (label)}
            <div class="link">
              <span class="w-24 shrink-0 text-sm font-medium text-ink-200">{label}</span>
              <input readonly value={link} aria-label={`${label} link`} class="min-w-0 flex-1 bg-transparent font-mono text-[12.5px] text-ink-300 outline-none" onfocus={(e) => (e.currentTarget as HTMLInputElement).select()} />
              <Button variant="secondary" onclick={() => void copy(label, link)}><Icon name={copied === label ? "check" : "copy"} class="h-4 w-4" />{copied === label ? "Copied" : "Copy"}</Button>
            </div>
          {/each}
        </div>
      {:else}
        <p class="mt-4 text-sm text-ink-400">The links were shown when sharing started. Stop and share again for new ones; the old ones stop working.</p>
      {/if}

      {#if pair && pair.on}
        <!-- Pair your phone: scan the QR, or open the page and match codes. -->
        <div class="mt-5 grid gap-4 rounded-xl border border-ink-700 bg-ink-950 p-4 sm:grid-cols-[auto_1fr]">
          <div class="qr mx-auto w-40 self-start rounded-lg bg-white p-2" aria-hidden="true">
            {@html pair.qr_svg}
          </div>
          <div class="min-w-0">
            <h4 class="flex items-center gap-2 font-semibold text-ink-50"><Icon name="link" class="h-4 w-4 text-ember-300" />Pair your phone</h4>
            <p class="mt-1 text-sm leading-6 text-ink-400">Scan the code with your phone's camera, or open the Remote link and look for the small code it shows. Match it against this one:</p>
            <p class="pair-code mt-2 w-fit">{pair.pair_code}</p>
            <p class="mt-2 text-sm text-ink-500">Matching codes mean the phone and this computer are the same session. Codes change every time sharing restarts.</p>
          </div>
        </div>
      {/if}
    </section>
  {:else}
    <section class="mt-6 rounded-xl border border-ink-700 bg-ink-900 p-4">
      {#if tool && !tool.installed}
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
          <Icon name="download" class="hidden h-5 w-5 shrink-0 text-ember-300 sm:block" />
          <p class="flex-1 text-sm leading-6 text-ink-300">A public link needs <b class="font-semibold text-ink-100">cloudflared</b>, Cloudflare's tunnel program. LocalAide downloads version {tool.version} from Cloudflare's GitHub ({formatBytes(tool.download_bytes ?? 0)}) and checks its checksum before using it.</p>
          {#if tool.available}
            <Button variant="primary" disabled={busy !== null} onclick={() => void fetchTool()}>{busy === "fetch" ? `Downloading ${pct ?? 0}%` : "Download cloudflared"}</Button>
          {/if}
        </div>
        {#if !tool.available}<p class="mt-2 text-sm text-ink-400">There is no pinned build for this computer. Install cloudflared from Cloudflare and it will be found.</p>{/if}
      {:else}
        <div class="flex flex-wrap items-center gap-3">
          <p class="flex-1 text-sm leading-6 text-ink-300">Starts a Cloudflare quick tunnel to this app and makes a Viewer link and a Remote user link. No Cloudflare account is needed.</p>
          <Button variant="subtle" disabled={busy !== null} onclick={() => void start(false)}>{busy === "local" ? "Starting…" : "Try on this computer only"}</Button>
          <Button variant="primary" disabled={busy !== null || !tool?.installed} onclick={() => void start(true)}><Icon name="link" class="h-4 w-4" />{busy === "start" ? "Opening the tunnel…" : "Share my assistant"}</Button>
        </div>
      {/if}
    </section>
  {/if}

  {#if error}<p class="mt-3 rounded-md bg-rose-500/10 px-3 py-2 text-sm text-rose-300" role="alert">{error}</p>{/if}

  <ul class="mt-6 space-y-1.5 text-sm text-ink-400">
    <li class="flex gap-2"><Icon name="shield" class="mt-0.5 h-4 w-4 shrink-0 text-ink-500" />Every request without a valid token is refused. Tokens are made on this computer and kept only as hashes.</li>
    <li class="flex gap-2"><Icon name="shield" class="mt-0.5 h-4 w-4 shrink-0 text-ink-500" />Tool calls from a link always ask, even when this app auto-approves; blocked commands never run. Each one is in the Activity log, marked as coming from the link.</li>
    <li class="flex gap-2"><Icon name="shield" class="mt-0.5 h-4 w-4 shrink-0 text-ink-500" />Answers come from the model this app is using. If that is an online model, the link's messages go to its provider too.</li>
  </ul>
</div>

<style>
  .perm {
    display: flex;
    gap: 12px;
    border-radius: 12px;
    border: 1px solid var(--color-ink-800);
    background: var(--color-ink-900);
    padding: 14px;
  }
  .perm-icon {
    display: grid;
    height: 32px;
    width: 32px;
    flex-shrink: 0;
    place-items: center;
    border-radius: 8px;
    background: var(--color-ink-800);
    color: var(--color-ink-200);
  }
  .on {
    border-radius: 12px;
    border: 1px solid color-mix(in oklab, var(--color-ember-500) 45%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 6%, var(--color-ink-900));
    padding: 16px;
  }
  .pulse {
    height: 10px;
    width: 10px;
    border-radius: 9999px;
    background: var(--color-ember-500);
    box-shadow: 0 0 0 0 color-mix(in oklab, var(--color-ember-500) 60%, transparent);
    animation: pulse 2s ease-out infinite;
  }
  @keyframes pulse {
    to { box-shadow: 0 0 0 8px transparent; }
  }
  .qr :global(svg) {
    display: block;
    width: 100%;
    height: auto;
  }
  .pair-code {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 1.375rem;
    font-weight: 700;
    letter-spacing: 0.3em;
    color: var(--color-ember-200);
    border: 1px solid color-mix(in oklab, var(--color-ember-500) 40%, transparent);
    border-radius: 0.5rem;
    padding: 0.5rem 0.875rem;
    background: color-mix(in oklab, var(--color-ember-500) 10%, transparent);
  }
  @media (prefers-reduced-motion: reduce) {
    .pulse {
      animation: none;
    }
  }
  :global([data-motion="off"]) .pulse {
    animation: none;
  }
  .link {
    display: flex;
    align-items: center;
    gap: 10px;
    border-radius: 10px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
    padding: 6px 6px 6px 12px;
  }
</style>
