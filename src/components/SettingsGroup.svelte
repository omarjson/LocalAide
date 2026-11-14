<script lang="ts">
  import type { Snippet } from "svelte";

  let { children }: { children?: Snippet } = $props();
</script>

<div class="overflow-hidden rounded-lg border border-ink-700 bg-ink-900">
  {@render children?.()}
</div>
