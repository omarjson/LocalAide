import type { Action } from "svelte/action";

export interface PopoverOptions {
  /** Whether the menu is showing. */
  open: boolean;
  /** Hides the menu. */
  onClose: () => void;
}

const ITEMS = '[role="menuitem"]:not([disabled]), [data-popover-item]:not([disabled])';

/**
 * Put this on an element that holds both a menu button and its menu. The
 * button is the element inside it with `aria-expanded`.
 *
 * While the menu is open: Escape closes it and puts focus back on the button,
 * a click outside closes it, and the arrow keys, Home and End move between the
 * items (`role="menuitem"` or `data-popover-item`). Opening the menu focuses
 * its first item.
 */
export const popover: Action<HTMLElement, PopoverOptions> = (node, initial) => {
  let options = initial;

  const items = () => Array.from(node.querySelectorAll<HTMLElement>(ITEMS));

  const onDocumentKeydown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    // Handled here first, so Escape does not also stop an answer or close a dialog.
    event.preventDefault();
    event.stopPropagation();
    options.onClose();
    node.querySelector<HTMLElement>("[aria-expanded]")?.focus();
  };

  const onPointerDown = (event: PointerEvent) => {
    if (event.target instanceof Node && !node.contains(event.target)) {
      options.onClose();
    }
  };

  const onKeydown = (event: KeyboardEvent) => {
    if (!options.open || !["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const list = items();
    if (list.length === 0) return;
    event.preventDefault();
    const current = list.findIndex((item) => item === document.activeElement);
    const last = list.length - 1;
    let next = 0;
    if (event.key === "End") next = last;
    else if (event.key === "ArrowDown") next = current < 0 || current === last ? 0 : current + 1;
    else if (event.key === "ArrowUp") next = current <= 0 ? last : current - 1;
    list[next]?.focus();
  };

  const sync = (wasOpen: boolean) => {
    if (options.open && !wasOpen) {
      document.addEventListener("keydown", onDocumentKeydown, true);
      document.addEventListener("pointerdown", onPointerDown, true);
      // The menu renders in the same update, so wait a frame before focusing it.
      requestAnimationFrame(() => {
        if (options.open) items()[0]?.focus();
      });
    } else if (!options.open && wasOpen) {
      document.removeEventListener("keydown", onDocumentKeydown, true);
      document.removeEventListener("pointerdown", onPointerDown, true);
    }
  };

  node.addEventListener("keydown", onKeydown);
  sync(false);

  return {
    update(next: PopoverOptions) {
      const wasOpen = options.open;
      options = next;
      sync(wasOpen);
    },
    destroy() {
      const wasOpen = options.open;
      options = { ...options, open: false };
      sync(wasOpen);
      node.removeEventListener("keydown", onKeydown);
    },
  };
};
