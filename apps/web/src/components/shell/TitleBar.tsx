import { useEffect, useState } from "react";
import { getCurrentWindow, platform } from "@goat/client-runtime/localaide";
import { MaximizeIcon, MinusIcon, XIcon } from "~/components/solar";
import { isElectron } from "~/env";
import { cn } from "~/lib/utils";

/**
 * The window chrome.
 *
 * The Electron window has no native frame, so the app draws its own bar: a
 * drag area across the top and the window buttons on the right. macOS keeps
 * its traffic lights, so the bar leaves room for them.
 */
export function TitleBar() {
  const [maximized, setMaximized] = useState(false);
  const isMac = platform() === "darwin";

  useEffect(() => {
    if (!isElectron) return;
    const window = getCurrentWindow();
    void window
      .isMaximized()
      .then((value) => setMaximized(value === true))
      .catch(() => undefined);
    return window.onMaximized((value) => setMaximized(value));
  }, []);

  if (!isElectron) return null;

  const act = (what: string, call: Promise<unknown>) => {
    call.catch((error: unknown) => console.warn(`Window ${what} failed`, error));
  };

  return (
    <header
      className={cn(
        "drag-region flex h-(--workspace-topbar-height) shrink-0 select-none items-center gap-1 border-b border-border bg-background px-2",
        isMac && "pl-20",
      )}
      aria-label="Window"
      onDoubleClick={(event) => {
        const target = event.target as HTMLElement | null;
        if (target?.closest("button, input, a")) return;
        act("maximize", getCurrentWindow().toggleMaximize());
      }}
    >
      <span className="min-w-0 flex-1" aria-hidden />
      <button
        type="button"
        className="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring"
        aria-label="Minimize the window"
        title="Minimize"
        onClick={() => act("minimize", getCurrentWindow().minimize())}
      >
        <MinusIcon aria-hidden className="size-4" />
      </button>
      <button
        type="button"
        className="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring"
        aria-label={maximized ? "Restore the window size" : "Maximize the window"}
        title={maximized ? "Restore" : "Maximize"}
        onClick={() => act("maximize", getCurrentWindow().toggleMaximize())}
      >
        <MaximizeIcon aria-hidden className="size-4" />
      </button>
      <button
        type="button"
        className="grid size-7 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground focus-visible:ring-2 focus-visible:ring-ring"
        aria-label="Close the window"
        title="Close"
        onClick={() => act("close", getCurrentWindow().close())}
      >
        <XIcon aria-hidden className="size-4" />
      </button>
    </header>
  );
}
