<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TextField from "../components/TextField.svelte";
  import { api, errorMessage } from "../lib/api/client";

  type DevModel = { id: string; name: string; ram: number; size: string };
  type DevPreset = { name: string; data: { ramTotal: number; selected: string[] } };

  let ramTotal = $state(16);
  let selected = $state<Set<string>>(new Set(["qwen3-embedding-0.6b-q8"]));
  let models = $state<DevModel[]>([]);
  let totalRam = $state(0);
  let overBudget = $state(false);
  let loading = $state(true);
  let presetName = $state("");
  let presets = $state<DevPreset[]>([]);
  let actionError = $state<string | null>(null);

  const recalc = async () => {
    if (models.length === 0) return;
    const ids = Array.from(selected);
    try {
      const result = await api.computeRam(ids);
      totalRam = result.total_ram_gb;
    } catch (e: unknown) {
      actionError = `The memory estimate could not update: ${errorMessage(e)}`;
      totalRam = ids.reduce((sum, id) => sum + (models.find((model) => model.id === id)?.ram || 0), 0);
    }
    overBudget = totalRam > ramTotal * 0.7;
  };

  const load = async () => {
    loading = true;
    actionError = null;
    try {
      const list = await api.listModels();
      models = list.map((model) => ({
        id: model.id,
        name: model.name,
        ram: Number(model.ram_estimate_gb),
        size: model.size_bytes ? `${(model.size_bytes / 1024 / 1024).toFixed(0)} MB` : "Unknown",
      }));
      const config = await api.loadConfig();
      ramTotal = config.ram_budget_mb ? Math.round(config.ram_budget_mb / 1024) : ramTotal;
      if (config.selected_models?.length) selected = new Set(config.selected_models);
      await recalc();
    } catch (e: unknown) {
      actionError = errorMessage(e);
    } finally {
      loading = false;
    }
    try {
      const stored = localStorage.getItem("dev-presets");
      presets = stored ? JSON.parse(stored) as DevPreset[] : [];
    } catch (e: unknown) {
      actionError = `Saved presets could not load: ${errorMessage(e)}`;
    }
  };

  onMount(load);
  $effect(() => { void recalc(); });

  const savePreset = () => {
    if (!presetName.trim()) return;
    try {
      const data = { ramTotal, selected: Array.from(selected) };
      const list = [...presets, { name: presetName.trim(), data }];
      localStorage.setItem("dev-presets", JSON.stringify(list));
      presets = list;
      presetName = "";
    } catch (e: unknown) {
      actionError = `The preset could not be saved: ${errorMessage(e)}`;
    }
  };

  const loadPreset = (preset: DevPreset) => {
    ramTotal = preset.data.ramTotal;
    selected = new Set(preset.data.selected);
  };
</script>

<div class="mx-auto max-w-6xl pb-10">
  <PageHeader title="Developer config" description="Choose local models and check their memory use.">
    {#snippet actions()}
      <Button variant="secondary" onclick={() => void load()}><Icon name="refresh" class="h-[18px] w-[18px]" />Refresh</Button>
    {/snippet}
  </PageHeader>

  {#if actionError}<div class="mb-4"><ErrorBanner message={actionError} onRetry={() => void load()} onDismiss={() => actionError = null} /></div>{/if}

  <div class="grid gap-6 lg:grid-cols-2">
    <div class="space-y-4">
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <div class="flex items-center justify-between gap-4"><h2 class="font-semibold text-ink-50">Available memory</h2><label class="flex items-center gap-2 text-sm text-ink-400"><input bind:value={ramTotal} type="number" class="h-9 w-24 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 outline-none focus:border-edge" />GB</label></div>
        <p class="mt-1 text-sm text-ink-400">Local models may use up to 70% of this amount.</p>
        <div class="my-4 h-2 w-full overflow-hidden rounded-full bg-ink-800"><div class={`h-full transition-all duration-300 ${overBudget ? "bg-rose-500" : "bg-emerald-500"}`} style={`width:${Math.min(100, (totalRam / (ramTotal * 0.7)) * 100)}%`}></div></div>
        <div class="text-sm text-ink-300">Selected: {totalRam.toFixed(2)} GB · limit: {(ramTotal * 0.7).toFixed(2)} GB <span class={overBudget ? "text-rose-400" : "text-emerald-400"}>· {overBudget ? "over limit" : "fits"}</span></div>
      </div>

      <div class="overflow-hidden rounded-lg border border-ink-700 bg-ink-900">
        {#if loading}<div class="px-4 py-10 text-center text-sm text-ink-400">Loading models…</div>{:else if models.length === 0}<EmptyState icon="models" title="No models available" description="Download a model before using this screen." />{:else}{#each models as model (model.id)}<label class="flex cursor-pointer items-center gap-3 border-b border-ink-800 p-4 last:border-0 hover:bg-veil/5"><input type="checkbox" bind:group={selected} value={model.id} class="accent-ember-600" /><div class="flex-1"><div class="font-semibold text-ink-50">{model.name}</div><div class="text-sm text-ink-400">{model.size} · about {model.ram.toFixed(2)} GB memory</div></div></label>{/each}{/if}
      </div>
    </div>

    <div>
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <h2 class="font-semibold text-ink-50">Saved choices</h2>
        <p class="mt-1 text-sm text-ink-400">Save a group of models for later.</p>
        <div class="mt-4 flex gap-2"><TextField bind:value={presetName} ariaLabel="Saved choice name" placeholder="Name this choice" class="flex-1" /><Button variant="secondary" disabled={!presetName.trim()} onclick={savePreset}>Save</Button></div>
        <div class="mt-3 space-y-2">
          {#each presets as preset (preset.name)}<button type="button" onclick={() => loadPreset(preset)} class="w-full rounded-md border border-ink-700 bg-ink-950 px-3 py-2 text-left text-sm text-ink-300 hover:bg-veil/5">{preset.name}</button>{:else}<p class="text-sm text-ink-400">No saved choices yet.</p>{/each}
        </div>
      </div>
    </div>
  </div>
</div>
