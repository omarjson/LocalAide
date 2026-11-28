/**
 * LocalAide's renderer runtime: the IPC bridge, the typed command client and
 * the backend wire types.
 *
 * Vendored from GOAT's `packages/client-runtime` (MIT, github.com/goat-si/goat),
 * reduced to the part that replaces GOAT's WebSocket RPC with LocalAide's
 * `localaide:invoke` command table.
 */

export * from "./bridge";
export * from "./client";
export * from "./errors";
export type * from "./bindings";
