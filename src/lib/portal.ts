/**
 * Renders a block at the document root. `position: fixed` inside an ancestor
 * with a `transform` (the animating sidebar, the chat settings sheet) is
 * positioned against that ancestor instead of the viewport, which shrinks and
 * misplaces dialogs. `use:portal` on a plain wrapper moves the dialog out.
 */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy: () => node.remove(),
  };
}
