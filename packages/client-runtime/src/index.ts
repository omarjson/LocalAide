/**
 * The shared client runtime.
 *
 * GOAT's runtime carried the WebSocket RPC session, the orchestration state
 * atoms and the server-bound helpers. LocalAide's backend is a local command
 * table instead, so the runtime is the IPC bridge and the typed client for
 * it. Everything the renderer needs lives under `./localaide`.
 */

export * from "./localaide";
