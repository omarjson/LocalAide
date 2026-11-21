<script lang="ts">
  // A chart the model described as JSON in a ```chart block: bar, line or
  // pie. Drawn here as SVG from plain numbers; nothing from the model runs.
  import { parseChart, PALETTE, type ChartSpec } from "../lib/charts";

  let { source }: { source: string } = $props();

  const chart = $derived<ChartSpec | null>(parseChart(source));
  const W = 560;
  const H = 260;
  const PAD = { l: 44, r: 12, t: 12, b: 34 };

  const max = $derived(chart ? Math.max(1e-9, ...chart.series.flatMap((s) => s.values)) : 1);
  const min = $derived(chart ? Math.min(0, ...chart.series.flatMap((s) => s.values)) : 0);
  const y = (v: number) => PAD.t + (H - PAD.t - PAD.b) * (1 - (v - min) / (max - min || 1));
  const ticks = $derived(Array.from({ length: 5 }, (_, i) => min + ((max - min) * i) / 4));
  const fmt = (v: number) => (Math.abs(v) >= 1000 ? `${+(v / 1000).toFixed(1)}k` : `${+v.toFixed(2)}`);

  const pieSlices = $derived.by(() => {
    if (!chart || chart.type !== "pie") return [];
    const values = chart.series[0]?.values ?? [];
    const total = values.reduce((a, b) => a + Math.max(0, b), 0) || 1;
    let angle = -Math.PI / 2;
    return values.map((v, i) => {
      const share = Math.max(0, v) / total;
      const a0 = angle;
      const a1 = angle + share * Math.PI * 2;
      angle = a1;
      const r = 100;
      const cx = 130;
      const cy = 130;
      const large = a1 - a0 > Math.PI ? 1 : 0;
      const d =
        share >= 0.9999
          ? `M ${cx} ${cy - r} A ${r} ${r} 0 1 1 ${cx - 0.01} ${cy - r} Z`
          : `M ${cx} ${cy} L ${cx + r * Math.cos(a0)} ${cy + r * Math.sin(a0)} A ${r} ${r} 0 ${large} 1 ${cx + r * Math.cos(a1)} ${cy + r * Math.sin(a1)} Z`;
      return { d, color: PALETTE[i % PALETTE.length], label: chart.labels[i] ?? `${i + 1}`, share };
    });
  });
</script>

{#if chart}
  <figure class="chart">
    {#if chart.title}<figcaption class="mb-2 text-sm font-semibold text-ink-100">{chart.title}</figcaption>{/if}
    {#if chart.type === "pie"}
      <div class="flex flex-wrap items-center gap-5">
        <svg viewBox="0 0 260 260" class="h-52 w-52 shrink-0" role="img" aria-label={chart.title || "Pie chart"}>
          {#each pieSlices as slice, i (i)}<path d={slice.d} fill={slice.color} stroke="var(--color-ink-950)" stroke-width="2"><title>{slice.label}: {(slice.share * 100).toFixed(1)}%</title></path>{/each}
        </svg>
        <ul class="space-y-1 text-[13px]">
          {#each pieSlices as slice, i (i)}
            <li class="flex items-center gap-2 text-ink-300"><span class="h-2.5 w-2.5 rounded-sm" style={`background:${slice.color}`}></span>{slice.label}<span class="tabular-nums text-ink-500">{(slice.share * 100).toFixed(1)}%</span></li>
          {/each}
        </ul>
      </div>
    {:else}
      <svg viewBox={`0 0 ${W} ${H}`} class="block max-h-64 w-full" role="img" aria-label={chart.title || `${chart.type} chart`}>
        {#each ticks as t, i (i)}
          <line x1={PAD.l} x2={W - PAD.r} y1={y(t)} y2={y(t)} stroke="var(--color-ink-800)" stroke-width="1" />
          <text x={PAD.l - 6} y={y(t) + 4} text-anchor="end" class="axis">{fmt(t)}</text>
        {/each}
        {#if chart.type === "bar"}
          {@const n = chart.labels.length || 1}
          {@const band = (W - PAD.l - PAD.r) / n}
          {@const groups = chart.series.length}
          {@const bw = Math.min(48, Math.max(2, (band * 0.6) / groups))}
          {@const offset = (band - bw * groups) / 2}
          {#each chart.labels as label, li (li)}
            {#each chart.series as s, si (si)}
              {@const v = s.values[li] ?? 0}
              <rect x={PAD.l + band * li + offset + bw * si} y={Math.min(y(v), y(0))} width={bw - 1} height={Math.abs(y(0) - y(v))} rx="2" fill={PALETTE[si % PALETTE.length]}><title>{s.name ? `${s.name}, ` : ""}{label}: {v}</title></rect>
            {/each}
            <text x={PAD.l + band * li + band / 2} y={H - PAD.b + 16} text-anchor="middle" class="axis">{label.length > 12 ? `${label.slice(0, 11)}…` : label}</text>
          {/each}
        {:else}
          {@const n = Math.max(1, chart.labels.length - 1)}
          {@const x = (i: number) => PAD.l + ((W - PAD.l - PAD.r) * i) / n}
          {#each chart.series as s, si (si)}
            <polyline fill="none" stroke={PALETTE[si % PALETTE.length]} stroke-width="2.25" stroke-linejoin="round" points={s.values.map((v, i) => `${x(i)},${y(v)}`).join(" ")} />
            {#each s.values as v, i (i)}<circle cx={x(i)} cy={y(v)} r="3" fill={PALETTE[si % PALETTE.length]}><title>{s.name ? `${s.name}, ` : ""}{chart.labels[i] ?? i + 1}: {v}</title></circle>{/each}
          {/each}
          {#each chart.labels as label, i (i)}
            <text x={x(i)} y={H - PAD.b + 16} text-anchor="middle" class="axis">{label.length > 10 ? `${label.slice(0, 9)}…` : label}</text>
          {/each}
        {/if}
      </svg>
      {#if chart.series.length > 1 || chart.series[0]?.name}
        <div class="mt-1 flex flex-wrap gap-3 text-[13px] text-ink-300">
          {#each chart.series as s, si (si)}<span class="flex items-center gap-1.5"><span class="h-2.5 w-2.5 rounded-sm" style={`background:${PALETTE[si % PALETTE.length]}`}></span>{s.name || `Series ${si + 1}`}</span>{/each}
        </div>
      {/if}
    {/if}
  </figure>
{:else}
  <pre class="rounded-xl border border-ink-700 bg-ink-950 p-3 font-mono text-[13px] text-ink-300">{source}</pre>
{/if}

<style>
  .chart {
    border-radius: 0.75rem;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
    padding: 0.85rem 1rem;
  }
  .axis {
    fill: var(--color-ink-500);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
</style>
