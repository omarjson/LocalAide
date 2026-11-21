<script lang="ts">
  // One icon set: 24px grid, 1.75 stroke, rounded caps and joins, drawn inline so it
  // takes the surrounding text color and needs no icon font or extra package.
  export type IconName =
    | "chat"
    | "search"
    | "memory"
    | "skills"
    | "models"
    | "settings"
    | "usage"
    | "pin"
    | "close"
    | "menu"
    | "alert"
    | "lock"
    | "shield"
    | "check"
    | "arrow-down"
    | "plus"
    | "file"
    | "file-plus"
    | "folder"
    | "folder-plus"
    | "more"
    | "sliders"
    | "refresh"
    | "download"
    | "trash"
    | "info"
    | "sun"
    | "computer"
    | "cloud"
    | "database"
    | "globe"
    | "chevron-right"
    | "chevron-down"
    | "flame"
    | "heart"
    | "clock"
    | "external"
    | "scale"
    | "key"
    | "sparkle"
    | "sidebar"
    | "arrow-up"
    | "stop"
    | "copy"
    | "pencil"
    | "image"
    | "bolt"
    | "unlock"
    | "code"
    | "flask"
    | "plug"
    | "eye"
    | "layers"
    | "target"
    | "brain"
    | "wrench"
    | "paperclip"
    | "star"
    | "star-filled"
    | "x-circle"
    | "window"
    | "terminal"
    | "volume"
    | "mic"
    | "link"
    | "lightbulb"
    | "hand"
    | "shield-check"
    | "users"
    | "list-check"
    | "undo"
    | "help"
    | "chart"
    | "folder-open"
    | "wand";

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
  {:else if name === "skills"}
    <path d="m12 3 1.9 4.6L18.5 9.5l-4.6 1.9L12 16l-1.9-4.6L5.5 9.5l4.6-1.9z" />
    <path d="M18 15.5 19 18l2.5 1L19 20l-1 2.5L17 20l-2.5-1 2.5-1z" />
  {:else if name === "models"}
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4" />
  {:else if name === "usage"}
    <path d="M3 13h3l2.5-8 5 15 2.5-8H21" />
  {:else if name === "pin"}
    <path d="M12 16v6" />
    <path d="M8.5 3h7l-1 6 3 3.5a1 1 0 0 1-.8 1.5H7.3a1 1 0 0 1-.8-1.5l3-3.5z" />
  {:else if name === "settings"}
    <path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1" />
    <circle cx="15" cy="6" r="2" />
    <circle cx="9" cy="12" r="2" />
    <circle cx="17" cy="18" r="2" />
  {:else if name === "close"}
    <path d="M6 6l12 12M18 6L6 18" />
  {:else if name === "menu"}
    <path d="M4 7h16M4 12h16M4 17h16" />
  {:else if name === "alert"}
    <path d="M12 4 2.5 20h19L12 4z" />
    <path d="M12 10v4" />
    <circle cx="12" cy="17" r="0.5" />
  {:else if name === "lock"}
    <rect x="5" y="11" width="14" height="9" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  {:else if name === "shield"}
    <path d="M12 3c3 2 6 2.5 8 2.5V12c0 5-3.5 8-8 9-4.5-1-8-4-8-9V5.5C6 5.5 9 5 12 3z" />
  {:else if name === "check"}
    <path d="m5 13 4 4 10-10" />
  {:else if name === "arrow-down"}
    <path d="M12 5v14m-6-6 6 6 6-6" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "file"}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h4M9 13h6M9 17h6" />
  {:else if name === "file-plus"}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h4M12 12v6M9 15h6" />
  {:else if name === "folder"}
    <path d="M3 7h7l2 2h9v10H3z" />
  {:else if name === "folder-plus"}
    <path d="M3 7h7l2 2h9v10H3z" />
    <path d="M12 12v5M9.5 14.5h5" />
  {:else if name === "more"}
    <circle cx="5" cy="12" r="1" />
    <circle cx="12" cy="12" r="1" />
    <circle cx="19" cy="12" r="1" />
  {:else if name === "sliders"}
    <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
    <circle cx="16" cy="7" r="2" />
    <circle cx="10" cy="17" r="2" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 0 0-14.6-4L3 10" />
    <path d="M3 5v5h5M4 13a8 8 0 0 0 14.6 4L21 14" />
    <path d="M21 19v-5h-5" />
  {:else if name === "download"}
    <path d="M12 3v12m-5-5 5 5 5-5M5 21h14" />
  {:else if name === "trash"}
    <path d="M4 7h16M10 11v6M14 11v6M6 7l1 14h10l1-14M9 7V4h6v3" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  {:else if name === "sun"}
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  {:else if name === "computer"}
    <rect x="3" y="4" width="18" height="12" rx="2" />
    <path d="M8 20h8M12 16v4" />
  {:else if name === "cloud"}
    <path d="M7 18a4 4 0 0 1 0-8 5 5 0 0 1 9.6-1.5A3.5 3.5 0 1 1 17.5 18z" />
  {:else if name === "database"}
    <ellipse cx="12" cy="5" rx="7" ry="3" />
    <path d="M5 5v7c0 1.7 3.1 3 7 3s7-1.3 7-3V5M5 12v7c0 1.7 3.1 3 7 3s7-1.3 7-3v-7" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
  {:else if name === "chevron-right"}
    <path d="m9 6 6 6-6 6" />
  {:else if name === "chevron-down"}
    <path d="m6 9 6 6 6-6" />
  {:else if name === "flame"}
    <path d="M12 21c-3.9 0-7-2.7-7-6.5 0-3 2-5 3.5-6.5.3 1.6 1.2 2.8 2.5 3.3C11 8 12.5 5 15 3c0 3 4 5.5 4 10.5 0 4-3.1 7.5-7 7.5z" />
    <path d="M12 21c-1.7 0-3-1.3-3-3 0-1.5 1-2.5 2-3.5.3 1 1 1.6 2 1.8.6-.9 1-2 1-3.3 1.2 1 2 2.6 2 4.5 0 2-1.6 3.5-4 3.5z" />
  {:else if name === "heart"}
    <path d="M12 20s-7.5-4.5-7.5-10A4.5 4.5 0 0 1 12 7a4.5 4.5 0 0 1 7.5 3c0 5.5-7.5 10-7.5 10z" />
  {:else if name === "clock"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v5l3 2" />
  {:else if name === "external"}
    <path d="M14 4h6v6M20 4l-9 9" />
    <path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5" />
  {:else if name === "scale"}
    <path d="M12 4v16M7 20h10M5 7h14M12 4l-1 3h2z" />
    <path d="M5 7l-3 6a3 3 0 0 0 6 0zM19 7l-3 6a3 3 0 0 0 6 0z" />
  {:else if name === "key"}
    <circle cx="8" cy="15" r="4" />
    <path d="m11 12 9-9M16 7l3 3M14 9l2 2" />
  {:else if name === "sidebar"}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M9 4v16M15 10l-2 2 2 2" />
  {:else if name === "arrow-up"}
    <path d="M12 19V5m-6 6 6-6 6 6" />
  {:else if name === "stop"}
    <rect x="7" y="7" width="10" height="10" rx="1.5" fill="currentColor" stroke="none" />
  {:else if name === "copy"}
    <rect x="9" y="9" width="11" height="11" rx="2" />
    <path d="M5 15V6a2 2 0 0 1 2-2h8" />
  {:else if name === "pencil"}
    <path d="M4 20h4L19 9l-4-4L4 16z" />
    <path d="m13.5 6.5 4 4" />
  {:else if name === "image"}
    <rect x="3" y="5" width="18" height="14" rx="2" />
    <circle cx="9" cy="10" r="1.5" />
    <path d="m21 16-5-5-8 8" />
  {:else if name === "bolt"}
    <path d="M13 3 5 13h6l-1 8 8-10h-6z" />
  {:else if name === "unlock"}
    <rect x="5" y="11" width="14" height="9" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 7.5-2" />
  {:else if name === "code"}
    <path d="m8 8-4 4 4 4M16 8l4 4-4 4M14 5l-4 14" />
  {:else if name === "flask"}
    <path d="M9 3h6M10 3v6L4.5 18.5A1.7 1.7 0 0 0 6 21h12a1.7 1.7 0 0 0 1.5-2.5L14 9V3" />
    <path d="M7 15h10" />
  {:else if name === "plug"}
    <path d="M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0zM12 17v4" />
  {:else if name === "eye"}
    <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" />
    <circle cx="12" cy="12" r="3" />
  {:else if name === "layers"}
    <path d="m12 3 9 5-9 5-9-5z" />
    <path d="m3 13 9 5 9-5" />
  {:else if name === "target"}
    <circle cx="12" cy="12" r="9" />
    <circle cx="12" cy="12" r="5" />
    <circle cx="12" cy="12" r="1" />
  {:else if name === "sparkle"}
    <path d="M12 3v4M12 17v4M3 12h4M17 12h4M6.3 6.3l2.5 2.5M15.2 15.2l2.5 2.5M6.3 17.7l2.5-2.5M15.2 8.8l2.5-2.5" />
  {:else if name === "brain"}
    <path d="M9.5 4A2.5 2.5 0 0 0 7 6.5v.1A3 3 0 0 0 4.5 10a3 3 0 0 0 .9 2.1A3 3 0 0 0 6 17.5 2.5 2.5 0 0 0 9.5 20 2.5 2.5 0 0 0 12 17.5v-11A2.5 2.5 0 0 0 9.5 4z" />
    <path d="M14.5 4A2.5 2.5 0 0 1 17 6.5v.1a3 3 0 0 1 2.5 3.4 3 3 0 0 1-.9 2.1 3 3 0 0 1-.6 5.4A2.5 2.5 0 0 1 14.5 20 2.5 2.5 0 0 1 12 17.5" />
  {:else if name === "wrench"}
    <path d="M14.7 6.3a4 4 0 0 0-5.4 5.1L4 16.7V20h3.3l5.3-5.3a4 4 0 0 0 5.1-5.4l-2.5 2.5-2.5-2.5z" />
  {:else if name === "paperclip"}
    <path d="m20 11.5-8.2 8.2a5 5 0 0 1-7.1-7.1l8.5-8.5a3.3 3.3 0 0 1 4.7 4.7l-8.5 8.5a1.7 1.7 0 0 1-2.4-2.4l7.8-7.8" />
  {:else if name === "star"}
    <path d="m12 3.5 2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" />
  {:else if name === "star-filled"}
    <path d="m12 3.5 2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z" fill="currentColor" />
  {:else if name === "x-circle"}
    <circle cx="12" cy="12" r="9" />
    <path d="m9 9 6 6M15 9l-6 6" />
  {:else if name === "window"}
    <rect x="3" y="5" width="18" height="14" rx="2" />
    <path d="M3 9h18" />
  {:else if name === "terminal"}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="m7 9 3 3-3 3M12 15h5" />
  {:else if name === "mic"}
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
  {:else if name === "link"}
    <path d="M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1" />
    <path d="M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1" />
  {:else if name === "lightbulb"}
    <path d="M9 18h6M10 21h4M12 3a6 6 0 0 0-3.6 10.8c.6.5 1 1.2 1.1 2l.1.7h4.8l.1-.7c.1-.8.5-1.5 1.1-2A6 6 0 0 0 12 3Z" />
  {:else if name === "hand"}
    <path d="M18 11V6a2 2 0 0 0-4 0M14 10V4a2 2 0 0 0-4 0v2M10 10.5V6a2 2 0 0 0-4 0v8" />
    <path d="M18 8a2 2 0 0 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.9-6-2.4L2.4 16a2 2 0 0 1 2.8-2.8L7 15" />
  {:else if name === "shield-check"}
    <path d="M12 3 5 6v5c0 4.4 3 8.4 7 10 4-1.6 7-5.6 7-10V6l-7-3Z" />
    <path d="m9 12 2 2 4-4" />
  {:else if name === "users"}
    <circle cx="9" cy="8" r="3.5" />
    <path d="M2.5 20a6.5 6.5 0 0 1 13 0M16 4.6a3.5 3.5 0 0 1 0 6.8M18 14.5a6.5 6.5 0 0 1 3.5 5.5" />
  {:else if name === "list-check"}
    <path d="m3 6 1.5 1.5L7 5M3 12l1.5 1.5L7 11M3 18l1.5 1.5L7 17M11 6h10M11 12h10M11 18h10" />
  {:else if name === "undo"}
    <path d="M9 14 4 9l5-5" />
    <path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11" />
  {:else if name === "help"}
    <circle cx="12" cy="12" r="9" />
    <path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01" />
  {:else if name === "chart"}
    <path d="M4 20V4M4 20h16M8 16v-4M12 16V8M16 16v-6" />
  {:else if name === "folder-open"}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v1" />
    <path d="M3 7v11a2 2 0 0 0 2 2h12.5a2 2 0 0 0 1.9-1.4l1.9-6A1.5 1.5 0 0 0 19.9 11H7.4a2 2 0 0 0-1.9 1.4L3 20" />
  {:else if name === "wand"}
    <path d="m15 4 1 2 2 1-2 1-1 2-1-2-2-1 2-1 1-2ZM19 13l.6 1.4L21 15l-1.4.6L19 17l-.6-1.4L17 15l1.4-.6L19 13Z" />
    <path d="M4 20 14 10" />
  {:else if name === "volume"}
    <path d="M11 5 6 9H3v6h3l5 4V5Z" />
    <path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9 9 0 0 1 0 13" />
  {/if}
</svg>
