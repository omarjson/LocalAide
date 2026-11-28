import { useSyncExternalStore } from "react";

/**
 * A tiny store for the renderer's state.
 *
 * GOAT kept its state in Effect Atoms fed by the server; LocalAide's state
 * comes from the backend on demand, so a plain store with a subscription is
 * enough.
 */
export type Store<T> = {
  get: () => T;
  set: (next: T | ((current: T) => T)) => void;
  subscribe: (listener: () => void) => () => void;
};

export function createStore<T>(initial: T): Store<T> {
  let current = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => current,
    set: (next) => {
      const value = typeof next === "function" ? (next as (current: T) => T)(current) : next;
      if (Object.is(value, current)) return;
      current = value;
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

/**
 * Reads part of a store. The selector must return a stable value for an
 * unchanged state, the way `useSyncExternalStore` expects.
 */
export function useStore<T, S>(store: Store<T>, selector: (state: T) => S): S {
  return useSyncExternalStore(
    store.subscribe,
    () => selector(store.get()),
    () => selector(store.get()),
  );
}
