<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { dur } from "../lib/motion";
  import Icon from "./Icon.svelte";
  import type { ToolAction } from "../lib/api/bindings";

  interface Props {
    show: boolean;
    toolName: string;
    action: ToolAction | null;
    onApproveOnce: () => void;
    onApproveSession: () => void;
    onDeny: () => void;
  }

  let {
    show = false,
    toolName = "",
    action = null,
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
      in:scale={{ start: 0.97, duration: dur(130) }}
      out:scale={{ start: 0.97, duration: dur(80) }}
      class="bg-ink-900 border border-amber-500/40 rounded-2xl max-w-lg w-full p-6 shadow-2xl space-y-4"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <div class="flex items-start gap-3">
        <div class="p-2.5 rounded-xl bg-amber-500/10 text-amber-400 border border-amber-500/20">
          <Icon name="shield" class="w-5 h-5" />
        </div>
        <div>
          <h3 id="approval-title" class="text-sm font-semibold text-ink-100">
            Tool Execution Approval Required
          </h3>
          <p class="text-xs text-ink-400 mt-1">
            The assistant is requesting to perform a filesystem modification inside the workspace sandbox.
          </p>
        </div>
      </div>

      <div class="bg-ink-950 border border-ink-800 rounded-xl p-3 space-y-2 text-xs font-mono">
        <div class="flex justify-between text-ink-400">
          <span>Tool:</span>
          <span class="text-ink-300 font-semibold">{toolName}</span>
        </div>
        <div class="flex justify-between text-ink-400">
          <span>Operation:</span>
          <span class="text-amber-400 uppercase font-semibold">{action.action_type}</span>
        </div>
        <div class="flex justify-between text-ink-400">
          <span>Target Path:</span>
          <span class="text-cyan-300 truncate max-w-xs">{action.path}</span>
        </div>
      </div>

      {#if action.content}
        <div>
          <div class="text-[11px] font-medium text-ink-400 mb-1">Payload / Content Preview:</div>
          <pre class="bg-ink-950 border border-ink-800 rounded-xl p-3 text-xs font-mono text-ink-300 max-h-40 overflow-y-auto whitespace-pre-wrap">{action.content}</pre>
        </div>
      {/if}

      <div class="text-[11px] text-ink-400">
        Workspace sandbox enforces boundary limits. Files outside the workspace cannot be touched.
      </div>

      <div class="flex items-center justify-end gap-2 pt-2 border-t border-ink-800">
        <button
          onclick={onDeny}
          class="px-3 py-1.5 text-xs rounded-xl bg-ink-800 hover:bg-ink-700 text-ink-300 transition-colors"
        >
          Deny
        </button>
        <button
          onclick={onApproveOnce}
          class="px-3 py-1.5 text-xs rounded-xl bg-amber-600 hover:bg-amber-500 text-ink-950 font-medium transition-colors"
        >
          Allow Once
        </button>
        <button
          onclick={onApproveSession}
          class="px-3 py-1.5 text-xs rounded-xl bg-ember-600 hover:bg-ember-500 text-ink-950 font-medium transition-colors"
        >
          Allow for Session
        </button>
      </div>
    </div>
  </div>
{/if}
