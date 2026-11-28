import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Copies text to the clipboard and reports whether the last copy worked.
 *
 * Vendored from GOAT (`apps/web/src/hooks/useCopyToClipboard.ts`, MIT), reduced
 * to the plain copy the toast and the chat need.
 */
export function useCopyToClipboard(_options?: { target?: string }) {
  const [isCopied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const copyToClipboard = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 2000);
      return true;
    } catch {
      setCopied(false);
      return false;
    }
  }, []);

  return { copyToClipboard, isCopied };
}
