/**
 * Wire types shared by the LocalAide renderer and the Electron backend.
 *
 * Vendored from GOAT (`packages/contracts`, MIT, github.com/goat-si/goat) and
 * reduced to what the LocalAide shell needs. GOAT's Effect Schema brands are
 * plain TypeScript brands here, so the renderer does not need the Effect
 * runtime for an id.
 */

declare const brand: unique symbol;

/** A string that only one kind of id can be. */
export type Brand<Value, Name extends string> = Value & { readonly [brand]: Name };

export type ThreadId = Brand<string, "ThreadId">;
export type MessageId = Brand<string, "MessageId">;
export type ProjectId = Brand<string, "ProjectId">;
export type EnvironmentId = Brand<string, "EnvironmentId">;
export type DraftId = Brand<string, "DraftId">;

export const asThreadId = (value: string): ThreadId => value as ThreadId;
export const asMessageId = (value: string): MessageId => value as MessageId;
export const asProjectId = (value: string): ProjectId => value as ProjectId;
export const asEnvironmentId = (value: string): EnvironmentId => value as EnvironmentId;
export const asDraftId = (value: string): DraftId => value as DraftId;

/** A thread plus the environment it lives in. GOAT threads are per-environment. */
export type ScopedThreadRef = {
  readonly threadId: ThreadId;
  readonly environmentId: EnvironmentId;
};

export type ConfirmDialogVariant = "default" | "danger";

export type ConfirmDialogOptions = {
  readonly title: string;
  readonly description?: string;
  readonly confirmLabel?: string;
  readonly cancelLabel?: string;
  readonly variant?: ConfirmDialogVariant;
  readonly destructive?: boolean;
  /** Which button starts focused when the dialog opens. */
  readonly initialFocus?: "confirm" | "cancel" | null;
};
