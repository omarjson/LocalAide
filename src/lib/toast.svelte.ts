// One short message at the bottom of the window, shared by every screen.
// App.svelte draws it. Errors stay longer and can be dismissed.

export type ToastTone = "info" | "error";

export const toast = $state<{ message: string | null; tone: ToastTone }>({
  message: null,
  tone: "info",
});

let timer: ReturnType<typeof setTimeout> | undefined;

export function showToast(message: string, tone: ToastTone = "info"): void {
  toast.message = message;
  toast.tone = tone;
  clearTimeout(timer);
  timer = setTimeout(() => {
    toast.message = null;
  }, tone === "error" ? 8000 : 3000);
}

export function dismissToast(): void {
  clearTimeout(timer);
  toast.message = null;
}
