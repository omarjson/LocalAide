<script lang="ts">
  // One icon set: 24px grid, 1.75 stroke, rounded caps and joins, drawn inline so it
  // takes the surrounding text color and needs no icon font or extra package.
  type IconName = "chat" | "search" | "memory" | "models" | "settings" | "dev" | "close";

  interface Props {
    name: IconName;
    class?: string;
  }

  let { name, class: cls = "w-4 h-4" }: Props = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={cls}
  aria-hidden="true"
  focusable="false"
>
  {#if name === "chat"}
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.5-3.5" />
  {:else if name === "memory"}
    <path d="M6 3h11a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6z" />
    <path d="M9 7h6M9 11h6M9 15h4" />
  {:else if name === "models"}
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4" />
  {:else if name === "settings"}
    <path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1" />
    <circle cx="15" cy="6" r="2" />
    <circle cx="9" cy="12" r="2" />
    <circle cx="17" cy="18" r="2" />
  {:else if name === "dev"}
    <path d="m8 8-4 4 4 4M16 8l4 4-4 4M13.5 5l-3 14" />
  {:else if name === "close"}
    <path d="M6 6l12 12M18 6L6 18" />
  {/if}
</svg>
