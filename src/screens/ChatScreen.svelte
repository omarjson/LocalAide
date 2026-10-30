<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  type Msg = { role: string; content: string; ts?: number };
  let messages = $state<Msg[]>([]);
  let input = $state("");
  let streaming = $state(false);
  let model = $state("Qwen3-0.6B Q4 (local)");
  let streamText = $state("");
  let systemPrompt = $state("You are a helpful local AI assistant.");
  let showSystem = $state(false);
  let showTools = $state(true);
  let scrollEl = $state<HTMLDivElement>();
  let toolPath = $state("");
  let imagePath = $state("");

  const scrollToBottom = async () => {
    await tick();
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  };

  $effect(() => { scrollToBottom(); });

  const send = async () => {
    if (!input.trim() || streaming) return;
    const userMsg = input;
    messages.push({ role: "user", content: userMsg, ts: Date.now() });
    input = "";
    streaming = true;
    const assistantIdx = messages.length;
    messages.push({ role: "assistant", content: "", ts: Date.now() });
    let acc = "";
    try {
      const streamId: string = await invoke("chat_stream", {
        message: userMsg,
        model,
        system_prompt: systemPrompt,
        temperature: 0.7,
        max_tokens: 512,
      });
      const streamUnsub = await listen("llm_stream", (event: any) => {
        if (event.payload?.stream_id !== streamId) return;
        acc += event.payload?.token ?? "";
        messages[assistantIdx].content = acc;
      });
      const doneUnsub = await listen("llm_done", async (event: any) => {
        if (event.payload?.stream_id !== streamId) return;
        streaming = false;
        await streamUnsub();
        await doneUnsub();
        await errUnsub();
      });
      const errUnsub = await listen("llm_error", async (event: any) => {
        if (event.payload?.stream_id !== streamId) return;
        messages[assistantIdx].content = "[chat error: " + (event.payload?.error ?? "unknown") + "]";
        streaming = false;
        await streamUnsub();
        await doneUnsub();
        await errUnsub();
      });
    } catch (e) {
      messages[assistantIdx].content = "[chat error: " + String(e) + "]";
      streaming = false;
    }
  };

  const summarizeLast = async () => {
    const last = messages.filter(m => m.role === "assistant").at(-1);
    if (!last || streaming) return;
    streaming = true;
    streamText = "";
    messages.push({ role: "assistant", content: "Summarizing…", ts: Date.now() });
    try {
      const streamId: string = await invoke("summarize", { text: last.content, model });
      const streamUnsub = await listen("summarize_stream", (event: any) => {
        if (event.payload?.stream_id !== streamId) return;
        streamText += event.payload?.token ?? "";
      });
      const doneUnsub = await listen("summarize_done", async (event: any) => {
        if (event.payload?.stream_id !== streamId) return;
        messages[messages.length - 1].content = streamText;
        streaming = false;
        await streamUnsub();
        await doneUnsub();
        await errUnsub();
      });
      const errUnsub = await listen("summarize_error", async (event: any) => {
        if (event.payload?.stream_id !== streamId) return;
        messages[messages.length - 1].content = "[summarize error: " + (event.payload?.error ?? "unknown") + "]";
        streaming = false;
        await streamUnsub();
        await doneUnsub();
        await errUnsub();
      });
    } catch (e) {
      messages[messages.length - 1].content = "[summarize error: " + String(e) + "]";
      streaming = false;
    }
  };

  const readFile = async () => {
    if (!toolPath.trim()) return;
    messages.push({ role: "user", content: `Read file: ${toolPath}`, ts: Date.now() });
    try {
      const res = await invoke<any>("tool_execute", { call: { name: "read_file", args: { path: toolPath } } });
      const out = res.ok ? res.output.slice(0, 4000) : `Error: ${res.error}`;
      messages.push({ role: "assistant", content: `File contents:\n\`\`\`\n${out}\n\`\`\``, ts: Date.now() });
    } catch (e) {
      messages.push({ role: "assistant", content: "[tool error: " + String(e) + "]", ts: Date.now() });
    }
    toolPath = "";
  };

  const identifyImage = async () => {
    if (!imagePath.trim()) return;
    messages.push({ role: "user", content: `Identify image: ${imagePath}`, ts: Date.now() });
    try {
      const res = await invoke<any>("vision_identify", { image_path: imagePath });
      const desc = res.description ?? "No description";
      const tags = (res.tags ?? []).join(", ");
      messages.push({ role: "assistant", content: `Vision result:\nDescription: ${desc}\nTags: ${tags}`, ts: Date.now() });
    } catch (e) {
      messages.push({ role: "assistant", content: "[vision error: " + String(e) + "]", ts: Date.now() });
    }
    imagePath = "";
  };

  onMount(() => {
    messages.push({ role: "assistant", content: "Hi! I'm your local AI assistant. Pick a model and start chatting.", ts: Date.now() });
  });
