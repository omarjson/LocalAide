<script lang="ts">
  import Button from "./Button.svelte";
  import FocusTrap from "./FocusTrap.svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    message,
    confirmLabel = "Delete",
    onConfirm,
    onCancel,
  }: {
    title: string;
    message: string;
    confirmLabel?: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
</script>

<div
  class="fixed inset-0 z-[70] grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
  role="presentation"
  onclick={onCancel}
  onkeydown={(event) => { if (event.key === "Escape") onCancel(); }}
>
  <div
    class="w-full max-w-md rounded-lg border border-ink-700 bg-ink-900 p-5 shadow-2xl"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-labelledby="confirm-title"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => { if (event.key === "Escape") onCancel(); event.stopPropagation(); }}
  >
    <FocusTrap>
      <div class="flex items-start gap-3">
        <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-rose-500/10 text-rose-400">
          <Icon name="alert" class="h-[18px] w-[18px]" />
        </div>
        <div class="min-w-0">
          <h2 id="confirm-title" class="font-semibold text-ink-50">{title}</h2>
          <p class="mt-1 text-sm leading-6 text-ink-400">{message}</p>
        </div>
      </div>
      <div class="mt-5 flex justify-end gap-2">
        <Button variant="subtle" onclick={onCancel}>Cancel</Button>
        <Button variant="danger" onclick={onConfirm}>{confirmLabel}</Button>
      </div>
    </FocusTrap>
  </div>
</div>
