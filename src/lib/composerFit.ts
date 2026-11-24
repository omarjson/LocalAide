// Composer bar-fit: on a narrow window the composer's pills fold to
// glyphs in a fixed order, and unfold last-folded-first as room returns.
// Nothing ever covers Send.
//
// The order follows the reference (permission, usage, team, context,
// model, effort), mapped onto the pills LocalAide actually has:
// permission and usage are already glyphs (an icon button and a meter
// bar), team and context have no pill here, so the folding pills are the
// Tools label, the model name, and the effort label — in that order.
// A folded pill keeps its name for a screen reader (aria-label / title)
// and its tooltip whole; the CSS in main.css hides only the label text.

/** Pills that fold, in fold order. */
export const COMPOSER_FOLD_ORDER = ["tools", "model", "effort"] as const;

export type ComposerFoldLabel = (typeof COMPOSER_FOLD_ORDER)[number];

/** How many fold steps exist. */
export const MAX_COMPOSER_FOLD = COMPOSER_FOLD_ORDER.length;

/** The labels hidden at a fold level: the first `fold` of the order. */
export function foldedLabels(fold: number): ComposerFoldLabel[] {
  return COMPOSER_FOLD_ORDER.slice(0, Math.max(0, Math.min(MAX_COMPOSER_FOLD, fold)));
}

/**
 * One step of the fit loop: fold one more pill while the row overflows,
 * never past the last one. The component re-measures after each step and
 * starts again from 0 when the window grows, so pills unfold
 * last-folded-first as room returns.
 */
export function nextFoldLevel(overflows: boolean, fold: number): number {
  if (!overflows) return fold;
  return Math.min(MAX_COMPOSER_FOLD, fold + 1);
}
