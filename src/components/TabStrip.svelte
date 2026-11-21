<script lang="ts">
  // The chat's tab strip: every open chat, left to right, each free to
  // stream on its own. A pulsing ember dot marks the tabs that are
  // answering; the plus opens another.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";
  import { dur, motion } from "../lib/motion";
  import type { ChatTab } from "../lib/chats.svelte";

  interface Props {
    tabs: ChatTab[];
    activeKey: string | null;
    visible: boolean;
    onFocus: (key: string) => void;
    onClose: (key: string) => void;
    onNew: () => void;
  }

  let { tabs, activeKey, visible, onFocus, onClose, onNew }: Props = $props();
</script>

{#if visible && tabs.length > 0}
  <div
    class="flex h-9 shrink-0 items-stretch gap-1 overflow-x-auto border-b border-ink-800 bg-ink-950 px-2 pt-1"
    role="tablist"
    aria-label="Open chats"
    in:fly={{ y: -6, duration: dur(motion.base), easing: motion.enter }}
  >
    {#each tabs as tab (tab.key)}
      <div
        role="tab"
        tabindex="0"
        aria-selected={tab.key === activeKey}
        aria-label={`Chat tab: ${tab.title}${tab.busy ? " (answering)" : ""}`}
        class={`group/tab flex max-w-52 min-w-0 shrink-0 cursor-pointer items-center gap-1.5 rounded-t-lg border-x border-t px-2.5 text-sm transition-colors ${
          tab.key === activeKey
            ? "border-ink-800 bg-ink-900 font-medium text-ink-50"
            : "border-transparent text-ink-400 hover:bg-veil/5 hover:text-ink-100"
        }`}
        onclick={() => onFocus(tab.key)}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onFocus(tab.key); } }}
      >
        {#if tab.busy}
          <span class="busy-dot" aria-hidden="true"></span>
        {:else}
          <LogoMark class="h-3.5 w-3.5 shrink-0 opacity-70" />
        {/if}
        <span class="min-w-0 flex-1 truncate">{tab.title}</span>
        <button
          type="button"
          class="grid h-5 w-5 shrink-0 place-items-center rounded-md text-ink-500 opacity-0 transition-[opacity,color] group-hover/tab:opacity-100 hover:bg-veil/10 hover:text-ink-50 focus-visible:opacity-100"
          aria-label={`Close the tab ${tab.title}`}
          onclick={(e) => { e.stopPropagation(); onClose(tab.key); }}
        >
          <Icon name="close" class="h-3 w-3" />
        </button>
      </div>
    {/each}
    <button
      type="button"
      class="mb-1 ml-0.5 flex shrink-0 items-center gap-1 self-center rounded-md px-2 py-1 text-sm text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-100"
      title="New chat tab (Ctrl+N)"
      aria-label="Open a new chat tab"
      onclick={onNew}
    >
      <Icon name="plus" class="h-3.5 w-3.5" />
    </button>
  </div>
{/if}

<style>
  .busy-dot {
    height: 0.55rem;
    width: 0.55rem;
    flex-shrink: 0;
    border-radius: 9999px;
    background: var(--color-ember-400);
    animation: tab-pulse 1.4s ease-in-out infinite;
  }
  @keyframes tab-pulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.45;
      transform: scale(0.8);
    }
  }
</style>
