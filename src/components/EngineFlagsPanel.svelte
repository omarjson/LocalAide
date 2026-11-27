<script lang="ts">
  // Settings → Performance → Engine flags. Every control is optional: a field
  // left alone sends nothing, so llama.cpp's own default stands. Values are
  // saved with one button rather than on every keystroke, because a flag
  // only takes effect at the next model load anyway.
  import type { EngineBuildInfo, EngineConfig } from "../lib/api/bindings";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import Switch from "./Switch.svelte";
  import { formatBytes } from "../lib/format";

  let {
    flags = $bindable(null),
    busy = false,
    error = null,
    builds = [],
    gpu = null,
    accelNote = null,
    fetching = false,
    onFetch,
    onSave,
  }: {
    flags: EngineConfig | null;
    busy?: boolean;
    error?: string | null;
    /** Which builds this computer can run, with their download sizes. */
    builds?: EngineBuildInfo[];
    /** The graphics processor, when the system can name it. */
    gpu?: string | null;
    /** One line about the chosen build's download, from the engine info. */
    accelNote?: string | null;
    /** A graphics build is downloading. */
    fetching?: boolean;
    /** Download the chosen build now, instead of at the next model load. */
    onFetch?: () => void;
    onSave: () => void;
  } = $props();

  const CACHE_TYPES = [
    { value: "", label: "Default" },
    { value: "f16", label: "f16 — exact" },
    { value: "bf16", label: "bf16" },
    { value: "q8_0", label: "q8_0 — half" },
    { value: "q4_0", label: "q4_0 — quarter" },
    { value: "q4_1", label: "q4_1" },
  ];

  const onOff = [
    { value: "", label: "Default" },
    { value: "on", label: "On" },
    { value: "off", label: "Off" },
  ];

  const tri = (v: boolean | null) => (v === null ? "" : v ? "on" : "off");
  const setTri = (v: string): boolean | null => (v === "" ? null : v === "on");

  const num = (v: number | null) => (v === null ? "" : String(v));
  const setNum = (v: string, min: number, max: number): number | null => {
    const n = Number(v);
    if (v.trim() === "" || !Number.isFinite(n)) return null;
    return Math.min(max, Math.max(min, Math.round(n)));
  };

  // Which llama.cpp build runs. A graphics build downloads once when picked
  // and stays for later loads.
  const options = [
    { value: "cpu", label: "CPU", note: "Works everywhere. Uses the fastest instructions this processor has." },
    { value: "vulkan", label: "Vulkan (GPU)", note: "Offloads layers to NVIDIA, AMD or Intel graphics. One download for any card." },
    { value: "rocm", label: "ROCm (AMD GPU)", note: "AMD's own compute stack. A larger download than Vulkan; needs a recent AMD driver." },
  ];
</script>

