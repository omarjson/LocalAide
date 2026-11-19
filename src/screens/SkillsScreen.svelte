<script lang="ts">
  import { onMount } from "svelte";
  import { flip } from "svelte/animate";
  import { fade, fly, slide } from "svelte/transition";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import MarkdownDoc from "../components/MarkdownDoc.svelte";
  import Modal from "../components/Modal.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Tabs from "../components/Tabs.svelte";
  import TextField from "../components/TextField.svelte";
  import { api, errorMessage as errMsg } from "../lib/api/client";
  import { openInBrowser } from "../lib/links";
  import { dur, enterScreen, flipOptions, motion, stagger } from "../lib/motion";
  import type { RemoteSkill, RemoteSkillDetail, Skill, SkillSource, SkillSuggestion } from "../lib/api/bindings";

  type View = "mine" | "discover";
  let view = $state<View>("mine");

  let skills = $state<Skill[]>([]);
  let suggestions = $state<SkillSuggestion[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let actionError = $state<string | null>(null);
  let savedNotice = $state<string | null>(null);
  let editing = $state<string | null>(null);
  let name = $state("");
  let description = $state("");
  let tags = $state("");
  let body = $state("");
  let showEditor = $state(false);
  let pendingDeletePath = $state<string | null>(null);
  let mineQuery = $state("");

  // Discover
  let sources = $state<SkillSource[]>([]);
  let activeSource = $state<string | null>(null);
  let customRepo = $state("");
  let activeRepo = $state<string | undefined>(undefined);
  let remote = $state<RemoteSkill[]>([]);
  let remoteBusy = $state(false);
  let remoteError = $state<string | null>(null);
  let remoteQuery = $state("");
  let remoteLimit = $state(60);
  let preview = $state<RemoteSkillDetail | null>(null);
  let previewBusy = $state<string | null>(null);
  let importing = $state<string | null>(null);

  const flash = (message: string) => {
    savedNotice = message;
    setTimeout(() => savedNotice = null, 2200);
  };

  const refresh = async () => {
    loading = true;
    try {
      skills = await api.skillsList();
      suggestions = await api.skillsSuggestions();
      actionError = null;
    } catch (e: unknown) {
      actionError = errMsg(e);
    } finally {
      loading = false;
    }
  };

  onMount(() => {
    void refresh();
    void api.skillsSources().then((list) => { sources = list; }).catch(() => { sources = []; });
  });

  const shownSkills = $derived(
    skills.filter((s) => {
      const q = mineQuery.trim().toLowerCase();
      return !q || s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q) || s.tags.some((t) => t.toLowerCase().includes(q));
    }),
  );

  const matchingRemote = $derived(
    remote.filter((r) => {
      const q = remoteQuery.trim().toLowerCase();
      return !q || r.name.toLowerCase().includes(q) || (r.description ?? r.preview ?? "").toLowerCase().includes(q);
    }),
  );

  const owned = $derived(new Set(skills.map((s) => s.name.toLowerCase())));

  const sourceLabel = $derived(
    activeSource === "github" ? activeRepo ?? "GitHub" : sources.find((s) => s.id === activeSource)?.label ?? "",
  );

  const openSource = async (id: string, repo?: string) => {
    activeSource = id;
    activeRepo = repo;
    remote = [];
    remoteError = null;
    remoteQuery = "";
    remoteLimit = 60;
    remoteBusy = true;
    try {
      remote = await api.skillsDiscover(id, repo);
    } catch (e: unknown) {
      remoteError = errMsg(e);
    } finally {
      remoteBusy = false;
    }
    void describeVisible();
  };

  $effect(() => {
    void remoteQuery;
    if (!activeSource || remote.length === 0) return;
    const timer = setTimeout(() => void describeVisible(), 400);
    return () => clearTimeout(timer);
  });

  // Skill folders list names only; read the descriptions of what is on
  // screen (one page at a time) so each card says what the skill does.
  let describing = $state(false);
  const describeVisible = async () => {
    const source = activeSource;
    if (!source || describing) return;
    const ids = matchingRemote
      .slice(0, remoteLimit)
      .filter((r) => !r.description && !r.preview)
      .map((r) => r.id)
      .slice(0, 60);
    if (ids.length === 0) return;
    describing = true;
    try {
      const found = await api.skillsRemoteDescribe(source, ids, activeRepo);
      if (source !== activeSource) return;
      const byId = new Map(found.map((d) => [d.id, d]));
      remote = remote.map((r) => {
        const d = byId.get(r.id);
        return d ? { ...r, name: d.name || r.name, description: d.description } : r;
      });
    } catch (e: unknown) {
      console.warn("Skill descriptions could not load", e);
    } finally {
      describing = false;
    }
  };

  const openPreview = async (item: RemoteSkill) => {
    if (!activeSource) return;
    previewBusy = item.id;
    remoteError = null;
    try {
      preview = await api.skillsRemoteDetail(activeSource, item.id, activeRepo);
    } catch (e: unknown) {
      remoteError = errMsg(e);
    } finally {
      previewBusy = null;
    }
  };

  const importSkill = async (id: string) => {
    if (!activeSource) return;
    importing = id;
    remoteError = null;
    try {
      const skill = await api.skillsImport(activeSource, id, activeRepo);
      flash(`Saved “${skill.name}”. Type / in chat to use it.`);
      preview = null;
      await refresh();
    } catch (e: unknown) {
      remoteError = errMsg(e);
    } finally {
      importing = null;
    }
  };

  const openCreate = (prefill?: string) => {
    view = "mine";
    editing = null;
    name = "";
    description = "";
    tags = "";
    body = prefill ?? "";
    showEditor = true;
    actionError = null;
  };

  const openEdit = async (skill: Skill) => {
    try {
      const full = await api.skillsRead(skill.path);
      editing = skill.path;
      name = full.name;
      description = full.description;
      tags = full.tags.join(", ");
      body = full.body;
      showEditor = true;
      actionError = null;
    } catch (e: unknown) {
      actionError = errMsg(e);
    }
  };

  const save = async () => {
    if (!name.trim() || !body.trim()) return;
    busy = true;
    actionError = null;
    try {
      const tagList = tags.split(",").map((tag) => tag.trim()).filter(Boolean);
      if (editing) {
        await api.skillsUpdate(editing, name, description, tagList, body);
        flash("Skill saved.");
      } else {
        await api.skillsCreate(name, description, tagList, body);
        flash("Skill created.");
      }
      showEditor = false;
      await refresh();
    } catch (e: unknown) {
      actionError = errMsg(e);
    } finally {
      busy = false;
    }
  };

  const remove = async (skill: Skill) => {
    busy = true;
    actionError = null;
    try {
      await api.skillsDelete(skill.path);
      flash("Skill deleted.");
      await refresh();
    } catch (e: unknown) {
      actionError = errMsg(e);
    } finally {
      busy = false;
    }
  };

  const confirmRemove = async () => {
    const path = pendingDeletePath;
    pendingDeletePath = null;
    const skill = skills.find((item) => item.path === path);
    if (skill) await remove(skill);
  };

  const copyBody = async (skill: Skill) => {
    try {
      await navigator.clipboard.writeText(skill.body);
      flash("Prompt copied.");
    } catch (e: unknown) {
      actionError = errMsg(e);
    }
  };

  const hostOf = (url: string): string => {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  };

  const sourceIcon = (id: string): "skills" | "sparkle" | "chat" => (id === "anthropic" ? "sparkle" : id === "copilot" ? "skills" : "chat");
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader title="Skills" description="Saved prompts you can pick from the chat box with /. Write your own, or find them in public collections.">
    {#snippet actions()}
      <Button variant="primary" onclick={() => openCreate()}><Icon name="plus" class="h-[18px] w-[18px]" />New skill</Button>
    {/snippet}
  </PageHeader>

  <div class="mb-5 flex flex-wrap items-center justify-between gap-3">
    <Tabs
      items={[{ value: "mine", label: `My skills${skills.length ? ` (${skills.length})` : ""}` }, { value: "discover", label: "Discover" }]}
      value={view}
      onChange={(v) => { view = v === "discover" ? "discover" : "mine"; }}
      ariaLabel="Skills view"
    />
    {#if view === "mine" && skills.length > 3}
      <TextField bind:value={mineQuery} type="search" icon="search" ariaLabel="Filter skills" placeholder="Filter skills" class="w-full sm:w-64" />
    {/if}
  </div>

  {#if savedNotice}
    <div class="mb-4 flex items-center gap-2 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300" in:fly={{ y: -4, duration: dur(motion.base) }} out:fade={{ duration: dur(motion.fast) }}>
      <Icon name="check" class="h-4 w-4" />{savedNotice}
    </div>
  {/if}
  {#if actionError}<div class="mb-4"><ErrorBanner message={actionError} onRetry={() => void refresh()} onDismiss={() => actionError = null} /></div>{/if}

  {#key view}
  <div in:fly={enterScreen()}>
  {#if view === "mine"}
    {#if suggestions.length > 0 && !showEditor}
      <div class="mb-5 rounded-lg border border-ember-500/30 bg-ember-500/5 p-4">
        <div class="flex items-center gap-2 font-semibold text-ember-200"><Icon name="sparkle" class="h-[18px] w-[18px]" />Suggested prompts</div>
        <p class="mt-1 text-sm text-ink-400">You used these prompts more than once. Save one for later?</p>
        <div class="mt-3 space-y-2">
          {#each suggestions.slice(0, 3) as suggestion (suggestion.prompt)}
            <div class="flex items-center justify-between gap-3 rounded-md border border-ink-700 bg-ink-950 p-3">
              <div class="min-w-0"><div class="truncate text-sm text-ink-300">{suggestion.prompt}</div><div class="text-sm text-ink-400">Used {suggestion.use_count} times</div></div>
              <Button variant="secondary" class="shrink-0" onclick={() => openCreate(suggestion.prompt)}>Save as skill</Button>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if showEditor}
      <div class="mb-5 rounded-lg border border-ink-700 bg-ink-900 p-4" transition:slide={{ duration: dur(motion.base) }}>
        <div class="flex items-center justify-between"><h2 class="font-semibold text-ink-50">{editing ? "Edit skill" : "New skill"}</h2><Button variant="subtle" ariaLabel="Close skill editor" onclick={() => showEditor = false}><Icon name="close" class="h-4 w-4" /></Button></div>
        <div class="mt-4 space-y-3">
          <div class="grid gap-3 sm:grid-cols-2"><TextField bind:value={name} label="Name" placeholder="Example: review a document" /><TextField bind:value={tags} label="Tags" placeholder="Work, writing" /></div>
          <TextField bind:value={description} label="Description" placeholder="What this prompt helps with" />
          <label class="block text-sm text-ink-400">Prompt<textarea bind:value={body} rows={8} placeholder="Write the prompt here…" class="mt-1 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-3 py-2 text-sm leading-6 text-ink-100 placeholder:text-ink-400"></textarea></label>
          <div class="flex justify-end gap-2"><Button variant="subtle" onclick={() => showEditor = false}>Cancel</Button><Button variant="primary" disabled={busy || !name.trim() || !body.trim()} onclick={() => void save()}>{busy ? "Saving…" : editing ? "Save changes" : "Create skill"}</Button></div>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="space-y-3" aria-label="Loading skills">
        {#each [0, 1, 2] as i (i)}<div class="skeleton h-24"></div>{/each}
      </div>
    {:else if skills.length === 0 && !showEditor}
      <EmptyState icon="skills" title="No skills yet" description="A skill is a prompt you save once and reuse. Write one, or pick from thousands in public collections.">
        {#snippet action()}
          <div class="flex flex-wrap justify-center gap-2">
            <Button variant="primary" onclick={() => { view = "discover"; }}><Icon name="globe" class="h-4 w-4" />Discover skills</Button>
            <Button variant="secondary" onclick={() => openCreate()}>Write one</Button>
          </div>
        {/snippet}
      </EmptyState>
    {:else}
      <div class="space-y-3">
        {#each shownSkills as skill, i (skill.path)}
          <div class="lift rounded-lg border border-ink-700 bg-ink-900 p-4 hover:border-edge" in:fly={stagger(i)} animate:flip={flipOptions()}>
            <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
              <div class="min-w-0">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="font-semibold text-ink-50">/{skill.name}</span>
                  {#each skill.tags as tag (tag)}<span class="rounded-full border border-ink-700 px-2 py-0.5 text-xs text-ink-400">{tag}</span>{/each}
                </div>
                {#if skill.description}<p class="mt-1 text-sm text-ink-400">{skill.description}</p>{/if}
                <p class="mt-2 line-clamp-2 text-sm leading-6 text-ink-300">{skill.body}</p>
                {#if skill.source}
                  <a href={skill.source} rel="noreferrer" onclick={(e) => openInBrowser(e, skill.source ?? "")} class="mt-2 inline-flex items-center gap-1 text-xs text-ink-500 hover:text-ember-300">
                    <Icon name="external" class="h-3.5 w-3.5" />From {hostOf(skill.source)}{skill.license ? ` · ${skill.license}` : ""}
                  </a>
                {/if}
              </div>
              <div class="flex shrink-0 items-center gap-2"><Button variant="secondary" onclick={() => void copyBody(skill)}>Copy</Button><Button variant="secondary" onclick={() => void openEdit(skill)}>Edit</Button><Button variant="danger" disabled={busy} onclick={() => pendingDeletePath = skill.path}>Delete</Button></div>
            </div>
          </div>
        {:else}
          <p class="py-8 text-center text-sm text-ink-400">No skills match “{mineQuery}”.</p>
        {/each}
      </div>
    {/if}
  {:else}
    <p class="mb-4 text-sm text-ink-400">Collections load from GitHub when you open them. Nothing is sent about you. Check each skill's license before you use it for work.</p>

    <div class="grid gap-3 sm:grid-cols-3">
      {#each sources as source, i (source.id)}
        <button
          type="button"
          aria-pressed={activeSource === source.id}
          onclick={() => void openSource(source.id)}
          in:fly={stagger(i)}
          class={`lift flex flex-col rounded-lg border p-4 text-left ${activeSource === source.id ? "border-ember-500/60 bg-ember-500/5" : "border-ink-700 bg-ink-900 hover:border-edge"}`}
        >
          <span class={`grid h-9 w-9 place-items-center rounded-md ${activeSource === source.id ? "bg-ember-500/15 text-ember-300" : "bg-ink-800 text-ink-200"}`}><Icon name={sourceIcon(source.id)} class="h-[18px] w-[18px]" /></span>
          <span class="mt-3 font-semibold text-ink-50">{source.label}</span>
          <span class="mt-1 text-sm leading-6 text-ink-400">{source.description}</span>
          <span class="mt-2 text-xs text-ink-500">{source.license_note}</span>
        </button>
      {:else}
        {#each [0, 1, 2] as i (i)}<div class="skeleton h-40"></div>{/each}
      {/each}
    </div>

    <div class="mt-3 flex flex-col gap-2 rounded-lg border border-ink-700 bg-ink-900 p-3 sm:flex-row sm:items-center">
      <span class="flex items-center gap-2 text-sm text-ink-300"><Icon name="globe" class="h-4 w-4" />Any public repo:</span>
      <TextField bind:value={customRepo} ariaLabel="GitHub repository" placeholder="owner/name, with a skills folder" class="min-w-0 flex-1" />
      <Button variant="secondary" disabled={remoteBusy || !customRepo.trim()} onclick={() => void openSource("github", customRepo.trim())}>Open</Button>
    </div>

    {#if remoteError}<div class="mt-4"><ErrorBanner message={remoteError} onRetry={activeSource ? () => void openSource(activeSource ?? "", activeRepo) : undefined} onDismiss={() => remoteError = null} /></div>{/if}

    {#if activeSource}
      <div class="mt-5" in:fade={{ duration: dur(motion.base) }}>
        <div class="mb-3 flex flex-wrap items-center justify-between gap-3">
          <h2 class="font-semibold text-ink-50">{sourceLabel}{#if remote.length}<span class="ml-2 text-sm font-normal text-ink-400">{remote.length.toLocaleString()} skills</span>{/if}</h2>
          {#if remote.length > 0}
            <TextField bind:value={remoteQuery} type="search" icon="search" ariaLabel="Search this collection" placeholder="Search this collection" class="w-full sm:w-72" />
          {/if}
        </div>
        {#if remoteBusy}
          <div class="grid gap-2 sm:grid-cols-2" aria-label="Loading">
            {#each [0, 1, 2, 3, 4, 5] as i (i)}<div class="skeleton h-[74px]"></div>{/each}
          </div>
        {:else if remote.length > 0}
          <div class="grid gap-2 sm:grid-cols-2">
            {#each matchingRemote.slice(0, remoteLimit) as item, i (item.id)}
              <div class="lift flex items-start gap-3 rounded-md border border-ink-700 bg-ink-900 p-3 hover:border-edge" in:fly={stagger(i % 12)}>
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span class="truncate font-medium text-ink-50">{item.name}</span>
                    {#if owned.has(item.name.toLowerCase())}<span class="shrink-0 rounded bg-emerald-500/15 px-1.5 text-xs font-semibold text-emerald-300">Saved</span>{/if}
                  </div>
                  {#if item.description || item.preview}
                    <p class="mt-1 line-clamp-2 text-sm text-ink-400" in:fade={{ duration: dur(motion.base) }}>{item.description ?? item.preview}</p>
                  {:else if describing}
                    <div class="mt-2 space-y-1.5" aria-hidden="true"><div class="skeleton h-3 w-11/12"></div><div class="skeleton h-3 w-2/3"></div></div>
                  {/if}
                </div>
                <div class="flex shrink-0 gap-1">
                  <Button variant="subtle" disabled={previewBusy !== null} onclick={() => void openPreview(item)}>{previewBusy === item.id ? "Opening…" : "Preview"}</Button>
                  <Button variant="secondary" disabled={importing !== null || owned.has(item.name.toLowerCase())} onclick={() => void importSkill(item.id)}>{importing === item.id ? "Saving…" : "Add"}</Button>
                </div>
              </div>
            {:else}
              <p class="py-6 text-sm text-ink-400 sm:col-span-2">Nothing in this collection matches “{remoteQuery}”.</p>
            {/each}
          </div>
          {#if matchingRemote.length > remoteLimit}
            <div class="mt-4 flex justify-center"><Button variant="secondary" onclick={() => { remoteLimit += 60; void describeVisible(); }}>Show more ({(matchingRemote.length - remoteLimit).toLocaleString()} left)</Button></div>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}
  </div>
  {/key}
</div>

{#if preview}
  {@const p = preview}
  <Modal title={p.name} size="lg" onClose={() => preview = null}>
    {#if p.description}<p class="mb-3 text-sm leading-6 text-ink-300">{p.description}</p>{/if}
    <div class="mb-4 flex flex-wrap items-center gap-3 text-xs text-ink-500">
      {#if p.license}<span class="rounded-full border border-ink-700 px-2 py-0.5">License: {p.license}</span>{/if}
      <a href={p.url} rel="noreferrer" onclick={(e) => openInBrowser(e, p.url)} class="inline-flex items-center gap-1 hover:text-ember-300"><Icon name="external" class="h-3.5 w-3.5" />View the source</a>
    </div>
    <div class="rounded-md border border-ink-800 bg-ink-950 p-4">
      <MarkdownDoc source={p.body} />
    </div>
    {#snippet footer()}
      <Button variant="subtle" onclick={() => preview = null}>Close</Button>
      <Button variant="primary" disabled={importing !== null || owned.has(p.name.toLowerCase())} onclick={() => void importSkill(p.id)}>
        <Icon name="plus" class="h-4 w-4" />{owned.has(p.name.toLowerCase()) ? "Already saved" : importing ? "Saving…" : "Add to my skills"}
      </Button>
    {/snippet}
  </Modal>
{/if}

{#if pendingDeletePath}
  <ConfirmDialog title="Delete this skill?" message="This removes the saved prompt from this computer. You cannot undo it." confirmLabel="Delete skill" onConfirm={() => void confirmRemove()} onCancel={() => pendingDeletePath = null} />
{/if}
