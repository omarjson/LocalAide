// Channels: the Tauri `Channel<T>` the frontend uses for streaming becomes
// an EventEmitter-backed IPC stream. The frontend sends a channel id, the
// backend emits {channelId, message} frames, the preload forwards them to the
// Channel instance that registered that id.

import { EventEmitter } from "node:events";

export const bus = new EventEmitter();
bus.setMaxListeners(0);

/** Emits one frame for a channel. */
export function emit(channelId, message) {
  bus.emit("frame", { channelId, message });
}

/** Emits a named app event (the `listen()` events the UI subscribes to). */
export function emitEvent(name, payload) {
  bus.emit("app-event", { name, payload });
}

/** A channel handle a command can write to. */
export function channel(id) {
  return {
    id,
    send: (message) => emit(id, message),
  };
}

