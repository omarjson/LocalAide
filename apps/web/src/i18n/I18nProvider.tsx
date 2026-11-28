import type { ReactNode } from "react";

/**
 * The translation host.
 *
 * Vendored from GOAT (`apps/web/src/i18n/I18nProvider.tsx`, MIT). GOAT loads a
 * locale bundle at runtime; LocalAide ships one language, so the provider
 * only has to exist.
 */
export function useFormattingLocale(): string | undefined {
  return undefined;
}

export function I18nProvider({ children }: { readonly children: ReactNode }) {
  return <>{children}</>;
}
