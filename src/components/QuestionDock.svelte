<script lang="ts">
  // Questions the model asked with ask_user: one or more questions, each
  // with options to pick (with a short description), an "Other" answer,
  // Dismiss and Submit. A question the user moved past by sending another
  // message stays open, with a note that the answer goes as a new message.
  import { slide } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import { orderOptionsRecommended } from "../lib/questions";
  import type { QuestionItem } from "../lib/api/bindings";

  interface Props {
    items: QuestionItem[];
    /** The answer that was sent, once answered. */
    answer?: string | null;
    deferred?: boolean;
    dismissed?: boolean;
    /** Set while the questions can be answered. */
    onAnswer?: (text: string) => void;
    onDismiss?: () => void;
  }

  let { items, answer = null, deferred = false, dismissed = false, onAnswer, onDismiss }: Props = $props();

  const active = $derived(!!onAnswer && !answer && !dismissed);
  let collapsed = $state(false);
  let step = $state(0);
  // Per question: the picked option labels, and the typed answer.
  let picked = $state<string[][]>([]);
  let other = $state<(string | null)[]>([]);
  $effect(() => {
    if (picked.length !== items.length) {
      picked = items.map(() => []);
      other = items.map(() => null);
    }
  });
  const current = $derived(items[Math.min(step, items.length - 1)]);
  const last = $derived(step >= items.length - 1);
  // A recommended option stands first and is marked; the rest keep the
  // model's order. The "Other" free-text answer stays last, after them.
  const shownOptions = $derived(current ? orderOptionsRecommended(current.options) : []);

  const answered = (i: number) => (picked[i]?.length ?? 0) > 0 || !!other[i]?.trim();
  const canSubmit = $derived(items.every((_, i) => answered(i)));

  const toggle = (label: string) => {
    const i = step;
    const q = items[i];
    const now = picked[i] ?? [];
    if (q.multiple) {
      picked[i] = now.includes(label) ? now.filter((l) => l !== label) : [...now, label];
    } else {
      picked[i] = [label];
      other[i] = null;
    }
  };

  const chooseOther = () => {
    const i = step;
    other[i] = other[i] ?? "";
    if (!items[i].multiple) picked[i] = [];
    queueMicrotask(() => document.getElementById(`q-other-${i}`)?.focus());
  };

  const answerFor = (i: number): string => {
    const parts = [...(picked[i] ?? [])];
    const typed = other[i]?.trim();
    if (typed) parts.push(typed);
    return parts.join(", ");
  };

  const submit = () => {
    if (!active || !canSubmit) return;
    let text: string;
    if (items.length === 1) {
      text = answerFor(0);
    } else {
      text = items.map((q, i) => `${q.header || q.question}: ${answerFor(i)}`).join("\n");
    }
    if (deferred) text = `Answer to your earlier question:\n${text}`;
    onAnswer?.(text);
  };

  const next = () => {
    if (answered(step) && !last) step += 1;
  };

  const onKey = (e: KeyboardEvent) => {
    if (!active) return;
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submit();
    } else if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement)) {
      e.preventDefault();
      if (last) submit();
      else next();
    }
  };
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class={`qd ${active ? "qd-live" : ""}`} aria-label="Questions" onkeydown={onKey}>
  <header class="qd-head">
    <Icon name="help" class="h-4 w-4 shrink-0 text-ember-300" />
    <button type="button" class="qd-toggle" aria-expanded={!collapsed} onclick={() => (collapsed = !collapsed)}>
      <Icon name="chevron-down" class={`h-3.5 w-3.5 transition-transform duration-200 ${collapsed ? "-rotate-90" : ""}`} />
      <span class="font-semibold text-ink-100">Questions</span>
    </button>
    <span class="text-xs tabular-nums text-ink-500">{active ? `${step + 1} of ${items.length}` : answer ? "Answered" : dismissed ? "Dismissed" : `${items.length}`}</span>
    {#if active && items.length > 1}
      <span class="ml-auto flex items-center gap-1" aria-hidden="true">
        {#each items as _, i (i)}
          <span class={`h-1.5 w-1.5 rounded-full ${i === step ? "bg-ember-400" : answered(i) ? "bg-ink-400" : "bg-ink-700"}`}></span>
        {/each}
      </span>
    {/if}
  </header>

  {#if !collapsed}
    <div transition:slide={{ duration: dur(motion.base) }}>
      {#if active && deferred}
        <p class="qd-note" role="note">You sent a message before answering. Submit sends your answer as a new message.</p>
      {/if}

      {#if active && current}
        <div class="px-3.5 pb-1 pt-2.5">
          {#if current.header}<p class="text-[13px] font-semibold text-ink-200">{current.header}</p>{/if}
          <p class="text-[15px] leading-6 text-ink-50 text-pretty">{current.question}</p>
          <div class="mt-2 space-y-0.5" role={current.multiple ? "group" : "radiogroup"} aria-label={current.question}>
            {#each shownOptions as option, oi (option.label + oi)}
              {@const on = (picked[step] ?? []).includes(option.label)}
              <button type="button" role={current.multiple ? "checkbox" : "radio"} aria-checked={on} class={`qd-option ${on ? "qd-option-on" : ""}`} onclick={() => toggle(option.label)}>
                <span class={`qd-mark ${current.multiple ? "rounded-[4px]" : "rounded-full"} ${on ? "qd-mark-on" : ""}`} aria-hidden="true">
                  {#if on}{#if current.multiple}<Icon name="check" class="h-3 w-3" />{:else}<span class="h-2 w-2 rounded-full bg-current"></span>{/if}{/if}
                </span>
                <span class="min-w-0">
                  <span class={`block text-sm ${on ? "font-semibold text-ink-50" : "text-ink-200"}`}>{option.label}{#if option.recommended}<span class="qd-rec">Recommended</span>{/if}</span>
                  {#if option.description}<span class="block text-[13px] leading-5 text-ink-500">{option.description}</span>{/if}
                </span>
              </button>
            {/each}
            {#if other[step] === null || other[step] === undefined}
              <button type="button" class="qd-option text-ink-400" onclick={chooseOther}><Icon name="pencil" class="h-3.5 w-3.5 shrink-0" /><span class="text-sm">Other</span></button>
            {:else}
              <div class="px-1.5 pt-1">
                <textarea id={`q-other-${step}`} rows={2} bind:value={other[step]} placeholder="Your answer" aria-label="Your own answer" class="w-full resize-y rounded-lg border border-ink-700 bg-ink-900 px-2.5 py-2 text-sm text-ink-100 placeholder:text-ink-500"></textarea>
              </div>
            {/if}
          </div>
        </div>
        <footer class="flex items-center gap-2 px-3.5 pb-3 pt-2">
          <button type="button" class="qd-btn qd-btn-quiet" onclick={onDismiss}>Dismiss</button>
          <span class="flex-1"></span>
          {#if step > 0}<button type="button" class="qd-btn" onclick={() => (step -= 1)}>Back</button>{/if}
          {#if !last}
            <button type="button" class="qd-btn" disabled={!answered(step)} onclick={next}>Next</button>
          {:else}
            <button type="button" class="qd-btn qd-btn-primary" disabled={!canSubmit} onclick={submit} title="Submit (Ctrl+Enter)"><Icon name="check" class="h-3.5 w-3.5" />Submit</button>
          {/if}
        </footer>
      {:else}
        <div class="space-y-2 px-3.5 pb-3 pt-2">
          {#each items as q, i (i)}
            <p class="text-sm text-ink-300">{#if q.header}<span class="font-semibold text-ink-200">{q.header}: </span>{/if}{q.question}</p>
          {/each}
          {#if answer}
            <p class="flex items-start gap-2 text-sm text-ink-100"><Icon name="check" class="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-400" /><span class="whitespace-pre-wrap">{answer.replace(/^Answer to your earlier question:\n/, "")}</span></p>
          {:else if dismissed}
            <p class="text-sm text-ink-500">You dismissed these questions.</p>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .qd {
    overflow: hidden;
    border-radius: 0.75rem;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
  }
  .qd-live {
    border-color: color-mix(in oklab, var(--color-ember-500) 45%, var(--color-ink-700));
    box-shadow: 0 10px 30px -18px color-mix(in oklab, var(--color-ember-500) 60%, transparent);
  }
  .qd-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.85rem;
    border-bottom: 1px solid var(--color-ink-800);
    font-size: 0.875rem;
  }
  .qd-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--color-ink-400);
  }
  .qd-note {
    margin: 0.6rem 0.85rem 0;
    border-radius: 0.45rem;
    background: color-mix(in oklab, #f59e0b 16%, transparent);
    padding: 0.4rem 0.65rem;
    font-size: 0.8125rem;
    color: color-mix(in oklab, #fbbf24 85%, var(--color-ink-50));
  }
  .qd-option {
    display: flex;
    width: 100%;
    align-items: flex-start;
    gap: 0.6rem;
    border-radius: 0.5rem;
    padding: 0.45rem 0.4rem;
    text-align: left;
    transition: background-color 120ms ease-out;
  }
  @media (hover: hover) and (pointer: fine) {
    .qd-option:hover {
      background: color-mix(in oklab, var(--color-veil) 5%, transparent);
    }
  }
  .qd-option-on {
    background: color-mix(in oklab, var(--color-veil) 7%, transparent);
  }
  .qd-rec {
    margin-left: 0.4rem;
    border-radius: 9999px;
    background: color-mix(in oklab, var(--color-ember-500) 16%, transparent);
    padding: 0.05rem 0.45rem;
    font-size: 0.6875rem;
    font-weight: 600;
    color: var(--color-ember-200);
    white-space: nowrap;
  }
  .qd-mark {
    margin-top: 0.15rem;
    display: grid;
    height: 1rem;
    width: 1rem;
    flex-shrink: 0;
    place-items: center;
    border: 1.5px solid var(--color-ink-500);
    color: var(--color-ink-50);
    transition: border-color 120ms ease-out;
  }
  .qd-mark-on {
    border-color: var(--color-ink-50);
  }
  .qd-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.9rem;
    border-radius: 0.5rem;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.75rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--color-ink-100);
    transition: background-color 120ms ease-out, transform 120ms ease-out;
  }
  .qd-btn:active:not(:disabled) {
    transform: scale(0.97);
  }
  .qd-btn:disabled {
    opacity: 0.45;
  }
  .qd-btn-quiet {
    border-color: transparent;
    color: var(--color-ink-300);
  }
  .qd-btn-primary {
    border-color: color-mix(in oklab, var(--color-ember-500) 55%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 18%, transparent);
    color: var(--color-ember-100);
  }
  .qd-btn:hover:not(:disabled) {
    background: color-mix(in oklab, var(--color-veil) 7%, transparent);
  }
  .qd-btn-primary:hover:not(:disabled) {
    background: color-mix(in oklab, var(--color-ember-500) 26%, transparent);
  }
</style>
