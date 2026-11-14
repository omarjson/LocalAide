<script lang="ts">
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    message,
    onDismiss,
    onRetry,
    retryLabel = "Retry",
  }: {
    message: string;
    onDismiss: () => void;
    onRetry?: () => void;
    retryLabel?: string;
  } = $props();
</script>

<div
  role="alert"
  class="flex items-start justify-between gap-3 rounded-lg border border-rose-500/20 bg-rose-500/10 p-3 text-sm text-rose-300"
>
  <div class="flex min-w-0 items-start gap-2">
    <Icon name="alert" class="mt-0.5 h-[18px] w-[18px] shrink-0" />
    <span class="break-words">{message}</span>
  </div>
  <div class="flex shrink-0 items-center gap-1">
    {#if onRetry}
      <Button variant="subtle" class="h-7 px-2 text-rose-400 hover:bg-rose-500/10 hover:text-rose-300" onclick={onRetry}>{retryLabel}</Button>
    {/if}
    <button type="button" onclick={onDismiss} aria-label="Dismiss error" class="grid h-7 w-7 place-items-center rounded-md text-rose-400 hover:bg-rose-500/10 hover:text-rose-200">
      <Icon name="close" class="h-4 w-4" />
    </button>
  </div>
</div>
