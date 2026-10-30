declare module "@tauri-apps/api/core" {
  export function invoke<T = any>(cmd: string, args?: any): Promise<T>;
}
declare module "@tauri-apps/api/event" {
  export function listen<T = any>(event: string, handler: (e: { payload: T }) => void): Promise<{ unlisten: () => void }>;
}
