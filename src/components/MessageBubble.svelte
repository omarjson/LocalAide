<script lang="ts">
  import FocusTrap from "./FocusTrap.svelte";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";
  import AnswerText from "./AnswerText.svelte";
  import ThinkingBlock from "./ThinkingBlock.svelte";
  import ToolCallCard from "./ToolCallCard.svelte";
  import QuestionCard from "./QuestionCard.svelte";
  import QuestionDock from "./QuestionDock.svelte";
  import TodoList from "./TodoList.svelte";
  import { questionItems } from "../lib/chatParts";
  import { parseQuestion } from "../lib/questions";
  import { splitEventMessage } from "../lib/a2ui";
  import { openInBrowser } from "../lib/links";
  import { dur, motion } from "../lib/motion";
  import { flag } from "../lib/dev.svelte";
  import { speak, speechAvailable, stopSpeaking } from "../lib/speech";
  import { voice } from "../lib/voice.svelte";
  import { onDestroy } from "svelte";
  import { fade, fly, scale } from "svelte/transition";
  import type { ChatAttachment, Citation, MessagePart } from "../lib/api/bindings";
  import type { AnswerStats } from "../lib/chatModes";

  interface Props {
    role: string;
    content: string;
    ts?: number;
    streaming?: boolean;
    citations?: Citation[];
    onRetry?: () => void;
    onEdit?: (newContent: string) => void;
    images?: string[];
    /** What LocalAide did before answering, newest last. */
    steps?: string[];
    stats?: AnswerStats | null;
    /** Developer flag: allow the plain-text view. */
    allowRaw?: boolean;
    onSendToOpenCode?: () => void;
    /** The newest message keeps its actions visible. */
    isLast?: boolean;
    /** Later messages parked here as a branch when the chat was edited. */
    branchCount?: number;
    /** Swaps the parked branch back into the chat. */
    onRestoreBranch?: () => void;
    /** The answer in order: thinking, tools, text. */
    parts?: MessagePart[];
    attachments?: ChatAttachment[];
    /** Which model wrote the answer. */
    model?: string;
    /** Tool calls waiting for the user, by tool call id. */
    approvals?: Record<string, { approval_id: string; preview: string }>;
    onDecide?: (toolId: string, decision: "allow" | "allow_chat" | "deny") => void;
    /** Set on the newest answer: send a tapped option as the reply. */
    onAnswer?: (text: string, questionId: string | null) => void;
    /** A button pressed on an A2UI form in this answer. */
    onA2uiAction?: (message: string) => void;
    /** "Other…": put the cursor in the chat box. */
    onOther?: () => void;
    /** Dismiss an ask_user question without answering. */
    onDismissQuestion?: (questionId: string) => void;
    /** Undo a skill the model saved, by tool call id. */
    onUndoSkill?: (toolId: string) => void;
    /** The chat's project folder, for file buttons. */
    folder?: string | null;
    /** An earlier question the user moved past can still be answered. */
    onAnswerDeferred?: (text: string, questionId: string) => void;
    /** Chat setting: questions longer than a few lines start collapsed. */
    collapseLong?: boolean;
    /** A steering note sent to a running answer: shown small, on the left. */
    steer?: boolean;
  }

  let {
    role,
    content,
    ts,
    streaming = false,
    citations = [],
    onRetry,
    onEdit,
    images = [],
    steps = [],
    stats = null,
    allowRaw = false,
    onSendToOpenCode,
    isLast = false,
    branchCount = 0,
    onRestoreBranch,
    parts = [],
    attachments = [],
    model = "",
    approvals = {},
    onDecide,
    onAnswer,
    onA2uiAction,
    onOther,
    onDismissQuestion,
    onUndoSkill,
    folder = null,
    onAnswerDeferred,
    collapseLong = false,
    steer = false,
  }: Props = $props();

  // A long question starts collapsed when the reading setting is on and
  // expands in place; editing always sees the full text.
  const LONG_QUESTION = 240;
  let questionExpanded = $state(false);
  const longQuestion = $derived(role === "user" && collapseLong && content.length > LONG_QUESTION);
  // A message sent from an A2UI form shows its readable line; the A2UI
  // envelope that rides along for the model shows as a small tag.
  const sentFromForm = $derived(role === "user" ? splitEventMessage(content) : { text: content, event: false });
  const shownContent = $derived(
    longQuestion && !questionExpanded ? sentFromForm.text.slice(0, LONG_QUESTION).trimEnd() + "…" : sentFromForm.text,
  );

  // A question at the end of a plain answer gets buttons too, when the
  // model did not use ask_user.
  const parsedQuestion = $derived(
    role === "assistant" && isLast && !streaming && onAnswer && !parts.some((p) => p.type === "question")
      ? parseQuestion(content)
      : null,
  );

  let copied = $state(false);
  let copyError = $state<string | null>(null);
  let isEditing = $state(false);
  let editDraft = $state("");
  let editEl = $state<HTMLTextAreaElement>();
  let activeCitation = $state<Citation | null>(null);
  let lightbox = $state<string | null>(null);
  let raw = $state(false);
  /** Sources stay folded until the answer is done, then open on a tap. */
  let sourcesOpen = $state(false);
  let speaking = $state(false);
  // Settings → Voice → Read answers aloud, where the system has a voice.
  const canSpeak = $derived(!!voice.settings?.read_aloud && speechAvailable());

  const toggleSpeak = () => {
    if (speaking) {
      stopSpeaking();
      speaking = false;
      return;
    }
    speaking = true;
    speak(content, () => { speaking = false; }, { voiceName: voice.settings?.voice_name, rate: voice.settings?.rate });
  };

  onDestroy(() => { if (speaking) stopSpeaking(); });

  $effect(() => {
    if (isEditing) editEl?.focus();
  });

  const copyMessage = async () => {
    try {
      await navigator.clipboard.writeText(content);
      copied = true;
      copyError = null;
      setTimeout(() => { copied = false; }, 1600);
    } catch {
      copyError = "Copy failed. Try again.";
    }
  };

  const submitEdit = () => {
    if (editDraft.trim() && onEdit) onEdit(editDraft);
    isEditing = false;
  };

  const host = (source: string): string => {
    try {
      return new URL(source).hostname.replace(/^www\./, "");
    } catch {
      return source.split(/[\\/]/).pop() ?? source;
    }
  };
  const isWeb = (source: string) => /^https?:\/\//.test(source);

  const openCitation = (event: MouseEvent, cit: Citation) => {
    if (isWeb(cit.source)) openInBrowser(event, cit.source);
    else activeCitation = cit;
  };

  // After a tool finishes, the model reads its result before it writes again.
  const betweenRounds = $derived.by(() => {
    const last = parts.at(-1);
    return last?.type === "tool" && last.status !== "running" && last.status !== "waiting";
  });

  const speed = $derived(stats && stats.ms > 0 ? (stats.tokens / (stats.ms / 1000)).toFixed(1) : null);
  const timeLabel = $derived(ts ? new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "");
