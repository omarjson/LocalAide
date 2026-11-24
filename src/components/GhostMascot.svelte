<script lang="ts" module>
  export type GhostMood = "hello" | "looking" | "working" | "done";
</script>

<script lang="ts">
  // The patched ghost from the logo, without its tile, as setup's guide.
  // Moods change only the eyes. Motion moves the HTML wrapper (transform
  // and opacity), never an SVG child: SVG children animate on WebKit's
  // main thread, which is what heated the operator's Mac in 0.2.2. The
  // float runs only while `mood` is "working"; the rest play once.
  let { mood = "hello", class: className = "" }: { mood?: GhostMood; class?: string } = $props();

  // Pupil offsets: looking glances up and aside, working watches the bar.
  const gaze = $derived(mood === "looking" ? { x: 7, y: -8 } : mood === "working" ? { x: 0, y: 9 } : { x: 0, y: 0 });
</script>

<div class={`ghost ghost-${mood} ${className}`} aria-hidden="true">
  <svg viewBox="84 72 344 372" xmlns="http://www.w3.org/2000/svg">
    <!-- Body, outlined so the cream ghost reads on the light theme too. -->
    <path
      d="M256 92c-88 0-152 66-152 156v170c0 14 14 22 25 15l32-21 32 21c7 5 15 5 22 0l27-20 27 20c7 5 15 5 22 0l32-21 32 21c11 7 25-1 25-15V248c0-90-64-156-152-156z"
      fill="#f3ead8"
      stroke="#1f1630"
      stroke-width="9"
      stroke-linejoin="round"
    />
    <!-- The stitched seam. -->
    <path d="M176 316q80-26 160 0" fill="none" stroke="#1f1630" stroke-width="6" stroke-linecap="round" stroke-dasharray="10 12" />
    <!-- The ember patch. -->
    <rect x="274" y="352" width="72" height="56" rx="12" fill="#ff7a1a" />
    <rect x="274" y="352" width="72" height="56" rx="12" fill="none" stroke="#1f1630" stroke-width="5" stroke-dasharray="8 8" />
    {#if mood === "done"}
      <!-- Happy eyes. -->
      <path d="M192 242q20-26 40 0" fill="none" stroke="#1f1630" stroke-width="10" stroke-linecap="round" />
      <path d="M280 242q20-26 40 0" fill="none" stroke="#ff7a1a" stroke-width="10" stroke-linecap="round" />
      <!-- Blush. -->
      <ellipse cx="178" cy="278" rx="16" ry="9" fill="#ff7a1a" opacity="0.28" />
      <ellipse cx="334" cy="278" rx="16" ry="9" fill="#ff7a1a" opacity="0.28" />
    {:else}
      <circle cx={212 + gaze.x} cy={236 + gaze.y} r="20" fill="#1f1630" />
      <circle cx={300 + gaze.x} cy={236 + gaze.y} r="27" fill="#ff7a1a" opacity="0.22" />
      <circle cx={300 + gaze.x} cy={236 + gaze.y} r="20" fill="#1f1630" />
      <circle cx={300 + gaze.x} cy={236 + gaze.y} r="11" fill="#ff7a1a" />
      <!-- Catch-lights. -->
      <circle cx={206 + gaze.x} cy={229 + gaze.y} r="5" fill="#f3ead8" />
    {/if}
  </svg>
</div>

<style>
  .ghost {
    display: inline-block;
    will-change: transform;
  }
  .ghost svg {
    display: block;
    height: 100%;
    width: 100%;
    filter: drop-shadow(0 10px 14px rgb(31 22 48 / 0.28));
  }
  .ghost-hello {
    animation: ghost-hello 900ms cubic-bezier(0.23, 1, 0.32, 1) 1;
  }
  .ghost-working {
    animation: ghost-float 2.4s ease-in-out infinite;
  }
  .ghost-done {
    animation: ghost-pop 700ms cubic-bezier(0.34, 1.56, 0.64, 1) 1;
  }
  @keyframes ghost-hello {
    0% { transform: translateY(10px) rotate(-6deg); opacity: 0; }
    60% { transform: translateY(-3px) rotate(3deg); opacity: 1; }
    100% { transform: none; }
  }
  @keyframes ghost-float {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-7px); }
  }
  @keyframes ghost-pop {
    0% { transform: scale(0.8); }
    55% { transform: scale(1.08); }
    100% { transform: scale(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .ghost {
      animation: none !important;
    }
  }
</style>