</script>

<div class="max-w-4xl mx-auto h-full flex flex-col">
  <div class="flex items-center justify-between px-2 py-3 border-b border-zinc-800">
    <div class="flex items-center gap-2">
      <span class="size-2 rounded-full bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.8)]"></span>
      <span class="text-xs uppercase tracking-widest text-zinc-400">Local model</span>
      <button onclick={() => showSystem = !showSystem} class="ml-3 text-[11px] px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400 hover:text-zinc-200">System</button>
      <button onclick={summarizeLast} disabled={streaming} class="ml-1 text-[11px] px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400 hover:text-zinc-200 disabled:opacity-50">Summarize</button>
    </div>
    <select bind:value={model} class="text-xs bg-zinc-900 border border-zinc-800 rounded-lg px-2 py-1 text-zinc-200 focus:outline-none focus:border-fuchsia-500">
      <option>Qwen3-0.6B Q4 (local)</option>
      <option>Qwen3-0.8B Q4 (local)</option>
      <option>Qwen3-4B Q4 (local)</option>
      <option>Cloud LLM (OpenRouter)</option>
    </select>
  </div>

  {#if showSystem}
    <div class="px-6 py-3 border-b border-zinc-800 bg-zinc-950/50">
      <textarea bind:value={systemPrompt} rows="2" placeholder="System prompt" class="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 focus:outline-none focus:border-fuchsia-500"/>
    </div>
  {/if}

  <div bind:this={scrollEl} class="flex-1 overflow-y-auto p-6 space-y-5">
    {#each messages as m}
      <div class={m.role === "user" ? "flex justify-end" : "flex justify-start"}>
        <div class={`max-w-[78%] px-4 py-3 rounded-[18px] text-sm leading-relaxed whitespace-pre-wrap relative ${m.role === "user" ? "bg-gradient-to-br from-fuchsia-600 to-cyan-500 text-white shadow-[0_0_30px_rgba(217,70,239,0.25)]" : "bg-zinc-900 border border-zinc-800 text-zinc-100"}`}>
          <div class="pr-6">{m.content}</div>
          <div class="mt-1 text-[10px] opacity-60 absolute bottom-1 right-2">{new Date(m.ts ?? 0).toLocaleTimeString()}</div>
          {#if streaming && m.role === "assistant" && m.content.length > 0}
            <span class="font-mono animate-pulse ml-1">▋</span>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <div class="border-t border-zinc-800 p-4">
    <div class="flex items-center justify-between mb-2">
      <button type="button" onclick={() => showTools = !showTools} class="text-[11px] px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400 hover:text-zinc-200">{showTools ? 'Hide tools' : 'Show tools'}</button>
      <div class="text-[10px] text-zinc-600">Enter to send • Shift+Enter for new line</div>
    </div>
    {#if showTools}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-2 mb-3">
        <div class="flex gap-2">
          <input bind:value={toolPath} placeholder="File path to read…" class="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-fuchsia-500"/>
          <button type="button" onclick={readFile} class="px-3 rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-xs text-zinc-300">Read</button>
        </div>
        <div class="flex gap-2">
          <input bind:value={imagePath} placeholder="Image path to identify…" class="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-fuchsia-500"/>
          <button type="button" onclick={identifyImage} class="px-3 rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-xs text-zinc-300">Identify</button>
        </div>
      </div>
    {/if}
    <form onsubmit={(e) => { e.preventDefault(); send(); }} class="flex gap-2">
      <textarea bind:value={input} placeholder="Message…" disabled={streaming} rows="1" onkeydown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); } }} class="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-fuchsia-500 resize-none"/>
      <button type="button" disabled={streaming} onclick={() => { input += " [voice input stub]"; }} class="size-11 grid place-items-center rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-zinc-300" title="Push-to-talk">🎙️</button>
      <button type="submit" disabled={streaming || !input.trim()} class="px-5 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm font-medium disabled:opacity-40">Send</button>
    </form>
  </div>
</div>
