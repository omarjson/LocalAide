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

/** True when at least one capability filter is on (including saved Tools). */
export function anyFilterOn(f: PickerFilters): boolean {
  return f.reasoning || f.vision || f.tools || f.free;
}

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

/** The canonical section order: Starred, On this computer, then providers.
 * The menu uses this to order both its provider switcher and its rows. */
export function sectionOrder(choices: ModelChoices | null, query: string, filters: PickerFilters): string[] {
  return pickerSections(choices, query, filters).map((s) => s.id);
}

/** "local" for models on this computer, else the provider id. */
export function providerKey(m: ModelChoice): string {
  return m.provider_id ?? "local";
}

export interface PickerProvider {
  key: string;
  providerId: string | null;
  label: string;
  /** "Free" when every listed model of the provider is free. */
  badge: string;
  router: boolean;
  needsRefresh: boolean;
}

/** The providers the picker can switch to: this computer first, then the
 * online providers that are switched on. */
export function providerList(choices: ModelChoices | null): PickerProvider[] {
  if (!choices) return [];
  const out: PickerProvider[] = [
    { key: "local", providerId: null, label: "This computer", badge: "Private", router: false, needsRefresh: false },
  ];
  if (choices.online_allowed === false) return out;
  const seen = new Set<string>();
  for (const g of choices.groups.filter((g) => g.enabled)) {
    // The same provider saved twice must still appear once.
    if (seen.has(g.provider_id)) continue;
    seen.add(g.provider_id);
    const models = choices.models.filter((m) => m.provider_id === g.provider_id);
    out.push({
      key: g.provider_id,
      providerId: g.provider_id,
      label: g.label,
      badge: models.length > 0 && models.every((m) => m.free) ? "Free" : g.local_router ? "Local" : "",
      router: g.local_router,
      needsRefresh: g.needs_refresh,
    });
  }
  return out;
}

/** One provider's models matching the search and filters, starred first. */
export function providerModels(choices: ModelChoices | null, key: string, query: string, filters: PickerFilters = NO_FILTERS): ModelChoice[] {
  if (!choices) return [];
  return choices.models
    .filter((m) => providerKey(m) === key && matchesModel(m, query, filters))
    .sort((a, b) => Number(b.favorite) - Number(a.favorite));
}
