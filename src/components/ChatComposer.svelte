<script lang="ts">
  // The bottom area: scroll pill, error banner, WorkStatus, plan card,
  // pinned chips, suggestions, undo, the queue list, the input, and the
  // disclaimer. Moved out of ChatScreen unchanged; the screen owns the
  // send loop and the data, this owns the markup. The running chat's
  // state arrives as one `run` instance (ChatRunState); the small
  // toggles this area flips itself (web, recall, files, shell, skills,
  // suggestions, planReady, the draft undo, the error banner) are
  // bindable props so the flips land in the screen.
  import { fade, fly } from "svelte/transition";
  import { dur, motion } from "../lib/motion";
  import { moduleOn } from "../lib/dev.svelte";
  import { requestSettings } from "../lib/nav.svelte";
  import type { ChatRunState } from "../lib/chatRun.svelte";
  import type {
    ApprovalMode,
    ChatAttachment,
    MessagePart,
    ModelChoice,
    ModelChoices,
    Skill,
    TodoItem,
  } from "../lib/api/bindings";
  import type { ContextSegment, ContextUsage } from "../lib/contextUsage";
  import ChatInput, { type ToolServer } from "./ChatInput.svelte";
  import ContextMeter from "./ContextMeter.svelte";
  import ModelPicker from "./ModelPicker.svelte";
  import PermissionMenu from "./PermissionMenu.svelte";
  import WorkStatus from "./WorkStatus.svelte";
  import ErrorBanner from "./ErrorBanner.svelte";
  import Icon from "./Icon.svelte";

  type ToolPart = Extract<MessagePart, { type: "tool" }>;

  interface Props {
    run: ChatRunState;
    railOpen: boolean;
    chatWidth: string;
    userScrolledUp: boolean;
    chatError: string | null;
    sendFailed: boolean;
    workTodos: TodoItem[] | null;
    workAgents: ToolPart[];
    runToolRounds: number;
    planReady: boolean;
    buildMode: ApprovalMode;
    pinnedTitles: string[];
    suggestions: string[];
    input: string;
    previousDraft: string | null;
    usage: ContextUsage;
    autoCompactAt: number;
    historyFrom: number;
    segments: ContextSegment[];
    lastCacheRate: number | null;
    ledgerCacheRate: number | null;
    approvalMode: ApprovalMode;
    effort: string;
    choices: ModelChoices | null;
    activeChoice: ModelChoice | null;
    pickerOpen: boolean;
    pickerBusy: boolean;
    engineLoading: boolean;
    refreshingProvider: string | null;
    toolPath: string;
    toolContent: string;
    showTools: boolean;
    pendingImages: string[];
    pendingFiles: ChatAttachment[];
    attaching: boolean;
    enhancing: boolean;
    memory: { usedGb: number; totalGb: number; loaded: boolean; modelName?: string | null } | null;
    inputSkills: Skill[];
    useWeb: boolean;
    webAvailable: boolean;
    useRecall: boolean;
    recallAvailable: boolean;
    useFiles: boolean;
    useShell: boolean;
    shellAvailable: boolean;
    useSkills: boolean;
    skillsAvailable: boolean;
    mcpServers: ToolServer[];
    mcpAvailable: boolean;
    onManageProviders?: () => void;
    onGetModels?: () => void;
    onScrollToBottom: () => void;
    onRetryLast: () => void;
    onBuildPlan: () => void;
    onSend: () => void;
    onCancel: () => void;
    onResume: () => void;
    onQueue: () => void;
    onSteer: () => void;
    onReadFile: () => void;
    onWriteFile?: () => void;
    onPickImages: (files: FileList | null) => void;
    onRemoveImage: (index: number) => void;
    onPickSkill: (body: string) => void;
    onSlashCommand: (command: string) => void;
    onEnhance: () => void;
    onLoadModel?: () => void;
    onUnloadModel?: () => void;
    onAttachFiles: () => void;
    onRemoveAttachment: (index: number) => void;
    onToggleMcp: (id: string) => void;
    onSelectModel: (choice: ModelChoice) => void;
    onToggleFavorite: (key: string) => void;
    onRefreshProvider: (providerId: string) => void;
    onChangeApprovalMode: (mode: ApprovalMode) => void;
    onEffortChange: (effort: string) => void;
    onCompact: () => void;
    onFocusComposer: () => void;
    onSendQueuedNow: (id: number) => void;
    onOpenAddProvider: () => void;
  }
  let {
    run,
    railOpen,
    chatWidth,
    userScrolledUp,
    chatError = $bindable(),
    sendFailed = $bindable(),
    workTodos,
    workAgents,
    runToolRounds,
    planReady = $bindable(),
    buildMode,
    pinnedTitles,
    suggestions = $bindable(),
    input = $bindable(),
    previousDraft = $bindable(),
    usage,
    autoCompactAt,
    historyFrom,
    segments,
    lastCacheRate,
    ledgerCacheRate,
    approvalMode,
    effort,
    choices,
    activeChoice,
    pickerOpen = $bindable(),
    pickerBusy,
    engineLoading,
    refreshingProvider,
    toolPath = $bindable(),
    toolContent = $bindable(),
    showTools = $bindable(),
    pendingImages,
    pendingFiles,
    attaching,
    enhancing,
    memory,
    inputSkills,
    useWeb = $bindable(),
    webAvailable,
    useRecall = $bindable(),
    recallAvailable,
    useFiles = $bindable(),
    useShell = $bindable(),
    shellAvailable,
    useSkills = $bindable(),
    skillsAvailable,
    mcpServers,
    mcpAvailable,
    onManageProviders,
    onGetModels,
    onScrollToBottom,
    onRetryLast,
    onBuildPlan,
    onSend,
    onCancel,
    onResume,
    onQueue,
    onSteer,
    onReadFile,
    onWriteFile,
    onPickImages,
    onRemoveImage,
    onPickSkill,
    onSlashCommand,
    onEnhance,
    onLoadModel,
    onUnloadModel,
    onAttachFiles,
    onRemoveAttachment,
    onToggleMcp,
    onSelectModel,
    onToggleFavorite,
    onRefreshProvider,
    onChangeApprovalMode,
    onEffortChange,
    onCompact,
    onFocusComposer,
    onSendQueuedNow,
    onOpenAddProvider,
  }: Props = $props();
