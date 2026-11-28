// The renderer talks to the backend through one bridge. These checks keep
// the bridge honest: the command reaches the backend under its own name, a
// streaming channel travels as a channel id and not as a class instance, and
// a page without the backend says so instead of failing silently.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  Channel,
  hasBackend,
  invoke,
  listen,
} from "../../packages/client-runtime/src/localaide/bridge.ts";

type Globals = {
  __LOCALAIDE__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

const globals = globalThis as unknown as Globals;

const withoutBridge = () => {
  delete globals.__LOCALAIDE__;
  delete globals.__TAURI_INTERNALS__;
};

const withBridge = (bridge: unknown, internals?: unknown) => {
  globals.__LOCALAIDE__ = bridge;
  if (internals) globals.__TAURI_INTERNALS__ = internals;
  else delete globals.__TAURI_INTERNALS__;
};

test("a command reaches the backend under its own name and returns the result", async () => {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  withBridge({
    invoke: async (cmd: string, args: Record<string, unknown>) => {
      calls.push({ cmd, args });
      return { ok: true };
    },
    on: () => () => undefined,
  });

  const result = await invoke<{ ok: boolean }>("memory_list", { path: "note.md" });

  assert.deepEqual(result, { ok: true });
  assert.equal(calls.length, 1);
  assert.equal(calls[0]?.cmd, "memory_list");
  assert.deepEqual(calls[0]?.args, { path: "note.md" });
  withoutBridge();
});

test("a streaming channel travels as a channel id, not as an object with functions", async () => {
  let sent: Record<string, unknown> | null = null;
  withBridge(
    {
      invoke: async (_cmd: string, args: Record<string, unknown>) => {
        sent = args;
        return "job-1";
      },
      on: () => () => undefined,
    },
    {
      transformCallback: () => 1,
      unregisterCallback: () => undefined,
    },
  );

  const channel = new Channel<{ type: string }>();
  const jobId = await invoke<string>("chat_stream", { req: { message: "hi" }, channel });

  assert.equal(jobId, "job-1");
  assert.ok(sent);
  const wire = sent as unknown as { channel: { __channelId: number } };
  assert.equal(typeof wire.channel.__channelId, "number");
  assert.deepEqual(Object.keys(wire.channel), ["__channelId"]);
  withoutBridge();
});

test("the channel delivers every frame the backend sends", () => {
  const handlers = new Map<number, (payload: unknown) => void>();
  let nextId = 0;
  withBridge(
    {
      invoke: async () => undefined,
      on: () => () => undefined,
    },
    {
      transformCallback: (cb: (payload: unknown) => void) => {
        const id = ++nextId;
        handlers.set(id, cb);
        return id;
      },
      unregisterCallback: (id: number) => handlers.delete(id),
    },
  );

  const seen: string[] = [];
  const channel = new Channel<{ type: string }>();
  channel.onmessage = (message) => seen.push(message.type);

  handlers.get(channel.id)?.({ message: { type: "token" } });
  handlers.get(channel.id)?.({ message: { type: "done" } });

  assert.deepEqual(seen, ["token", "done"]);
  withoutBridge();
});

test("a page without the backend says so", async () => {
  withoutBridge();
  assert.equal(hasBackend(), false);
  await assert.rejects(() => invoke("memory_list"), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "NO_BACKEND");
    return true;
  });
});

test("a named event can be listened to and stopped", async () => {
  const listeners = new Map<string, (payload: unknown) => void>();
  let stopped = false;
  withBridge({
    invoke: async () => undefined,
    on: (name: string, cb: (payload: unknown) => void) => {
      listeners.set(name, cb);
      return () => {
        stopped = true;
        listeners.delete(name);
      };
    },
  });

  const seen: unknown[] = [];
  const stop = await listen("automations-changed", (payload) => seen.push(payload));
  listeners.get("automations-changed")?.({ changed: true });
  stop();

  assert.deepEqual(seen, [{ changed: true }]);
  assert.equal(stopped, true);
  withoutBridge();
});
