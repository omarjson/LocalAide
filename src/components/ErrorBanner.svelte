<script lang="ts">
  import Icon from "./Icon.svelte";

  let { message, onDismiss }: { message: string; onDismiss: () => void } = $props();
</script>

<div
  role="alert"
  class="p-3 bg-rose-500/10 border border-rose-500/20 rounded-xl text-xs text-rose-300 flex justify-between items-center"
>
  <span>{message}</span>
  <button onclick={onDismiss} aria-label="Dismiss error" class="text-rose-400 hover:text-rose-200 transition-colors">
    <Icon name="close" class="w-3.5 h-3.5" />
  </button>
</div>
