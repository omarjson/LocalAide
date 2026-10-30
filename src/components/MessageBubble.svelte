<script lang="ts">
  let { role, content, ts, streaming = false } = $props<{
    role: string;
    content: string;
    ts?: number;
    streaming?: boolean;
  }>();
</script>

<div class={role === "user" ? "flex justify-end" : "flex justify-start"}>
  <div class={`max-w-[78%] px-4 py-3 rounded-[18px] text-sm leading-relaxed whitespace-pre-wrap relative ${role === "user" ? "bg-gradient-to-br from-fuchsia-600 to-cyan-500 text-white shadow-[0_0_30px_rgba(217,70,239,0.25)]" : "bg-zinc-900 border border-zinc-800 text-zinc-100"}`}>
    <div class="pr-6">{content}</div>
    <div class="mt-1 text-[10px] opacity-60 absolute bottom-1 right-2">{new Date(ts ?? 0).toLocaleTimeString()}</div>
    {#if streaming && role === "assistant" && content.length > 0}
      <span class="font-mono animate-pulse ml-1">▋</span>
    {/if}
  </div>
</div>
