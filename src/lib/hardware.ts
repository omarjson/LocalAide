// Hardware-aware fit for local models: does this model run well on this
// computer, slowly, or not at all? Pure, so the Models page and its tests
// share one answer.
//
// The heuristic, documented because a badge is a claim:
// - "too large": the model's RAM estimate is bigger than the RAM actually
//   free right now. It cannot load without pushing the system into swap.
// - "slowly": it fits in free RAM but is bigger than the model budget
//   (Settings → Memory for models, the share of RAM models may use), so
//   loading it squeezes everything else.
// - "runs well": it fits inside the model budget.
// When the budget is unknown (probe failed, 0), only free RAM decides:
// too large or runs well, never slowly on a guess.

export type ModelFit = "runs-well" | "slow" | "too-large";

/** The badge text for each fit, exhaustive by typecheck. */
export const FIT_LABELS = {
  "runs-well": "Runs well",
  slow: "Runs slowly",
  "too-large": "Too large",
} as const satisfies Record<ModelFit, string>;

export function fitBadge(ramGb: number, budgetGb: number, availableGb: number): ModelFit {
  if (availableGb > 0 && ramGb > availableGb) return "too-large";
  if (budgetGb > 0 && ramGb > budgetGb) return "slow";
  return "runs-well";
}

export interface FittableModel {
  id: string;
  kind?: string;
  ramGb: number;
  installed?: boolean;
}

export interface HardwareProbe {
  budgetGb: number;
  availableGb: number;
}

/**
 * The one "Recommended for you" model: the most capable chat model that
 * runs well (largest RAM estimate inside the budget), installed ones
 * first on a tie. When nothing runs well, the smallest model that only
 * runs slowly. Null when no chat model fits at all.
 */
export function recommendedModel<T extends FittableModel>(
  models: T[],
  probe: HardwareProbe,
): T | null {
  const chat = models.filter((m) => (m.kind ?? "chat") === "chat");
  if (chat.length === 0) return null;
  const withFit = chat.map((m) => ({ model: m, fit: fitBadge(m.ramGb, probe.budgetGb, probe.availableGb) }));
  const well = withFit.filter((w) => w.fit === "runs-well");
  const pool = well.length > 0 ? well : withFit.filter((w) => w.fit === "slow");
  if (pool.length === 0) return null;
  const sorted = [...pool].sort((a, b) => {
    if (well.length > 0) {
      // Most capable first; installed wins a tie so the pick is usable now.
      if (b.model.ramGb !== a.model.ramGb) return b.model.ramGb - a.model.ramGb;
    } else {
      // Nothing runs well: smallest slow model, least squeeze.
      if (a.model.ramGb !== b.model.ramGb) return a.model.ramGb - b.model.ramGb;
    }
    return Number(b.model.installed ?? false) - Number(a.model.installed ?? false);
  });
  return sorted[0].model;
}