{#if flags}
  <div class="mt-3 rounded-xl border border-ink-700 bg-ink-900">
    <div class="flex flex-wrap items-center gap-2 border-b border-ink-800 px-4 py-3">
      <Icon name="sliders" class="h-4 w-4 text-ink-400" />
      <span class="text-sm font-semibold text-ink-50">Engine flags</span>
      <span class="text-xs text-ink-500">Applied the next time a model loads</span>
    </div>

    <div class="grid gap-4 p-4 sm:grid-cols-2">
      <fieldset class="sm:col-span-2">
        <legend class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Engine build</span>
          <span class="min-w-0 truncate text-xs text-ink-500">{gpu ?? "downloaded once"}</span>
        </legend>
        <div class="grid gap-1.5 sm:grid-cols-3">
          {#each options as b (b.value)}
            {@const info = builds.find((x) => x.id === b.value)}
            {@const blocked = info ? !info.available : false}
            <label
              class={`flex items-start gap-2 rounded-lg border px-3 py-2.5 transition-colors ${blocked ? "cursor-not-allowed border-ink-800 opacity-70" : "cursor-pointer"} ${(flags.accel || "cpu") === b.value ? "border-edge bg-veil/5" : "border-ink-700 hover:bg-veil/5"}`}
            >
              <input
                type="radio"
                name="engine-build"
                value={b.value}
                checked={(flags.accel || "cpu") === b.value}
                disabled={blocked}
                onchange={() => (flags.accel = b.value)}
                class="mt-0.5 h-4 w-4 accent-[var(--color-ink-100)]"
              />
              <span class="min-w-0">
                <span class="block text-sm font-medium text-ink-100">{b.label}</span>
                <span class="block text-xs leading-5 text-ink-500">{b.note}</span>
                {#if info && info.bytes}
                  <span class="mt-1 block font-mono text-2xs text-ink-500">{formatBytes(info.bytes)}</span>
                {:else if blocked}
                  <span class="mt-1 block text-2xs text-ink-500">Not built for this computer</span>
                {/if}
              </span>
            </label>
          {/each}
        </div>
        {#if accelNote}<p class="mt-1.5 text-xs leading-5 text-ink-500">{accelNote}</p>{/if}
        {#if onFetch && (flags.accel || "cpu") !== "cpu"}
          <Button variant="secondary" class="mt-2" disabled={fetching} onclick={onFetch}>
            <Icon name="download" class={`h-4 w-4 ${fetching ? "animate-pulse" : ""}`} />{fetching ? "Downloading…" : "Download this build now"}
          </Button>
        {/if}
      </fieldset>
      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>GPU layers</span>
          <span class="text-xs text-ink-500">-ngl</span>
        </span>
        <input
          type="number"
          min="0"
          max="999"
          placeholder="llama.cpp decides"
          value={num(flags.gpu_layers)}
          oninput={(e) => (flags.gpu_layers = setNum(e.currentTarget.value, 0, 999))}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 text-sm text-ink-100 placeholder:text-ink-500"
        />
        <span class="mt-1 block text-xs text-ink-500">999 keeps every layer on the GPU. Leave empty for the default.</span>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Threads</span>
          <span class="text-xs text-ink-500">-t</span>
        </span>
        <input
          type="number"
          min="1"
          max="256"
          placeholder="physical cores"
          value={num(flags.threads)}
          oninput={(e) => (flags.threads = setNum(e.currentTarget.value, 1, 256))}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 text-sm text-ink-100 placeholder:text-ink-500"
        />
        <span class="mt-1 block text-xs text-ink-500">More than the physical core count usually slows a reply down.</span>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Flash attention</span>
          <span class="text-xs text-ink-500">-fa</span>
        </span>
        <select
          value={tri(flags.flash_attn)}
          onchange={(e) => (flags.flash_attn = setTri(e.currentTarget.value))}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100"
        >
          {#each onOff as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        </select>
        <span class="mt-1 block text-xs text-ink-500">Faster prompt processing and less memory on most GPUs.</span>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Load from disk</span>
          <span class="text-xs text-ink-500">--mmap</span>
        </span>
        <select
          value={tri(flags.mmap)}
          onchange={(e) => (flags.mmap = setTri(e.currentTarget.value))}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100"
        >
          {#each onOff as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        </select>
        <span class="mt-1 block text-xs text-ink-500">Off reads the whole file into memory at load. Slower to start, steadier while it answers.</span>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Key cache</span>
          <span class="text-xs text-ink-500">-ctk</span>
        </span>
        <select
          value={flags.cache_type_k ?? ""}
          onchange={(e) => (flags.cache_type_k = e.currentTarget.value || null)}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100"
        >
          {#each CACHE_TYPES as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
        </select>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Value cache</span>
          <span class="text-xs text-ink-500">-ctv</span>
        </span>
        <select
          value={flags.cache_type_v ?? ""}
          onchange={(e) => (flags.cache_type_v = e.currentTarget.value || null)}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100"
        >
          {#each CACHE_TYPES as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
        </select>
        <span class="mt-1 block text-xs text-ink-500">A smaller cache fits a longer conversation in the same memory.</span>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Parallel slots</span>
          <span class="text-xs text-ink-500">-np</span>
        </span>
        <input
          type="number"
          min="1"
          max="16"
          placeholder="1"
          value={num(flags.parallel)}
          oninput={(e) => (flags.parallel = setNum(e.currentTarget.value, 1, 16))}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 text-sm text-ink-100 placeholder:text-ink-500"
        />
        <span class="mt-1 block text-xs text-ink-500">Each slot gets its own share of the cache.</span>
      </label>

      <label class="block">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Prompt batch</span>
          <span class="text-xs text-ink-500">-b</span>
        </span>
        <input
          type="number"
          min="32"
          max="8192"
          step="32"
          placeholder="512"
          value={num(flags.batch_size)}
          oninput={(e) => (flags.batch_size = setNum(e.currentTarget.value, 32, 8192))}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 text-sm text-ink-100 placeholder:text-ink-500"
        />
        <span class="mt-1 block text-xs text-ink-500">Bigger batches read long prompts faster and use more memory.</span>
      </label>

      <label class="block sm:col-span-2">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Context scale</span>
          <span class="text-xs text-ink-500">--rope-freq-scale</span>
        </span>
        <input
          type="number"
          min="0.1"
          max="8"
          step="0.05"
          placeholder="1"
          value={num(flags.rope_freq_scale)}
          oninput={(e) => {
            const n = Number(e.currentTarget.value);
            flags.rope_freq_scale = e.currentTarget.value.trim() === "" || !Number.isFinite(n) ? null : n;
          }}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 text-sm text-ink-100 placeholder:text-ink-500"
        />
        <span class="mt-1 block text-xs text-ink-500">Below 1 stretches the context past what the model was trained on. Test the result: quality drops if it is too small.</span>
      </label>

      <label class="flex items-center justify-between gap-3 rounded-lg border border-ink-700 px-3 py-2.5 sm:col-span-2">
        <span class="min-w-0">
          <span class="block text-sm text-ink-100">Keep the model in memory</span>
          <span class="block text-xs text-ink-500">Stops the system from swapping it out mid-answer. Needs enough free memory.</span>
        </span>
        <Switch checked={flags.mlock} label="Keep the model in memory" onchange={(on) => (flags.mlock = on)} />
      </label>

      <label class="block sm:col-span-2">
        <span class="mb-1 flex items-baseline justify-between gap-2 text-sm text-ink-200">
          <span>Extra flags</span>
          <span class="text-xs text-ink-500">passed through as written</span>
        </span>
        <input
          type="text"
          placeholder="--no-warmup --cache-reuse 256"
          bind:value={flags.extra_args}
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 font-mono text-sm text-ink-100 placeholder:text-ink-500"
        />
        <span class="mt-1 block text-xs text-ink-500">Anything llama-server accepts. A typo makes the next load fail, and the chat says which flag.</span>
      </label>
    </div>

    {#if error}
      <p class="border-t border-ink-800 px-4 py-2.5 text-sm text-rose-300">{error}</p>
    {/if}

    <div class="flex items-center justify-end gap-2 border-t border-ink-800 px-4 py-3">
      <Button variant="primary" disabled={busy} onclick={onSave}>{busy ? "Saving…" : "Save flags"}</Button>
    </div>
  </div>
{/if}
