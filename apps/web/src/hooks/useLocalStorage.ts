import { useSyncExternalStore } from "react";

/**
 * Local storage for the renderer.
 *
 * Vendored from GOAT (`apps/web/src/hooks/useLocalStorage.ts`, MIT). GOAT
 * validated every value with an Effect Schema; LocalAide stores plain JSON,
 * so a stored value that no longer matches its shape falls back to the
 * initial value instead of throwing.
 */

const fallbackStorage: Storage = (() => {
  const store = new Map<string, string>();
  return {
    clear: () => store.clear(),
    getItem: (key) => store.get(key) ?? null,
    key: (index) => [...store.keys()][index] ?? null,
    get length() {
      return store.size;
    },
    removeItem: (key) => store.delete(key),
    setItem: (key, value) => store.set(key, value),
  };
})();

const getStorage = () =>
  typeof window !== "undefined" ? window.localStorage : fallbackStorage;

const LOCAL_STORAGE_CHANGE_EVENT = "localaide:local_storage_change";

type ChangeDetail = { key: string };

function dispatchLocalStorageChange(key: string) {
  if (typeof window === "undefined") return;
  window.dispatchEvent(
    new CustomEvent<ChangeDetail>(LOCAL_STORAGE_CHANGE_EVENT, { detail: { key } }),
  );
}

function readRaw(key: string): string | null {
  try {
    return getStorage().getItem(key);
  } catch (error) {
    console.error("Could not read a stored value.", error);
    return null;
  }
}

/** The stored value, or the initial value when it is missing or unreadable. */
export function readLocalStorageValue<T>(key: string, initialValue: T): T {
  const raw = readRaw(key);
  if (raw === null) return initialValue;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return initialValue;
  }
}

export function getLocalStorageItem<T>(key: string, initialValue: T): T {
  return readLocalStorageValue(key, initialValue);
}

export function setLocalStorageItem<T>(key: string, value: T): void {
  try {
    getStorage().setItem(key, JSON.stringify(value));
  } catch (error) {
    console.error("Could not store a value.", error);
  }
}

export function removeLocalStorageItem(key: string): void {
  try {
    getStorage().removeItem(key);
  } catch (error) {
    console.error("Could not remove a stored value.", error);
  }
}

export function writeLocalStorageValue<T>(
  key: string,
  value: T | ((current: T) => T),
  initialValue: T,
): void {
  try {
    const current = readLocalStorageValue(key, initialValue);
    const next = typeof value === "function" ? (value as (current: T) => T)(current) : value;
    setLocalStorageItem(key, next);
    dispatchLocalStorageChange(key);
  } catch (error) {
    console.error("Could not update a stored value.", error);
  }
}

/** Fires on writes from this tab and from other tabs. */
export function subscribeLocalStorageKey(key: string, onChange: () => void): () => void {
  const handleStorageChange = (event: StorageEvent) => {
    if (event.key === key) onChange();
  };
  const handleLocalChange = (event: Event) => {
    if ((event as CustomEvent<ChangeDetail>).detail?.key === key) onChange();
  };
  window.addEventListener("storage", handleStorageChange);
  window.addEventListener(LOCAL_STORAGE_CHANGE_EVENT, handleLocalChange);
  return () => {
    window.removeEventListener("storage", handleStorageChange);
    window.removeEventListener(LOCAL_STORAGE_CHANGE_EVENT, handleLocalChange);
  };
}

export function useLocalStorage<T>(
  key: string,
  initialValue: T,
): [T, (value: T | ((current: T) => T)) => void] {
  const getSnapshot = () => readRaw(key);
  const subscribe = (onStoreChange: () => void) => subscribeLocalStorageKey(key, onStoreChange);
  const serialized = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  const stored = (() => {
    if (serialized === null) return initialValue;
    try {
      return JSON.parse(serialized) as T;
    } catch {
      return initialValue;
    }
  })();

  const setValue = (value: T | ((current: T) => T)) => {
    writeLocalStorageValue(key, value, initialValue);
  };

  return [stored, setValue];
}
