import { api, errorMessage } from "./api/client";
import { showToast } from "./toast.svelte";

/**
 * Opens a link in the user's browser instead of inside the app window. Use it
 * as the link's click handler. The backend opens http(s) links only.
 */
export function openInBrowser(event: MouseEvent, href: string): void {
  event.preventDefault();
  api.openLink(href).catch((e: unknown) => {
    showToast(`The link could not open: ${errorMessage(e)}`, "error");
  });
}
