// The bridge. Exposes exactly what the app reads — window.__LOCALAIDE__
// (the full bridge the shim prefers) and window.__TAURI_INTERNALS__ (the
// Tauri compatibility surface) — backed by Electron IPC. No node in the page.

const { contextBridge, ipcRenderer, webUtils } = require("electron");

let nextId = 0;
const callbacks = new Map();

// Channel<T>: an id the backend streams frames to.
class Channel {
  constructor() {
    this.id = ++nextId;
    callbacks.set(this.id, (message) => this.onmessage?.(message));
  }
  onmessage = null;
  toJSON() {
    return { __channelId: this.id };
  }
}

const transformCallback = (cb, once = false) => {
  const id = ++nextId;
  callbacks.set(
    id,
    once
      ? (payload) => {
          callbacks.delete(id);
          cb(payload);
        }
      : cb,
  );
  return id;
};

// Frames from the main process. The shim's Channel reads {index, message};
// the index is optional (ordered delivery is the default), so the wrapper
// carries the message and channels keep their own sequence.
ipcRenderer.on("localaide:frame", (_event, { channelId, message }) => {
  callbacks.get(channelId)?.({ message });
});
ipcRenderer.on("localaide:event", (_event, { name, payload }) => {
  for (const [, cb] of callbacks) {
    // Event listeners register with a name on the payload.
    if (cb.__eventName === name) cb(payload);
  }
});
ipcRenderer.on("localaide:window", (_event, state) => {
  window.dispatchEvent(new CustomEvent("localaide:window", { detail: state }));
});

// Invoke: serialize channels to ids, restore errors with their code.
const invoke = async (cmd, args = {}) => {
  const wire = {};
  for (const [key, value] of Object.entries(args ?? {})) {
    wire[key] = value instanceof Channel ? { __channelId: value.id } : value;
  }
  try {
    return await ipcRenderer.invoke("localaide:invoke", { cmd, args: wire });
  } catch (error) {
    // Electron wraps thrown values in a string; the main process sends a
    // structured object so the UI can read .code and .message.
    const payload = error?.cause ?? error;
    throw payload && typeof payload === "object" && "code" in payload
      ? payload
      : { code: "INTERNAL", message: String(payload?.message ?? payload) };
  }
};

contextBridge.exposeInMainWorld("__TAURI_INTERNALS__", {
  transformCallback,
  unregisterCallback: (id) => callbacks.delete(id),
  convertFileSrc: (filePath) => "localaide-file://" + encodeURIComponent(filePath),
  invoke,
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { label: "main", windowLabel: "main" },
    currentMonitor: null,
  },
});

// The full bridge the shim prefers: invoke, events, window controls, the
// app version, and drop-file paths.
contextBridge.exposeInMainWorld("__LOCALAIDE__", {
  invoke,
  on: (name, cb) => {
    const id = ++nextId;
    cb.__eventName = name;
    callbacks.set(id, cb);
    return () => {
      callbacks.delete(id);
    };
  },
  convertFileSrc: (filePath) => "localaide-file://" + encodeURIComponent(filePath),
  version: () => ipcRenderer.invoke("localaide:app-version"),
  window: {
    minimize: () => ipcRenderer.invoke("localaide:window-control", "minimize"),
    toggleMaximize: () => ipcRenderer.invoke("localaide:window-control", "toggle-maximize"),
    close: () => ipcRenderer.invoke("localaide:window-control", "close"),
    isMaximized: () => ipcRenderer.invoke("localaide:window-maximized"),
    onMaximized: (cb) => {
      const handler = (_event, state) => cb(state?.maximized === true);
      ipcRenderer.on("localaide:window", handler);
      return () => ipcRenderer.removeListener("localaide:window", handler);
    },
  },
  pathForFile: (file) => {
    try {
      return webUtils.getPathForFile(file);
    } catch {
      return "";
    }
  },
  // The event API (named app events), tagged like transformCallback does.
  listen: (name, cb) => {
    const id = ++nextId;
    cb.__eventName = name;
    callbacks.set(id, cb);
    return Promise.resolve(id);
  },
  unlisten: (id) => {
    callbacks.delete(id);
    return Promise.resolve();
  },
  platform: process.platform,
  versions: { electron: process.versions.electron, chrome: process.versions.chrome },
});
