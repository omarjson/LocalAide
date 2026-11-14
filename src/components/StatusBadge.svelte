<script lang="ts">
  type Tone = "ok" | "warn" | "idle";

  let {
    label,
    tone = "idle",
    dot = false,
  }: {
    label: string;
    tone?: Tone;
    dot?: boolean;
  } = $props();
</script>

<span class={`inline-flex h-[22px] items-center gap-1.5 rounded-full px-2 text-xs font-semibold ${tone === "ok"
  ? "bg-emerald-500/10 text-emerald-400"
  : tone === "warn"
    ? "bg-amber-500/10 text-amber-300"
    : "bg-ink-800 text-ink-300"}`}>
  {#if dot}<span class="h-2 w-2 rounded-full bg-current" aria-hidden="true"></span>{/if}
  {label}
</span>
