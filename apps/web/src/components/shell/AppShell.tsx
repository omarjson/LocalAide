import { Outlet } from "@tanstack/react-router";
import { SidebarProvider } from "~/components/ui/sidebar";
import { DestinationRail } from "./DestinationRail";
import { ThreadSidebar } from "./ThreadSidebar";
import { TitleBar } from "./TitleBar";

/**
 * The app shell: the window chrome, the destination rail, the conversation
 * list and the page the route chose.
 *
 * Vendored from GOAT (`apps/web/src/components/AppSidebarLayout.tsx`, MIT).
 */
export function AppShell() {
  return (
    <div className="flex h-svh w-full flex-col overflow-hidden bg-background text-foreground">
      <TitleBar />
      <SidebarProvider
        className="h-full min-h-0! flex-1"
        storageKey="localaide_sidebar_open"
        style={{ "--destination-rail-width": "3.5rem" } as React.CSSProperties}
      >
        <DestinationRail />
        <ThreadSidebar />
        <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          <Outlet />
        </main>
      </SidebarProvider>
    </div>
  );
}
