import {
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";
import { AppShell } from "~/components/shell/AppShell";
import { ChatHome } from "~/pages/ChatHome";
import { SettingsPage } from "~/pages/SettingsPage";
import { ThreadPage } from "~/pages/ThreadPage";

/**
 * The routes.
 *
 * Vendored from GOAT (`apps/web/src/routes/`, MIT). GOAT's routes were files
 * under `src/routes` with a generated route tree; LocalAide keeps the same
 * router but writes the tree in code, so no code generation step runs.
 */
const rootRoute = createRootRoute({
  component: AppShell,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: ChatHome,
});

const threadRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/thread/$threadId",
  component: ThreadPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsPage,
});

export const routeTree = rootRoute.addChildren([indexRoute, threadRoute, settingsRoute]);

export const router = createRouter({
  routeTree,
  // The Electron window loads the app from a file, so the address bar holds a
  // file path. The hash carries the route instead.
  history: createHashHistory(),
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
