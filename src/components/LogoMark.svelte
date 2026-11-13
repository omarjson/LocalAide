<script lang="ts">
  // The stitched-ghost mark, inlined so the ember eye can glow. Same art as
  // public/logo.svg; that file stays the favicon and any <img> use.
  let { class: className = "" }: { class?: string } = $props();
</script>

<svg
  viewBox="0 0 512 512"
  class={className}
  role="img"
  aria-label="LocalAide logo"
  xmlns="http://www.w3.org/2000/svg"
>
  <path
    d="M256 92c-88 0-152 66-152 156v170c0 14 14 22 25 15l32-21 32 21c7 5 15 5 22 0l27-20 27 20c7 5 15 5 22 0l32-21 32 21c11 7 25-1 25-15V248c0-90-64-156-152-156z"
    fill="#f3ead8"
  />
  <path
    d="M176 316q80-26 160 0"
    fill="none"
    stroke="#1f1630"
    stroke-width="6"
    stroke-linecap="round"
    stroke-dasharray="10 12"
  />
  <rect x="274" y="352" width="72" height="56" rx="12" fill="#ff7a1a" />
  <rect
    x="274"
    y="352"
    width="72"
    height="56"
    rx="12"
    fill="none"
    stroke="#1f1630"
    stroke-width="5"
    stroke-dasharray="8 8"
  />
  <circle cx="212" cy="236" r="20" fill="#1f1630" />
  <circle cx="300" cy="236" r="27" fill="#ff7a1a" opacity="0.22" class="logo-glow" />
  <circle cx="300" cy="236" r="20" fill="#1f1630" />
  <circle cx="300" cy="236" r="11" fill="#ff7a1a" />
</svg>

<style>
  .logo-glow {
    animation: logo-pulse 2.6s ease-in-out infinite;
    transform-origin: 300px 236px;
  }
  @keyframes logo-pulse {
    0%,
    100% {
      opacity: 0.18;
      transform: scale(1);
    }
    50% {
      opacity: 0.4;
      transform: scale(1.12);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .logo-glow {
      animation: none;
    }
  }
</style>
