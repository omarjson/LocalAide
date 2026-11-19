// The legal documents ship inside the app bundle (Vite inlines them at build
// time), so they read offline and always match the installed version.
import privacy from "../../PRIVACY.md?raw";
import terms from "../../TERMS.md?raw";
import notices from "../../THIRD_PARTY_NOTICES.md?raw";

export type LegalDoc = "privacy" | "terms" | "notices";

export const LEGAL_DOCS: Record<LegalDoc, { title: string; source: string }> = {
  privacy: { title: "Privacy Policy", source: privacy },
  terms: { title: "Terms of Use", source: terms },
  notices: { title: "Open-source notices", source: notices },
};

// The open request lives here so any screen (Settings, the setup wizard, the
// command palette) can open the same dialog that App.svelte renders.
export const legal = $state<{ open: LegalDoc | null }>({ open: null });

export function openLegal(doc: LegalDoc): void {
  legal.open = doc;
}
