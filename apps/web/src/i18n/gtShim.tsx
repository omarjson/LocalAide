import type { ReactNode } from "react";

/**
 * GOAT's translation runtime, reduced to one language.
 *
 * GOAT ships nine locales and loads them with `gt-react`. LocalAide's copy
 * is English only for now, so the calls pass their text through. The
 * `{name}` placeholders still work, so a message reads the same.
 */

type Variables = Record<string, string | number>;

/** Marks a string as translatable. The text is the translation. */
export function msg(text: string): string {
  return text;
}

function interpolate(text: string, variables?: Variables): string {
  if (!variables) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = variables[name];
    return value === undefined ? match : String(value);
  });
}

/** The translation function. */
export function useGT(): (text: string, variables?: Variables) => string {
  return interpolate;
}

/** The message table. With one language it is the text itself. */
export function useMessages(): (text: string) => string {
  return (text: string) => text;
}

/** Holds the locale for the tree below it. */
export function GTProvider({
  children,
  locale = "en",
}: {
  children: ReactNode;
  locale?: string;
  translations?: unknown;
  _reload?: (state: { locale: string }) => void;
}) {
  void locale;
  return <>{children}</>;
}

export default GTProvider;