</script>

<div class={`shrink-0 bg-gradient-to-t from-ink-900 via-ink-900 to-ink-900/0 px-3 pb-3 pt-2 transition-[padding] duration-200 md:px-6 ${railOpen ? "lg:pr-[19.5rem]" : ""}`}>
  <div class={`relative mx-auto ${chatWidth}`}>
    {#if userScrolledUp}
      <button
        type="button"
        onclick={onScrollToBottom}
        aria-label="Scroll to the newest message"
        in:fly={{ y: 8, duration: dur(motion.base) }}
        out:fade={{ duration: dur(100) }}
        class="absolute -top-12 right-2 z-10 grid h-9 w-9 place-items-center rounded-full border border-edge bg-ink-800/95 text-ink-100 shadow-xl transition-colors hover:bg-ink-700"
      ><Icon name="arrow-down" class="h-4 w-4" /></button>
    {/if}
    {#if chatError}
      <div class="mb-2"><ErrorBanner message={chatError} onRetry={sendFailed && !run.streaming ? onRetryLast : undefined} onDismiss={() => { chatError = null; sendFailed = false; }} /></div>
    {/if}
    <WorkStatus todos={workTodos} subagents={workAgents} startedAt={run.runStartedAt} toolRounds={runToolRounds} live={run.streaming} />
    {#if planReady && !run.streaming}
      <div class="mb-2 flex items-center gap-3 rounded-xl border border-sky-400/30 bg-sky-400/10 px-3 py-2 text-sm" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
        <Icon name="lightbulb" class="h-4 w-4 shrink-0 text-sky-300" />
        <span class="min-w-0 flex-1 text-ink-200">The plan is ready. Build carries it out with the {buildMode === "auto" ? "Full access" : buildMode === "smart" ? "Autonomous" : "Ask before changes"} permission.</span>
        <button type="button" onclick={() => (planReady = false)} class="rounded-lg px-2.5 py-1 text-ink-300 hover:bg-veil/5">Keep planning</button>
        <button type="button" onclick={onBuildPlan} class="rounded-lg bg-ember-500 px-3 py-1 font-semibold text-on-ember hover:bg-ember-400">Build</button>
      </div>
    {/if}
    {#if useRecall && pinnedTitles.length > 0}
      <div class="mb-2 flex flex-wrap items-center gap-1.5" in:fly={{ y: 4, duration: dur(motion.base), easing: motion.enter }}>
        {#each pinnedTitles as title (title)}
          <span class="pinned-chip" title="Pinned note — always in context while Recall is on"><Icon name="pin" class="h-3 w-3 shrink-0 text-ember-300" /><span class="max-w-48 truncate">{title}</span></span>
        {/each}
      </div>
    {/if}
    {#if suggestions.length > 0 && !input.trim() && !run.streaming}
      <div class="mb-2 flex flex-wrap items-stretch gap-2" in:fly={{ y: 4, duration: dur(motion.base), easing: motion.enter }}>
        {#each suggestions as option, i (option)}
          <button type="button" class="suggest-chip min-w-0 flex-1 basis-0" onclick={() => { input = option; onFocusComposer(); }} title={`Use this prompt (${i === 0 ? "Tab" : "click"})`}><Icon name="sparkle" class="h-3.5 w-3.5 shrink-0 text-ember-300" /><span class="min-w-0 flex-1 truncate">{option}</span></button>
        {/each}
        <button type="button" aria-label="Hide the suggestions" class="grid h-8 w-8 shrink-0 place-items-center rounded-md text-ink-500 hover:bg-veil/5 hover:text-ink-200" onclick={() => (suggestions = [])}><Icon name="close" class="h-3.5 w-3.5" /></button>
      </div>
    {/if}
    {#if previousDraft && input.trim() && !run.streaming}
      <div class="mb-2 flex items-center gap-2 text-xs text-ink-400">
        <Icon name="wand" class="h-3.5 w-3.5 text-ember-300" />Improved by the model.
        <button type="button" class="underline underline-offset-2 hover:text-ink-100" onclick={() => { input = previousDraft ?? input; previousDraft = null; }}>Undo</button>
      </div>
    {/if}
    {#if run.queued.length > 0 || run.steering.length > 0}
      <ul class="mb-2 space-y-1" aria-label="Messages waiting to be sent">
        {#each run.steering as note (note.id)}
          <li class="note-row">
            <Icon name="wand" class="h-3.5 w-3.5 shrink-0 text-ember-300" />
            <span class="min-w-0 flex-1 truncate" title={note.text}>{note.text}</span>
            <span class="shrink-0 text-xs text-ink-500">Steering · read at the next step</span>
          </li>
        {/each}
        {#each run.queued as note, i (note.id)}
          <li class="note-row">
            <span class="shrink-0 text-xs tabular-nums text-ink-500">{i + 1}</span>
            {#if run.editingNoteId === note.id}
              <input
                type="text"
                bind:value={run.editingNoteText}
                aria-label="Edit this run.queued message"
                class="min-w-0 flex-1 rounded-md border border-edge bg-ink-950 px-2 py-1 text-sm text-ink-100 outline-none focus:border-ember-500"
                onkeydown={(e) => { if (e.key === "Enter") run.saveNoteEdit(); else if (e.key === "Escape") run.cancelNoteEdit(); }}
              />
              <button type="button" class="note-action" onclick={() => run.saveNoteEdit()} disabled={!run.editingNoteText.trim()}>Save</button>
              <button type="button" class="note-action" onclick={() => run.cancelNoteEdit()}>Cancel</button>
            {:else}
              <span class="min-w-0 flex-1 truncate" title={note.text}>{note.text}</span>
              <button type="button" class="note-action" onclick={() => run.moveNote(note.id, -1)} disabled={i === 0} aria-label="Move this run.queued message earlier" title="Move earlier">↑</button>
              <button type="button" class="note-action" onclick={() => run.moveNote(note.id, 1)} disabled={i === run.queued.length - 1} aria-label="Move this run.queued message later" title="Move later">↓</button>
              <button type="button" class="note-action" onclick={() => run.startNoteEdit(note.id)} title="Edit this run.queued message here">Edit</button>
              <button type="button" class="note-action" onclick={() => onSendQueuedNow(note.id)} title={run.streaming ? "Give it to the running answer now" : "Send it now"}>{run.streaming ? "Steer now" : "Send now"}</button>
            {/if}
            <button type="button" aria-label="Remove this run.queued message" class="grid h-6 w-6 shrink-0 place-items-center rounded text-ink-500 hover:bg-veil/5 hover:text-ink-200" onclick={() => { if (run.editingNoteId === note.id) run.cancelNoteEdit(); run.removeNote(note.id); }}><Icon name="close" class="h-3 w-3" /></button>
          </li>
        {/each}
      </ul>
    {/if}
    <ChatInput
      bind:input={input}
      bind:toolPath={toolPath}
      bind:toolContent={toolContent}
      bind:showTools={showTools}
      streaming={run.streaming}
      onSend={onSend}
      onCancel={onCancel}
      canResume={run.stoppedAnswer}
      onResume={onResume}
      onQueue={onQueue}
      onSteer={onSteer}
      onReadFile={onReadFile}
      onWriteFile={onWriteFile}
      pendingImages={pendingImages}
      onPickImages={onPickImages}
      onRemoveImage={onRemoveImage}
      skills={inputSkills}
      onPickSkill={onPickSkill}
      onSlashCommand={onSlashCommand}
      onEnhance={onEnhance}
      {enhancing}
      memory={memory}
      onLoadModel={onLoadModel}
      onUnloadModel={onUnloadModel}
      {suggestions}
      {useWeb}
      {webAvailable}
      onToggleWeb={() => (useWeb = !useWeb)}
      {useRecall}
      {recallAvailable}
      onToggleRecall={() => (useRecall = !useRecall)}
      attachments={pendingFiles}
      {attaching}
      onAttachFiles={onAttachFiles}
      onRemoveAttachment={onRemoveAttachment}
      {useFiles}
      onToggleFiles={() => (useFiles = !useFiles)}
      {useShell}
      {shellAvailable}
      onToggleShell={() => (useShell = !useShell)}
      {useSkills}
      {skillsAvailable}
      onToggleSkills={() => (useSkills = !useSkills)}
      mcpServers={mcpServers}
      {mcpAvailable}
      onToggleMcp={onToggleMcp}
      onManageTools={() => requestSettings("tools")}
    >
      {#snippet leading()}
        <PermissionMenu mode={approvalMode} onChange={onChangeApprovalMode} />
      {/snippet}
      {#snippet trailing()}
        <ContextMeter {usage} streaming={run.streaming} {autoCompactAt} compacted={historyFrom > 0} {segments} cacheRate={lastCacheRate} cacheRateAverage={ledgerCacheRate} onCompact={onCompact} />
        <ModelPicker
          {effort}
          onEffort={onEffortChange}
          {choices}
          activeKey={activeChoice?.key ?? null}
          bind:open={pickerOpen}
          busy={pickerBusy || engineLoading}
          onSelect={onSelectModel}
          onToggleFavorite={onToggleFavorite}
          onRefresh={onRefreshProvider}
          refreshing={refreshingProvider}
          onManageProviders={moduleOn("online") ? onOpenAddProvider : undefined}
          onGetModels={onGetModels}
        />
      {/snippet}
    </ChatInput>
    <p class="mt-2 text-center text-xs text-ink-500">LocalAide can make mistakes. Check important facts.</p>
  </div>
</div>

<style>
  .suggest-chip {
    display: inline-flex;
    min-width: 0;
    max-width: 100%;
    align-items: center;
    gap: 0.45rem;
    border-radius: 9999px;
    border: 1px dashed var(--color-ink-700);
    padding: 0.3rem 0.75rem;
    font-size: 0.8125rem;
    color: var(--color-ink-300);
    transition: border-color 150ms ease-out, color 150ms ease-out, background-color 150ms ease-out;
  }
  .suggest-chip:hover {
    border-style: solid;
    border-color: var(--color-edge);
    background: color-mix(in oklab, var(--color-veil) 4%, transparent);
    color: var(--color-ink-50);
  }
  .note-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border: 1px solid var(--color-edge);
    border-radius: 0.6rem;
    background: var(--color-ink-950);
    padding: 0.3rem 0.4rem 0.3rem 0.65rem;
    font-size: 0.8125rem;
    color: var(--color-ink-200);
  }
  .note-action {
    flex-shrink: 0;
    border-radius: 0.375rem;
    padding: 0.125rem 0.45rem;
    font-size: 0.75rem;
    color: var(--color-ink-300);
  }
  .note-action:hover {
    background: color-mix(in oklab, var(--color-veil) 7%, transparent);
    color: var(--color-ink-50);
  }
  .pinned-chip {
    display: inline-flex;
    min-width: 0;
    max-width: 100%;
    align-items: center;
    gap: 0.4rem;
    border-radius: 9999px;
    border: 1px solid color-mix(in oklab, var(--color-ember-500) 40%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 8%, transparent);
    padding: 0.25rem 0.65rem;
    font-size: 0.75rem;
    color: var(--color-ink-300);
  }
</style>
