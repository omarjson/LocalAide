import { api } from "@goat/client-runtime/localaide";
import { useEffect, useState } from "react";

/**
 * The settings page.
 *
 * The settings panels land with the rest of the rewiring; this page is the
 * place they live.
 */
export function SettingsPage() {
  const [setupComplete, setSetupComplete] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api
      .loadConfig()
      .then((config) => {
        if (!cancelled) setSetupComplete(config.setup_complete === true);
      })
      .catch(() => {
        if (!cancelled) setSetupComplete(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
      <div className="mx-auto w-full max-w-2xl">
        <h1 className="text-xl font-semibold tracking-tight">Settings</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Choose how LocalAide behaves on this computer.
        </p>
        <div className="mt-6 rounded-lg border border-border p-4 text-sm text-muted-foreground">
          {setupComplete === null
            ? "Reading your settings…"
            : setupComplete
              ? "Your settings are saved."
              : "Finish the setup to choose a model."}
        </div>
      </div>
    </div>
  );
}
