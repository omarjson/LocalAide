/**
 * The bridge from the renderer to LocalAide's Electron backend.
 *
 * GOAT's client runtime talked to a GOAT server over a WebSocket carrying the
 * Effect RPC protocol (`orchestration.dispatchCommand`, `subscribeThread`, and
 * a few dozen more methods). LocalAide has no such server: the backend
 * answers one `localaide:invoke` channel with a flat table of 204 commands,
 * and streams progress over a channel handle.
 *
 * This module is the replacement transport. It speaks the same two shapes the
 * old runtime used - a unary call and a streaming channel - but over Electron
 * IPC instead of a socket.
 */

export type InvokeArgs = Record<string, unknown>;

/** The bridge Electron's preload puts on the page. */
export type LocalAideBridge = {
  invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>;
  on: (name: string, cb: (payload: unknown) => void) => () => void;
  convertFileSrc: (filePath: string) => string;
  version: () => Promise<string>;
  window: {
    minimize: () => Promise<void>;
    toggleMaximize: () => Promise<void>;
    close: () => Promise<void>;
    isMaximized: () => Promise<boolean>;
    onMaximized: (cb: (maximized: boolean) => void) => () => void;
  };
  pathForFile: (file: File) => string;
  platform: string;
  versions: { electron: string; chrome: string };
};

/** The Tauri-compatible surface the preload also exposes. */
export type LocalAideInternals = {
  invoke: (cmd: string, args?: InvokeArgs) => Promise<unknown>;
  transformCallback?: (cb: (payload: unknown) => void, once?: boolean) => number;
  unregisterCallback?: (id: number) => void;
  convertFileSrc?: (filePath: string) => string;
};

type Globals = {
  __LOCALAIDE__?: LocalAideBridge;
  __TAURI_INTERNALS__?: LocalAideInternals;
};

const globals = () => globalThis as unknown as Globals;

const bridge = (): LocalAideBridge | null => globals().__LOCALAIDE__ ?? null;

/** True when the Electron backend is reachable from this page. */
export function hasBackend(): boolean {
  return Boolean(globals().__LOCALAIDE__ ?? globals().__TAURI_INTERNALS__);
}

/**
 * A streaming channel. The backend sends frames for it while a command runs;
 * `onmessage` sees each frame's message in order.
 */
export class Channel<T> {
  /** The id the backend streams frames to. */
  readonly id: number;
  onmessage: (message: T) => void = () => {};

  constructor() {
    const internals = globals().__TAURI_INTERNALS__;
    if (!internals?.transformCallback) {
      throw { code: "NO_BACKEND", message: "LocalAide's backend is not running." };
    }
    this.id = internals.transformCallback((frame) => {
      const message = (frame as { message?: unknown } | undefined)?.message as T;
      try {
        this.onmessage?.(message);
      } catch (error) {
        queueMicrotask(() => {
          throw error;
        });
      }
    });
  }

  /** How the channel travels to the backend. */
  toJSON(): { __channelId: number } {
    return { __channelId: this.id };
  }
}

/** Calls one backend command and returns its result. */
export async function invoke<T>(cmd: string, args: InvokeArgs = {}): Promise<T> {
  const wire: InvokeArgs = {};
  for (const [key, value] of Object.entries(args)) {
    wire[key] = value instanceof Channel ? value.toJSON() : value;
  }
  const local = bridge();
  if (local) return (await local.invoke(cmd, wire)) as T;
  const internals = globals().__TAURI_INTERNALS__;
  if (internals) return (await internals.invoke(cmd, wire)) as T;
  throw { code: "NO_BACKEND", message: "LocalAide's backend is not running." };
}

/** Subscribes to a named backend event ("engine-download", "automations-changed"). */
export async function listen<T>(name: string, handler: (payload: T) => void): Promise<() => void> {
  const local = bridge();
  if (local) return local.on(name, (payload) => handler(payload as T));
  return () => undefined;
}

/** The app version the backend reports. */
export async function getVersion(): Promise<string> {
  const local = bridge();
  if (local) return local.version();
  return "0.0.0";
}

/** A file path the renderer can show, through the app's own file protocol. */
export function convertFileSrc(filePath: string): string {
  const local = bridge();
  if (local) return local.convertFileSrc(filePath);
  const internals = globals().__TAURI_INTERNALS__;
  return internals?.convertFileSrc?.(filePath) ?? filePath;
}

/** The window controls the title bar drives. */
export function getCurrentWindow() {
  const local = bridge();
  const noop = async () => undefined;
  return {
    minimize: () => local?.window.minimize() ?? noop(),
    toggleMaximize: () => local?.window.toggleMaximize() ?? noop(),
    close: () => local?.window.close() ?? noop(),
    isMaximized: async () => (local ? await local.window.isMaximized() : false),
    onMaximized: (cb: (maximized: boolean) => void) => {
      if (!local) return () => undefined;
      return local.window.onMaximized(cb);
    },
  };
}

/** The path of a file the user dropped on the window. */
export function pathForFile(file: File): string {
  const local = bridge();
  if (!local) return "";
  try {
    return local.pathForFile(file);
  } catch {
    return "";
  }
}

/** The platform the app runs on ("darwin", "win32", "linux"). */
export function platform(): string {
  return bridge()?.platform ?? "unknown";
}
