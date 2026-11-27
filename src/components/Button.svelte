<script lang="ts">
  import type { Snippet } from "svelte";

  type ButtonVariant = "primary" | "secondary" | "subtle" | "danger";

  let {
    variant = "secondary",
    type = "button",
    disabled = false,
    onclick,
    title,
    ariaLabel,
    ariaExpanded,
    ariaHaspopup,
    class: className = "",
    children,
  }: {
    variant?: ButtonVariant;
    type?: "button" | "submit" | "reset";
    disabled?: boolean;
    onclick?: (event: MouseEvent) => void;
    title?: string;
    ariaLabel?: string;
    /** Set on a button that opens a menu, so the menu can find it again. */
    ariaExpanded?: boolean;
    ariaHaspopup?: "menu" | "listbox" | "dialog";
    class?: string;
    children?: Snippet;
  } = $props();

  const variants: Record<ButtonVariant, string> = {
    primary: "btn-ink font-semibold",
    secondary: "bg-ink-900 border border-ink-700 text-ink-100 hover:bg-ink-800",
    subtle: "text-ink-300 hover:bg-veil/5 hover:text-ink-50",
    danger: "text-rose-400 hover:bg-rose-500/10",
  };
</script>

<button
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  aria-expanded={ariaExpanded}
  aria-haspopup={ariaHaspopup}
  onclick={onclick}
  class={`inline-flex h-9 items-center justify-center gap-2 rounded-md px-3 text-sm whitespace-nowrap transition-[background-color,color,border-color,box-shadow] duration-150 ease-out disabled:cursor-default disabled:opacity-50 ${variants[variant]} ${className}`}
>
  {@render children?.()}
</button>
