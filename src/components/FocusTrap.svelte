<script lang="ts">
  import { onDestroy, type Snippet } from "svelte";

  let { children }: { children?: Snippet } = $props();
  let container = $state<HTMLDivElement | null>(null);
  // Captured before the dialog pulls focus in, then restored when it closes:
  // the trigger button (or whatever had focus) gets focus back, not the body.
  let previous: HTMLElement | null = null;
  let captured = false;

  const focusable = () => Array.from(container?.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
  ) ?? []);

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Tab") return;
    const elements = focusable();
    if (elements.length === 0) {
      event.preventDefault();
      return;
    }
    const first = elements[0];
    const last = elements[elements.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  $effect(() => {
    if (!container || captured) return;
    captured = true;
    const active = document.activeElement;
    previous = active instanceof HTMLElement ? active : null;
    focusable()[0]?.focus();
  });

  onDestroy(() => {
    if (previous?.isConnected) previous.focus();
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div bind:this={container} class="contents" role="group" aria-label="Dialog controls" onkeydown={handleKeydown}>
  {@render children?.()}
</div>
