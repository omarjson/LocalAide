import { RouterProvider } from "@tanstack/react-router";
import { TooltipProvider } from "~/components/ui/tooltip";
import { I18nProvider } from "~/i18n/I18nProvider";
import { useThemeClass } from "~/hooks/useSettings";
import { router } from "~/routes";

/**
 * The providers around the app.
 *
 * Vendored from GOAT (`apps/web/src/AppRoot.tsx`, MIT). GOAT's root also held
 * the server's auth gate, the command palette and the tracing setup; LocalAide
 * talks to a local backend that needs no sign-in.
 */
export function AppRoot() {
  useThemeClass();

  return (
    <I18nProvider>
      <TooltipProvider>
        <RouterProvider router={router} />
      </TooltipProvider>
    </I18nProvider>
  );
}
