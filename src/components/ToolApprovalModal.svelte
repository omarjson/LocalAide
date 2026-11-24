<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { dur } from "../lib/motion";
  import FocusTrap from "./FocusTrap.svelte";
  import Icon from "./Icon.svelte";
  import ModeChip from "./ModeChip.svelte";
  import type { ApprovalMode, ToolAction } from "../lib/api/bindings";

  interface Props {
    show: boolean;
    toolName: string;
    action: ToolAction | null;
    /** The chat's permission mode, shown so the user knows how the agent runs. */
    mode?: ApprovalMode | null;
    onApproveOnce: () => void;
    onApproveSession: () => void;
    onDeny: () => void;
  }

  let {
    show = false,
    toolName = "",
    action = null,
    mode = null,
    onApproveOnce,
    onApproveSession,
    onDeny,
  }: Props = $props();
</script>

{#if show && action}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={onDeny}
    onkeydown={(e) => { if (e.key === 'Escape') onDeny(); }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="approval-title"
      tabindex="-1"
      in:scale={{ start: 0.97, duration: dur(130) }}
      out:scale={{ start: 0.97, duration: dur(80) }}
      class="bg-ink-900 border border-amber-500/40 rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-4"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => { if (e.key === 'Escape') onDeny(); e.stopPropagation(); }}
    >
      <FocusTrap>
      <div class="flex items-start gap-3">
        <div class="p-2.5 rounded-xl bg-amber-500/10 text-amber-400 border border-amber-500/20">
          <Icon name="shield" class="w-5 h-5" />
        </div>
        <div>
          <h3 id="approval-title" class="text-sm font-semibold text-ink-100">
            Allow this file change?
          </h3>
          <p class="text-sm text-ink-400 mt-1">
            LocalAide wants to change a file inside its safe workspace.
          </p>
          {#if mode}<div class="mt-2"><ModeChip {mode} /></div>{/if}
        </div>
      </div>

      <div class="bg-ink-950 border border-edge rounded-xl p-3 space-y-2 text-sm">
        <div class="flex justify-between text-ink-400">
          <span>Action:</span>
          <span class="text-ink-300 font-semibold font-mono">{toolName}</span>
        </div>
        <div class="flex justify-between text-ink-400">
          <span>Change:</span>
          <span class="text-amber-400 uppercase font-semibold font-mono">{action.action_type}</span>
        </div>
        <div class="flex justify-between text-ink-400">
          <span>File:</span>
          <span class="text-cyan-300 truncate max-w-xs font-mono">{action.path}</span>
        </div>
      </div>

      {#if action.content}
        <div>
          <div class="text-sm font-medium text-ink-400 mb-1">File content preview:</div>
          <pre class="bg-ink-950 border border-edge rounded-xl p-3 text-xs font-mono text-ink-300 max-h-40 overflow-y-auto whitespace-pre-wrap">{action.content}</pre>
        </div>
      {/if}

      <div class="text-sm text-ink-400">
        LocalAide can only change files inside the safe workspace.
      </div>

      <div class="flex items-center justify-end gap-2 pt-2 border-t border-edge">
        <button
          onclick={onDeny}
          class="px-3 py-1.5 text-xs rounded-xl bg-ink-800 hover:bg-ink-700 text-ink-300 transition-colors"
        >
          Deny
        </button>
        <button
          onclick={onApproveOnce}
          class="px-3 py-1.5 text-xs rounded-xl bg-amber-600 hover:bg-amber-500 text-on-ember font-medium transition-colors"
        >
          Allow Once
        </button>
        <button
          onclick={onApproveSession}
          title="Allows changes to files in this folder for the next 30 minutes"
          class="px-3 py-1.5 text-xs rounded-xl bg-ember-600 hover:bg-ember-500 text-on-ember font-medium transition-colors"
        >
          Allow in this folder for 30 min
        </button>
      </div>
      </FocusTrap>
    </div>
  </div>
{/if}
