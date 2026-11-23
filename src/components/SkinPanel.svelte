<script lang="ts">
  // Shareable skins. There is no skin IPC in this build: skins are JSON files
  // of CSS custom-property overrides, kept in localStorage and imported or
  // exported as files. See docs/evidence/skins-and-plugins.md.
  import { onMount } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import {
    activeSkinOverrides, applySkin, cleanSkinOverrides, deleteSkin,
    listSkins, parseSkin, saveSkin, type Skin,
  } from "../lib/theme";

  // The tokens most people change, shown as a short form. Imported skins can
  // carry any token in SKIN_PROPERTIES; those are kept and applied unchanged.
  const EDIT_PROPERTIES = [
    "--color-ember-500",
    "--color-ink-50",
    "--color-ink-100",
    "--color-ink-900",
    "--color-ink-950",
    "--color-edge",
    "--radius-lg",
    "--font-sans",
  ] as const;

  let name = $state("");
  let overrides = $state<Record<string, string>>({});
  let skins = $state<Skin[]>([]);
  let notice = $state<{ ok: boolean; text: string } | null>(null);
  let fileInput = $state<HTMLInputElement>();

  onMount(() => {
    skins = listSkins();
    overrides = activeSkinOverrides();
  });

  const setProp = (prop: string, value: string) => {
    const next = { ...overrides };
    if (value.trim()) next[prop] = value;
    else delete next[prop];
    overrides = next;
  };

  const applyCurrent = () => {
    applySkin(overrides);
    notice = { ok: true, text: "Skin applied. It will be restored at the next launch." };
  };

  const saveCurrent = () => {
    if (!name.trim()) {
      notice = { ok: false, text: "Give the skin a name before saving." };
      return;
    }
    saveSkin({ name, overrides });
    skins = listSkins();
    applySkin(overrides);
    notice = { ok: true, text: `Skin “${name.trim()}” saved and applied.` };
  };

  const useSkin = (skin: Skin) => {
    name = skin.name;
    overrides = { ...skin.overrides };
    applySkin(overrides);
    notice = { ok: true, text: `Skin “${skin.name}” applied.` };
  };

  const removeSkin = (skin: Skin) => {
    deleteSkin(skin.name);
    skins = listSkins();
    notice = { ok: true, text: `Skin “${skin.name}” removed from this browser.` };
  };

  const exportSkin = () => {
    const json = JSON.stringify({ name: name.trim() || "My skin", overrides: cleanSkinOverrides(overrides) }, null, 2);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `${(name.trim() || "skin").replace(/[^a-z0-9_-]+/gi, "-").toLowerCase() || "skin"}.json`;
    link.click();
    URL.revokeObjectURL(url);
    notice = { ok: true, text: "Skin exported as a JSON file you can share." };
  };

  const importSkin = async (event: Event) => {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const skin = parseSkin(await file.text());
      if (!skin) {
        notice = { ok: false, text: "That file is not a LocalAide skin (a name and overrides are required)." };
      } else {
        saveSkin(skin);
        skins = listSkins();
        name = skin.name;
        overrides = skin.overrides;
        applySkin(overrides);
        notice = { ok: true, text: `Imported and applied “${skin.name}”.` };
      }
    } catch (e: unknown) {
      notice = { ok: false, text: `Could not read that file: ${String(e)}` };
    } finally {
      input.value = "";
    }
  };
</script>

<section class="mt-9" aria-labelledby="skin-panel-title">
  <h3 id="skin-panel-title" class="mb-2 flex items-center gap-2 text-[15px] font-semibold text-ink-50">
    <Icon name="sparkle" class="h-4 w-4 text-ember-300" />Skins
  </h3>
  <p class="mb-3 text-sm text-ink-400">
    A skin is a JSON file of design-token overrides. Change a value, save it here, then export the file to
    share it. Importing a file applies it straight away.
  </p>

  {#if notice}
    <p class={`mb-3 flex items-center gap-2 text-sm ${notice.ok ? "text-emerald-300" : "text-rose-300"}`} role="status">
      <Icon name={notice.ok ? "check" : "alert"} class="h-4 w-4 shrink-0" />{notice.text}
    </p>
  {/if}

  <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
    <label class="text-sm text-ink-400">Skin name
      <input bind:value={name} placeholder="Deep Ember, Ocean…" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
    </label>
    <div class="mt-3 grid gap-2 sm:grid-cols-2">
      {#each EDIT_PROPERTIES as prop (prop)}
        <label class="text-xs text-ink-400">{prop}
          <input value={overrides[prop] ?? ""} placeholder="use theme default" oninput={(e) => setProp(prop, (e.currentTarget as HTMLInputElement).value)} class="mt-1 h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-xs text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
        </label>
      {/each}
    </div>
    <div class="mt-4 flex flex-wrap gap-2">
      <Button variant="primary" onclick={saveCurrent}>Save skin</Button>
      <Button variant="secondary" onclick={applyCurrent}>Apply now</Button>
      <Button variant="secondary" onclick={exportSkin}><Icon name="download" class="h-4 w-4" />Export JSON</Button>
      <label class="inline-flex h-9 cursor-pointer items-center gap-2 rounded-md border border-ink-700 px-3 text-sm text-ink-200 transition-colors hover:bg-veil/5">
        <Icon name="plus" class="h-4 w-4" />Import JSON
        <input bind:this={fileInput} type="file" accept=".json,application/json" class="sr-only" onchange={(e) => void importSkin(e)} />
      </label>
    </div>
  </div>

  <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-4">
    <h4 class="text-sm font-semibold text-ink-100">Saved on this computer</h4>
    {#if skins.length === 0}
      <p class="mt-2 text-sm text-ink-500">No skins saved yet. Edit a token above and save, or import a JSON file.</p>
    {:else}
      <div class="mt-2 flex flex-wrap gap-2">
        {#each skins as skin (skin.name)}
          <div class="flex items-center gap-1 rounded-md border border-ink-700 px-2 py-1 text-sm text-ink-200">
            <button type="button" class="hover:text-ink-50" onclick={() => useSkin(skin)}>{skin.name}</button>
            <button type="button" class="text-ink-500 hover:text-rose-300" aria-label={`Remove ${skin.name}`} onclick={() => removeSkin(skin)}><Icon name="close" class="h-3.5 w-3.5" /></button>
          </div>
        {/each}
      </div>
    {/if}
    <p class="mt-3 text-xs leading-5 text-ink-500">
      Skins live in this browser's localStorage, not in ~/.aiassistant/skins: this build has no skin IPC.
      Clearing site data removes saved skins, and a skin applies on top of both light and dark themes.
      Export a JSON file to keep or share one.
    </p>
  </div>
</section>
