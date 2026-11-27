<script lang="ts">
  // Buttons for a file a chat made: open it in its app, open it with
  // another app, or show it in Finder / File Explorer.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import { fileManagerName } from "../lib/platform";
  import { showToast } from "../lib/toast.svelte";
  import type { OpenWithApp } from "../lib/api/bindings";

  interface Props {
    /** The path as the model wrote it, inside the chat's folder. */
    path: string;
    /** The chat's project folder; null for the shared workspace. */
    folder?: string | null;
  }

  let { path, folder = null }: Props = $props();

  let menuOpen = $state(false);
  let apps = $state<OpenWithApp[] | null>(null);
  const name = $derived(path.split(/[\\/]/).pop() ?? path);

  const run = async (what: () => Promise<void>) => {
    try {
      await what();
    } catch (e: unknown) {
      showToast(errorMessage(e), "error");
    }
  };

  const openMenu = async () => {
    menuOpen = !menuOpen;
    if (menuOpen && apps === null) {
      try {
        apps = await api.openWithApps();
      } catch {
        apps = [];
      }
    }
  };
</script>

<div class="flex flex-wrap items-center gap-1.5">
  <span class="mr-1 flex min-w-0 items-center gap-1.5 text-[13px] text-ink-300" title={path}>
    <Icon name="file" class="h-3.5 w-3.5 shrink-0 text-ink-500" /><span class="truncate font-medium text-ink-200">{name}</span>
  </span>
  <button type="button" class="file-btn" onclick={() => void run(() => api.openFile(path, folder))}>Open</button>
  <div class="relative" use:popover={{ open: menuOpen, onClose: () => (menuOpen = false) }}>
    <button type="button" class="file-btn" aria-haspopup="menu" aria-expanded={menuOpen} onclick={() => void openMenu()}>Open with<Icon name="chevron-down" class="h-3 w-3" /></button>
    {#if menuOpen}
<div role="menu" aria-label={`Open ${name} with`} class="file-menu surface-glass" in:fly={{ y: 4, duration: dur(motion.fast), easing: motion.enter }}>
        {#if apps === null}
          <div class="px-3 py-2 text-sm text-ink-500">Looking for apps…</div>
        {:else if apps.length === 0}
          <div class="px-3 py-2 text-sm text-ink-500">No editors found. Use Open, or show the file in its folder.</div>
        {:else}
          {#each apps as app (app.id)}
            <button type="button" role="menuitem" class="file-menu-item" onclick={() => { menuOpen = false; void run(() => api.openFileWith(path, folder, app.id)); }}>{app.name}</button>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
  <button type="button" class="file-btn" onclick={() => void run(() => api.revealFile(path, folder))}><Icon name="folder-open" class="h-3.5 w-3.5" />Show in {fileManagerName()}</button>
</div>

<style>
  .file-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.75rem;
    border-radius: 0.45rem;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.6rem;
    font-size: 0.8125rem;
    color: var(--color-ink-200);
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
  }
  .file-btn:hover {
    border-color: var(--color-edge);
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .file-menu {
    position: absolute;
    left: 0;
    top: 2.1rem;
    z-index: 30;
    min-width: 12rem;
    border-radius: 0.6rem;
    border: 1px solid var(--color-ink-700);
    padding: 0.3rem;
    box-shadow: var(--shadow-elev);
    transform-origin: top left;
  }
  .file-menu-item {
    display: block;
    width: 100%;
    border-radius: 0.4rem;
    padding: 0.4rem 0.6rem;
    text-align: left;
    font-size: 0.875rem;
    color: var(--color-ink-200);
  }
  .file-menu-item:hover,
  .file-menu-item:focus {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    outline: none;
  }
</style>
