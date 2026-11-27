/**
 * The IPC surface the app was written against, backed by Electron.
 *
 * Every component imports `@tauri-apps/api/*`; Vite aliases those specifiers
 * here. Nothing in the UI changed: the names, argument shapes and streaming
 * behaviour are the ones the Tauri build had. The browser preview keeps
 * working because the preview shim installs the same globals this reads.
 */

type Frame = { index?: number; message?: unknown; end?: unknown };
type Callback = (frame: Frame) => void;

type Internals = {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback?: (cb: Callback, once?: boolean) => number;
  unregisterCallback?: (id: number) => void;
  convertFileSrc?: (path: string, protocol?: string) => string;
};

type Bridge = {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  on: (name: string, cb: (payload: unknown) => void) => () => void;
  convertFileSrc: (path: string, protocol?: string) => string;
  version: () => Promise<string>;
  window: {
    minimize: () => Promise<void>;
    toggleMaximize: () => Promise<void>;
    close: () => Promise<void>;
    isMaximized: () => Promise<boolean>;
    onMaximized: (cb: (maximized: boolean) => void) => () => void;
  };
  pathForFile: (file: File) => string;
};

export type InvokeArgs = Record<string, unknown>;

const globals = () => globalThis as unknown as {
  __LOCALAIDE__?: Bridge;
  __TAURI_INTERNALS__?: Internals;
  __LOCALAIDE_PREVIEW__?: boolean;
};

const bridge = (): Bridge | null => globals().__LOCALAIDE__ ?? null;

/** A streaming channel: the backend sends frames, `onmessage` sees each one. */
export class Channel<T> {
  id: number;
  onmessage: (message: T) => void = () => {};
  private next = 0;
  private pending = new Map<number, T>();

  constructor(onmessage?: (message: T) => void) {
    if (onmessage) this.onmessage = onmessage;
    const internals = globals().__TAURI_INTERNALS__;
    const handler: Callback = (frame) => {
      const index = typeof frame?.index === "number" ? frame.index : this.next;
      if (frame && "end" in frame) return;
      const message = frame?.message as T;
      if (index === this.next) {
        this.deliver(message);
        this.next += 1;
        let queued = this.pending.get(this.next);
        while (queued !== undefined) {
          this.pending.delete(this.next);
          this.deliver(queued);
          this.next += 1;
          queued = this.pending.get(this.next);
        }
      } else {
        this.pending.set(index, message);
      }
    };
    this.id = internals?.transformCallback ? internals.transformCallback(handler) : nextChannelId();
    if (!internals?.transformCallback) localChannels.set(this.id, handler);
  }

  private deliver(message: T) {
    try {
      this.onmessage?.(message);
    } catch (error) {
      queueMicrotask(() => {
        throw error;
      });
    }
  }

  toJSON() {
    return { __channelId: this.id };
  }
}

let channelSeq = 0;
const localChannels = new Map<number, Callback>();
const nextChannelId = () => ++channelSeq;

/** Frames the Electron bridge routed to a channel created here. */
export function deliverFrame(channelId: number, frame: Frame) {
  localChannels.get(channelId)?.(frame);
}

export async function invoke<T>(cmd: string, args: InvokeArgs = {}): Promise<T> {
  const local = bridge();
  if (local) return (await local.invoke(cmd, args)) as T;
  const internals = globals().__TAURI_INTERNALS__;
  if (internals) return (await internals.invoke(cmd, args)) as T;
  throw { code: "NO_BACKEND", message: "LocalAide's backend is not running." };
}

export function convertFileSrc(filePath: string, protocol = "asset"): string {
  const local = bridge();
  if (local) return local.convertFileSrc(filePath, protocol);
  const internals = globals().__TAURI_INTERNALS__;
  return internals?.convertFileSrc?.(filePath, protocol) ?? filePath;
}

/** A named app event (engine-status, automations, downloads). */
export async function listen<T>(
  event: string,
  handler: (event: { event: string; id: number; payload: T }) => void,
): Promise<() => void> {
  const local = bridge();
  if (local) {
    const off = local.on(event, (payload) => handler({ event, id: 0, payload: payload as T }));
    return off;
  }
  const internals = globals().__TAURI_INTERNALS__;
  if (internals?.transformCallback) {
    const id = internals.transformCallback((frame) => handler(frame as { event: string; id: number; payload: T }));
    await invoke("plugin:event|listen", { event, target: { kind: "Any" }, handler: id });
    return () => {
      void invoke("plugin:event|unlisten", { event, eventId: id }).catch(() => undefined);
    };
  }
  return () => undefined;
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  await invoke("plugin:event|emit", { event, payload });
}

export async function getVersion(): Promise<string> {
  const local = bridge();
  if (local) return local.version();
  try {
    return await invoke<string>("plugin:app|version");
  } catch {
    return "0.0.0";
  }
}

/** The window controls the title bar drives. */
export function getCurrentWindow() {
  const local = bridge();
  const noop = async () => undefined;
  return {
    label: "main",
    minimize: () => local?.window.minimize() ?? noop(),
    toggleMaximize: () => local?.window.toggleMaximize() ?? noop(),
    close: () => local?.window.close() ?? noop(),
    destroy: () => local?.window.close() ?? noop(),
    isMaximized: async () => (local ? local.window.isMaximized() : false),
    onResized: async (cb: () => void) => {
      if (typeof window === "undefined") return () => undefined;
      const onResize = () => cb();
      window.addEventListener("resize", onResize);
      return () => window.removeEventListener("resize", onResize);
    },
    onCloseRequested: async () => () => undefined,
  };
}

/** Files dropped on the window, as paths the backend can read. */
export function getCurrentWebview() {
  return {
    label: "main",
    onDragDropEvent: async (
      handler: (event: { payload: { type: string; paths?: string[]; position?: { x: number; y: number } } }) => void,
    ) => {
      if (typeof window === "undefined") return () => undefined;
      const pathOf = (file: File): string => {
        const local = bridge();
        try {
          return local ? local.pathForFile(file) : "";
        } catch {
          return "";
        }
      };
      let depth = 0;
      const onEnter = (e: DragEvent) => {
        depth += 1;
        if (depth === 1) handler({ payload: { type: "enter", paths: [] } });
        e.preventDefault();
      };
      const onOver = (e: DragEvent) => {
        e.preventDefault();
        handler({ payload: { type: "over", paths: [] } });
      };
      const onLeave = () => {
        depth = Math.max(0, depth - 1);
        if (depth === 0) handler({ payload: { type: "leave" } });
      };
      const onDrop = (e: DragEvent) => {
        e.preventDefault();
        depth = 0;
        const paths = [...(e.dataTransfer?.files ?? [])].map(pathOf).filter(Boolean);
        handler({ payload: { type: "drop", paths } });
      };
      window.addEventListener("dragenter", onEnter);
      window.addEventListener("dragover", onOver);
      window.addEventListener("dragleave", onLeave);
      window.addEventListener("drop", onDrop);
      return () => {
        window.removeEventListener("dragenter", onEnter);
        window.removeEventListener("dragover", onOver);
        window.removeEventListener("dragleave", onLeave);
        window.removeEventListener("drop", onDrop);
      };
    },
  };
}
