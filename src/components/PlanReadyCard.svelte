<script lang="ts">
  // The card under a finished Plan-mode answer: Keep planning, or Build.
  // With Settings → Tools → Plan auto-accept on, a countdown builds the
  // plan when it ends. It holds while the user types (they may be
  // correcting the plan) and while the chat is off screen.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import ModeChip from "./ModeChip.svelte";
  import ApprovalCountdown from "./ApprovalCountdown.svelte";
  import { dur, motion } from "../lib/motion";
  import { permissionLabel } from "../lib/chatModes";
  import type { ApprovalMode } from "../lib/api/bindings";

  interface Props {
    buildMode: ApprovalMode;
    /** 0 waits for a choice (the default). */
    autoAcceptSecs?: number;
    typing?: boolean;
    active?: boolean;
    onKeepPlanning: () => void;
    onBuild: () => void;
  }

  let { buildMode, autoAcceptSecs = 0, typing = false, active = true, onKeepPlanning, onBuild }: Props = $props();
</script>

<div class="mb-2 rounded-xl border border-sky-400/30 bg-sky-400/10 px-3 py-2 text-sm" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
  <div class="flex flex-wrap items-center gap-3">
    <Icon name="lightbulb" class="h-4 w-4 shrink-0 text-sky-300" />
    <span class="min-w-0 flex-1 text-ink-200">The plan is ready. Build carries it out with the {permissionLabel(buildMode === "plan" ? "ask" : buildMode).replace(/ mode$/, "")} permission.</span>
    <ModeChip mode="plan" />
    <button type="button" onclick={onKeepPlanning} class="rounded-lg px-2.5 py-1 text-ink-300 hover:bg-veil/5">Keep planning</button>
    <button type="button" onclick={onBuild} class="rounded-lg bg-ember-500 px-3 py-1 font-semibold text-on-ember hover:bg-ember-400">Build</button>
  </div>
  {#if autoAcceptSecs > 0}
    <div class="mt-2 pl-7">
      <ApprovalCountdown
        seconds={autoAcceptSecs}
        label="Builds in"
        suffix="unless you choose Keep planning."
        paused={typing || !active}
        pausedLabel={typing ? "Auto-accept paused while you type." : "Auto-accept paused while this chat is not on screen."}
        onExpire={onBuild}
      />
    </div>
  {/if}
</div>
