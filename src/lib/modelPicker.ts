// What the chat's model menu shows: search, capability filters, and the
// sections (Starred, On this computer, then one per provider). Kept out of
// the component so it can be tested.
import type { ModelChoice, ModelChoices } from "./api/bindings";

export interface PickerFilters {
  reasoning: boolean;
  vision: boolean;
  tools: boolean;
  free: boolean;
}

export const NO_FILTERS: PickerFilters = { reasoning: false, vision: false, tools: false, free: false };

export interface PickerSection {
  id: string;
  label: string;
  providerId: string | null;
  needsRefresh: boolean;
  /** Starred models from this provider, shown under Starred instead. */
  starred?: number;
  items: ModelChoice[];
}

/** Read saved filters; anything unexpected counts as off. */
export function parseFilters(raw: string | null): PickerFilters {
  try {
    const value = JSON.parse(raw ?? "{}") as Record<string, unknown>;
    return {
      reasoning: value.reasoning === true,
      vision: value.vision === true,
      tools: value.tools === true,
      free: value.free === true,
    };
  } catch {
    return { ...NO_FILTERS };
  }
}

export function matchesModel(m: ModelChoice, query: string, filters: PickerFilters): boolean {
  const q = query.trim().toLowerCase();
  if (q && !`${m.name} ${m.model} ${m.provider_label}`.toLowerCase().includes(q)) return false;
  if (filters.reasoning && !m.reasoning) return false;
  if (filters.vision && !m.vision) return false;
  if (filters.tools && !m.tools) return false;
  if (filters.free && !m.free) return false;
  return true;
}

export function pickerSections(choices: ModelChoices | null, query: string, filters: PickerFilters): PickerSection[] {
  if (!choices) return [];
  const list = choices.models.filter((m) => matchesModel(m, query, filters));
  const out: PickerSection[] = [];
  const starred = list.filter((m) => m.favorite);
  if (starred.length) out.push({ id: "starred", label: "Starred", providerId: null, needsRefresh: false, items: starred });
  // A starred model shows once, under Starred, not again in its group.
  const local = list.filter((m) => m.provider_id === null && !m.favorite);
  if (local.length) out.push({ id: "local", label: "On this computer", providerId: null, needsRefresh: false, items: local });
  // With online providers switched off, no provider section appears.
  if (choices.online_allowed === false) return out;
  for (const g of choices.groups.filter((g) => g.enabled)) {
    const items = list.filter((m) => m.provider_id === g.provider_id && !m.favorite);
    if (items.length || !query.trim()) {
      const starredHere = list.filter((m) => m.provider_id === g.provider_id && m.favorite).length;
      out.push({ id: g.provider_id, label: g.label, providerId: g.provider_id, needsRefresh: g.needs_refresh, starred: starredHere, items });
    }
  }
  return out;
}
