<script lang="ts">
  // Settings → About. The doctor is read-only: it runs local checks and shows
  // the report here; it never repairs or uploads anything.
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { DoctorReport } from "../lib/api/bindings";

  let report = $state<DoctorReport | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const run = async () => {
    if (busy) return;
    busy = true;
    error = null;
    try {
      report = await api.doctorRun();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const reportText = $derived(
    report
      ? [
          `Setup doctor: ${report.ok ? "all required checks passed" : "some checks failed"}`,
          ...report.checks.map((c) => `${c.ok ? "PASS" : "FAIL"}  ${c.name}: ${c.detail}`),
        ].join("\n")
      : "",
  );
</script>

<div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h3 class="flex items-center gap-2 font-semibold text-ink-50"><Icon name="stethoscope" class="h-4 w-4 text-ember-300" />Setup doctor</h3>
      <p class="mt-0.5 text-sm text-ink-400">Checks your local setup. Nothing is changed or sent anywhere.</p>
    </div>
    <Button variant="secondary" disabled={busy} onclick={() => void run()}>
      <Icon name="stethoscope" class="h-4 w-4" />{busy ? "Running…" : "Run doctor"}
    </Button>
  </div>
  {#if error}<p class="mt-3 text-sm text-rose-300" role="alert">{error}</p>{/if}
  {#if report}
    <pre class="mt-3 max-h-72 overflow-auto rounded-md border border-ink-700 bg-ink-950 p-3 text-sm leading-6 text-ink-300 whitespace-pre-wrap" aria-live="polite">{reportText}</pre>
  {/if}
</div>
