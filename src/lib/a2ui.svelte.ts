// The A2UI surfaces of one chat. Each finished ```a2ui block in an answer
// is applied once, in chat order, so a later answer can update or delete a
// surface an earlier one created. The data model lives here, so what the
// user types into a surface stays while the chat is open.

import { getContext, setContext } from "svelte";
import { buildSurfaces, type Surface } from "./a2ui";

const KEY = Symbol("a2ui-chat");

export interface BlockResult {
  /** Surfaces this block created; they are drawn where the block sits. */
  created: string[];
  problems: string[];
}

/** Finished ```a2ui blocks in a text, in order. */
export function a2uiBlocks(text: string): string[] {
  const out: string[] = [];
  const fence = /```a2ui\n([\s\S]*?)```/g;
  let m: RegExpExecArray | null;
  while ((m = fence.exec(text)) !== null) out.push(m[1].trimEnd());
  return out;
}

export class A2uiChat {
  surfaces = $state<Record<string, Surface>>({});
  results = $state<Record<string, BlockResult>>({});
  private order: string[] = [];

  /**
   * Applies every finished block in `texts` (the chat's answers, oldest
   * first) that was not applied yet. When the chat changed before the end
   * (an edit or a restored branch), everything is rebuilt from the start.
   */
  sync(texts: string[]): void {
    const blocks = texts.flatMap(a2uiBlocks);
    const keep = this.order.every((source, i) => blocks[i] === source);
    if (!keep) this.reset();
    for (let i = this.order.length; i < blocks.length; i++) {
      const source = blocks[i];
      const { created, problems } = buildSurfaces(source, this.surfaces);
      // A repeated identical block keeps the first result.
      if (!this.results[source]) this.results[source] = { created, problems };
      this.order.push(source);
    }
  }

  reset(): void {
    this.surfaces = {};
    this.results = {};
    this.order = [];
  }
}

export const provideA2ui = (chat: A2uiChat): A2uiChat => setContext(KEY, chat);
export const useA2ui = (): A2uiChat | undefined => getContext<A2uiChat | undefined>(KEY);
