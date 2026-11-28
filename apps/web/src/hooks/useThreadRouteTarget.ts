import { useParams } from "@tanstack/react-router";
import { asEnvironmentId, asThreadId, type ScopedThreadRef } from "@goat/contracts";

/**
 * The thread the window is showing.
 *
 * Vendored from GOAT (`apps/web/src/hooks/useThreadRouteTarget.ts`, MIT).
 * GOAT's threads live inside an environment and a draft could stand in for
 * one; LocalAide runs on the user's own computer, so the environment is
 * always the local one and there are no drafts.
 */
export function useThreadRouteTarget(): { kind: "server"; threadRef: ScopedThreadRef } | null {
  const threadId = useParams({ strict: false, select: (params) => params.threadId });
  if (!threadId) return null;
  return {
    kind: "server",
    threadRef: {
      threadId: asThreadId(threadId),
      environmentId: asEnvironmentId("local"),
    },
  };
}

export function useThreadRouteRef(): ScopedThreadRef | null {
  return useThreadRouteTarget()?.threadRef ?? null;
}
