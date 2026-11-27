/** A chart from a ```chart block, cleaned to plain labels and numbers. */
export interface ChartSpec {
  type: "bar" | "line" | "pie";
  title: string;
  labels: string[];
  series: { name: string; values: number[] }[];
}

/** Distinct on both themes; the first is the app's blue accent. */
export const PALETTE = ["#339cff", "#38bdf8", "#34d399", "#a78bfa", "#f472b6", "#facc15", "#94a3b8"];

const num = (v: unknown): number => {
  const n = typeof v === "number" ? v : typeof v === "string" ? Number(v.replace(/[,%\s]/g, "")) : NaN;
  return Number.isFinite(n) ? n : 0;
};

/** Read the model's JSON. Null when it is not a chart. Accepts a few
 * common shapes: {labels, series:[{name, values}]}, {labels, values} and
 * {data: [{label, value}]}. */
export function parseChart(source: string): ChartSpec | null {
  let raw: unknown;
  try {
    raw = JSON.parse(source);
  } catch {
    return null;
  }
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const type = o.type === "line" || o.type === "pie" ? o.type : "bar";
  const title = typeof o.title === "string" ? o.title.slice(0, 120) : "";
  let labels: string[] = Array.isArray(o.labels) ? o.labels.map((l) => String(l).slice(0, 60)) : [];
  let series: ChartSpec["series"] = [];
  if (Array.isArray(o.series)) {
    series = o.series
      .filter((s): s is Record<string, unknown> => !!s && typeof s === "object")
      .map((s, i) => ({
        name: typeof s.name === "string" ? s.name.slice(0, 60) : `Series ${i + 1}`,
        values: Array.isArray(s.values) ? s.values.map(num) : Array.isArray(s.data) ? s.data.map(num) : [],
      }));
  } else if (Array.isArray(o.values)) {
    series = [{ name: "", values: o.values.map(num) }];
  } else if (Array.isArray(o.data)) {
    const rows = o.data.filter((d): d is Record<string, unknown> => !!d && typeof d === "object");
    labels = rows.map((d) => String(d.label ?? d.name ?? d.x ?? "").slice(0, 60));
    series = [{ name: "", values: rows.map((d) => num(d.value ?? d.y)) }];
  }
  series = series.filter((s) => s.values.length > 0).slice(0, 8);
  if (series.length === 0) return null;
  const longest = Math.max(...series.map((s) => s.values.length));
  if (labels.length < longest) labels = [...labels, ...Array.from({ length: longest - labels.length }, (_, i) => `${labels.length + i + 1}`)];
  labels = labels.slice(0, 60);
  series = series.map((s) => ({ ...s, values: s.values.slice(0, labels.length) }));
  return { type, title, labels, series };
}