</script>

{#if role === "user"}
  <div class={`group flex ${steer ? "justify-start" : "justify-end"}`} in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
    {#if branchCount > 0}
      <div class="mb-1 flex w-full items-center justify-end gap-2 text-xs text-ink-500">
        <span class="flex items-center gap-1"><Icon name="undo" class="h-3 w-3" />{branchCount} later {branchCount === 1 ? "message" : "messages"} branched here</span>
        {#if onRestoreBranch}
          <button type="button" class="rounded-md border border-ink-700 px-2 py-0.5 text-ink-300 transition-colors hover:bg-veil/5 hover:text-ink-50" onclick={onRestoreBranch}>Restore</button>
        {/if}
      </div>
    {/if}
    <div class={`flex max-w-[82%] flex-col ${steer ? "items-start" : "items-end"}`}>
      {#if isEditing}
        <div class="w-[min(36rem,80vw)] rounded-2xl border border-edge bg-ink-950 p-3">
          <textarea
            bind:this={editEl}
            bind:value={editDraft}
            rows={3}
            aria-label="Edit your message"
            onkeydown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) submitEdit(); if (e.key === "Escape") isEditing = false; }}
            class="field-bare w-full resize-y bg-transparent text-[15px] leading-6 text-ink-100"
          ></textarea>
          <div class="mt-2 flex justify-end gap-2">
            <button type="button" onclick={() => isEditing = false} class="rounded-lg px-3 py-1.5 text-sm text-ink-300 hover:bg-veil/5">Cancel</button>
            <button type="button" onclick={submitEdit} class="rounded-lg bg-ember-500 px-3 py-1.5 text-sm font-semibold text-on-ember hover:bg-ember-400">Send</button>
          </div>
        </div>
      {:else}
        {#if images.length > 0}
          <div class="mb-2 flex flex-wrap justify-end gap-2">
            {#each images as src, i (i)}
              <button type="button" onclick={() => (lightbox = src)} aria-label="View image {i + 1}" class="block cursor-zoom-in overflow-hidden rounded-xl border border-ink-700">
                <img src={src} alt="Attachment {i + 1}" class="max-h-48 object-cover transition-transform duration-300 hover:scale-[1.02]" />
              </button>
            {/each}
          </div>
        {/if}
        {#if attachments.length > 0}
          <div class="mb-2 flex flex-wrap justify-end gap-2">
            {#each attachments as file, i (file.name + i)}
              <span class="flex max-w-[16rem] items-center gap-2 rounded-lg border border-ink-700 bg-ink-900 px-2.5 py-1.5 text-sm text-ink-200" title={file.name}>
                <Icon name="file" class="h-4 w-4 shrink-0 text-ink-400" /><span class="truncate">{file.name}</span>
              </span>
            {/each}
          </div>
        {/if}
        {#if content}
          {#if steer}
            <span class="mb-1 flex items-center gap-1.5 text-xs text-ink-500"><Icon name="wand" class="h-3.5 w-3.5 text-ember-300" />Steered this answer{#if timeLabel}<span>· {timeLabel}</span>{/if}</span>
          {/if}
          <div class={`whitespace-pre-wrap break-words rounded-[18px] px-4 py-2.5 text-[15px] leading-6 ${steer ? "rounded-bl-md border border-edge bg-ink-900 text-sm text-ink-300" : "rounded-br-md border border-ember-500/25 bg-ember-500/12 text-ink-50"}`}>{shownContent}{#if sentFromForm.event}<span class="mt-1 flex items-center gap-1 text-xs text-ember-200/80"><Icon name="sparkle" class="h-3 w-3" />Sent from the form</span>{/if}</div>
          {#if longQuestion && !questionExpanded}
            <button type="button" onclick={() => (questionExpanded = true)} class="mt-0.5 text-xs font-medium text-ember-300 transition-colors hover:text-ember-200">Show full question</button>
          {/if}
        {/if}
        <div class="mt-1 flex items-center gap-0.5 text-ink-500 opacity-0 transition-opacity duration-150 focus-within:opacity-100 group-hover:opacity-100">
          {#if timeLabel}<span class="px-1.5 text-xs">{timeLabel}</span>{/if}
          <button type="button" onclick={copyMessage} class="msg-action" aria-label="Copy message" title="Copy"><Icon name={copied ? "check" : "copy"} class="h-4 w-4" /></button>
          {#if onEdit}<button type="button" onclick={() => { editDraft = content; isEditing = true; }} class="msg-action" aria-label="Edit and resend" title="Edit"><Icon name="pencil" class="h-4 w-4" /></button>{/if}
          {#if onSendToOpenCode}<button type="button" onclick={onSendToOpenCode} class="msg-action" aria-label="Send to OpenCode" title="Send this task to OpenCode"><Icon name="code" class="h-4 w-4" /></button>{/if}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="group flex gap-3" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
    <div class="mt-0.5 shrink-0"><LogoMark class="h-7 w-7" /></div>
    <div class="min-w-0 flex-1">
      <div class="mb-1 flex h-7 items-center gap-2 text-sm">
        <span class="font-semibold text-ink-100">LocalAide</span>
        {#if model}<span class="truncate rounded-full border border-ink-800 px-2 py-px text-xs leading-4 text-ink-400" title="The model that wrote this answer">{model}</span>{/if}
        {#if timeLabel}<span class="shrink-0 text-xs text-ink-500">{timeLabel}</span>{/if}
      </div>

      {#if steps.length > 0}
        <ul class="mb-2 space-y-1" aria-label="Notes about this answer">
          {#each steps as step, i (i)}
            {@const active = streaming && i === steps.length - 1 && !content && parts.length === 0}
            <li class="flex items-center gap-2 text-sm text-ink-400" in:fly={{ x: -4, duration: dur(motion.base) }}>
              {#if active}
                <span class="h-3.5 w-3.5 shrink-0 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
              {:else}
                <Icon name="check" class="h-3.5 w-3.5 shrink-0 text-emerald-400/80" />
              {/if}
              <span class={active ? "text-ink-200" : ""}>{step}</span>
            </li>
          {/each}
        </ul>
      {/if}

      {#if streaming && !content.trim() && parts.length === 0}
        <div class="typing flex h-7 items-center gap-1.5" role="status" aria-label="LocalAide is thinking">
          <span></span><span></span><span></span>
        </div>
      {/if}

      {#if raw}
        <pre class="whitespace-pre-wrap break-words rounded-xl border border-ink-800 bg-ink-950 p-3 font-mono text-[13px] leading-6 text-ink-200">{content}</pre>
      {:else if parts.length > 0}
        <div class="space-y-2">
          {#each parts as part, i (i)}
            {#if part.type === "reasoning"}
              <ThinkingBlock text={part.text} ms={part.ms ?? null} live={streaming && i === parts.length - 1} />
            {:else if part.type === "question"}
              <QuestionDock
                items={questionItems(part)}
                answer={part.answer ?? null}
                deferred={part.deferred ?? false}
                dismissed={part.dismissed ?? false}
                onAnswer={isLast && !streaming && onAnswer
                  ? (text) => onAnswer?.(text, part.id)
                  : part.deferred && onAnswerDeferred
                    ? (text) => onAnswerDeferred?.(text, part.id)
                    : undefined}
                onDismiss={() => onDismissQuestion?.(part.id)}
              />
            {:else if part.type === "todos"}
              <TodoList items={part.items} />
            {:else if part.type === "tool"}
              <ToolCallCard
                title={part.title}
                detail={part.detail}
                group={part.group}
                status={part.status}
                summary={part.summary}
                arguments={part.arguments}
                output={part.output}
                approval={approvals[part.id] ? { preview: approvals[part.id].preview } : null}
                onDecide={(decision) => onDecide?.(part.id, decision)}
                steps={part.steps ?? []}
                skill={part.skill ?? null}
                onUndoSkill={onUndoSkill ? () => onUndoSkill?.(part.id) : undefined}
                {folder}
              />
            {:else if part.type === "text" && part.text.trim()}
              <div class={i > 0 ? "pt-1" : ""}>
                <AnswerText text={part.text} caret={streaming && i === parts.length - 1} onChoose={isLast && !streaming && onAnswer ? (text) => onAnswer?.(text, null) : undefined} {onA2uiAction} />
              </div>
            {/if}
          {/each}
          {#if streaming && betweenRounds}
            <div class="typing flex h-6 items-center gap-1.5 pl-1" aria-hidden="true"><span></span><span></span><span></span></div>
          {/if}
        </div>
      {:else}
        <AnswerText text={content} caret={streaming && !!content.trim()} onChoose={isLast && !streaming && onAnswer ? (text) => onAnswer?.(text, null) : undefined} {onA2uiAction} />
      {/if}

      {#if parsedQuestion}
        <div class="mt-3">
          <QuestionCard
            question={parsedQuestion.question}
            options={parsedQuestion.options}
            multiple={parsedQuestion.multiple}
            showQuestion={false}
            onAnswer={(text) => onAnswer?.(text, null)}
            {onOther}
          />
        </div>
      {/if}

      {#if citations.length > 0 && !streaming}
        <div class="mt-3" aria-label="Sources">
          <button
            type="button"
            class="flex items-center gap-1.5 text-xs text-ink-500 transition-colors hover:text-ink-200"
            aria-expanded={sourcesOpen}
            onclick={() => (sourcesOpen = !sourcesOpen)}
          >
            <Icon name="layers" class="h-3.5 w-3.5" />Sources ({citations.length})
            <Icon name="chevron-down" class={`h-3 w-3 transition-transform ${sourcesOpen ? "rotate-180" : ""}`} />
          </button>
          {#if sourcesOpen}
            <div class="mt-1.5 flex flex-wrap items-center gap-1.5">
              {#each citations as cit (cit.index)}
                <button
                  type="button"
                  class="reference-chip"
                  aria-label={`Source ${cit.index}: ${cit.title}`}
                  title={`${cit.title} — ${host(cit.source)}`}
                  onclick={(e) => openCitation(e, cit)}
                >[{cit.index}]</button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      {#if !streaming && content}
        <div class={`mt-2 flex flex-wrap items-center gap-0.5 text-ink-500 transition-opacity duration-150 focus-within:opacity-100 group-hover:opacity-100 ${isLast ? "opacity-100" : "opacity-0"}`}>
          <button type="button" onclick={copyMessage} class="msg-action" aria-label="Copy answer" title="Copy"><Icon name={copied ? "check" : "copy"} class="h-4 w-4" /></button>
          {#if canSpeak && !streaming && content.trim()}<button type="button" onclick={toggleSpeak} aria-pressed={speaking} class="msg-action" aria-label={speaking ? "Stop reading aloud" : "Read aloud"} title={speaking ? "Stop reading" : "Read aloud"}><Icon name={speaking ? "stop" : "volume"} class="h-4 w-4" /></button>{/if}
          {#if onRetry}<button type="button" onclick={onRetry} class="msg-action" aria-label="Answer again" title="Answer again"><Icon name="refresh" class="h-4 w-4" /></button>{/if}
          {#if allowRaw}<button type="button" onclick={() => raw = !raw} aria-pressed={raw} class="msg-action" aria-label="Show plain text" title="Plain text"><Icon name="eye" class="h-4 w-4" /></button>{/if}
          {#if onSendToOpenCode}<button type="button" onclick={onSendToOpenCode} class="msg-action" aria-label="Send to OpenCode" title="Continue this in OpenCode"><Icon name="code" class="h-4 w-4" /></button>{/if}
          {#if stats}
            <span class="ml-1 font-mono text-xs text-ink-500" title="Answer statistics">{stats.inputTokens !== undefined ? `${stats.inputTokens} in · ${stats.outputTokens ?? stats.tokens} out` : `${stats.tokens} tokens`} · {(stats.ms / 1000).toFixed(1)} s{speed ? ` · ${speed} tok/s` : ""}{stats.firstTokenMs !== null ? ` · first token ${(stats.firstTokenMs / 1000).toFixed(1)} s` : ""}</span>
          {/if}
          {#if copyError}<span role="alert" class="ml-1 text-xs text-rose-300">{copyError}</span>{/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if lightbox}
  <div
    class="fixed inset-0 z-[60] flex items-center justify-center bg-black/90 p-6"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={() => (lightbox = null)}
    onkeydown={(e) => { if (e.key === 'Escape') lightbox = null; }}
  >
    <FocusTrap>
      <button type="button" class="max-h-[85vh] max-w-[92vw] cursor-zoom-out border-none bg-transparent p-0" title="Close" onclick={(e) => e.stopPropagation()}>
        <img src={lightbox} alt="" class="max-h-[85vh] max-w-[92vw] rounded-xl border border-edge" />
      </button>
      <button type="button" onclick={() => (lightbox = null)} aria-label="Close fullscreen image" class="absolute right-5 top-5 rounded-xl border border-edge bg-ink-900 p-2 text-ink-300 transition-colors hover:text-ink-100">
        <Icon name="close" class="h-4 w-4" />
      </button>
    </FocusTrap>
  </div>
{/if}

{#if activeCitation}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={() => activeCitation = null}
    onkeydown={(e) => { if (e.key === 'Escape') activeCitation = null; }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Source"
      tabindex="-1"
      in:scale={{ start: 0.96, duration: dur(motion.base) }}
      out:scale={{ start: 0.98, duration: dur(80) }}
      class="w-full max-w-lg space-y-3 rounded-xl border border-edge bg-ink-900 p-5 shadow-2xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => { if (e.key === 'Escape') activeCitation = null; e.stopPropagation(); }}
    >
      <FocusTrap>
        <div class="flex items-center justify-between border-b border-ink-800 pb-2">
          <div class="flex min-w-0 items-center gap-2">
            <span class="rounded bg-ember-500/15 px-2 py-0.5 text-xs font-semibold text-ember-300">Source {activeCitation.index}</span>
            <span class="truncate text-sm font-medium text-ink-100">{activeCitation.title}</span>
          </div>
          <button type="button" aria-label="Close" onclick={() => activeCitation = null} class="rounded p-1 text-ink-400 hover:text-ink-100"><Icon name="close" class="h-4 w-4" /></button>
        </div>
        <div class="break-all font-mono text-xs text-ink-500">{activeCitation.source}</div>
        <div class="max-h-60 overflow-y-auto rounded-lg border border-ink-800 bg-ink-950 p-3">
          <p class="whitespace-pre-wrap text-sm leading-6 text-ink-300">{activeCitation.snippet}</p>
        </div>
        <div class="flex justify-end">
          <button type="button" onclick={() => activeCitation = null} class="rounded-lg bg-ink-800 px-3 py-1.5 text-sm text-ink-100 hover:bg-ink-700">Close</button>
        </div>
      </FocusTrap>
    </div>
  </div>
{/if}

<style>
  .reference-chip {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    border-radius: 0.375rem;
    border: 1px solid color-mix(in oklab, var(--color-ember-500) 25%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 12%, transparent);
    padding: 0.05rem 0.45rem;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.75rem;
    line-height: 1.15rem;
    color: var(--color-ember-200);
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
  }
  .reference-chip:hover {
    background: color-mix(in oklab, var(--color-ember-500) 20%, transparent);
    border-color: color-mix(in oklab, var(--color-ember-500) 40%, transparent);
  }
  .msg-action {
    display: grid;
    height: 1.875rem;
    width: 1.875rem;
    place-items: center;
    border-radius: 0.5rem;
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .msg-action:hover,
  .msg-action[aria-pressed="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .typing span {
    height: 0.45rem;
    width: 0.45rem;
    border-radius: 9999px;
    background: var(--color-ink-400);
    will-change: transform, opacity;
    animation: typing-dot 1.2s ease-in-out infinite;
  }
  .typing span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .typing span:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes typing-dot {
    0%,
    80%,
    100% {
      opacity: 0.3;
      transform: translateY(0);
    }
    40% {
      opacity: 1;
      transform: translateY(-3px);
    }
  }
</style>
