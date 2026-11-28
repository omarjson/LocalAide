import { useEffect } from "react";
import { Link, useNavigate, useParams } from "@tanstack/react-router";
import { MessageCircleIcon, PlusIcon, TrashIcon } from "~/components/solar";
import { Button } from "~/components/ui/button";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "~/components/ui/sidebar";
import { deleteThread, refreshThreads, useThreads, useThreadsError } from "~/state/threads";
import { cn } from "~/lib/utils";

/**
 * The conversation list.
 *
 * Vendored from GOAT (`apps/web/src/components/Sidebar.tsx`, MIT). GOAT's
 * sidebar grouped threads by project and environment; LocalAide keeps one
 * list of conversations, newest first.
 */
export function ThreadSidebar() {
  const threads = useThreads();
  const error = useThreadsError();
  const navigate = useNavigate();
  const threadId = useParams({
    strict: false,
    select: (params) => (params.threadId as string | undefined) ?? null,
  });

  useEffect(() => {
    void refreshThreads();
  }, []);

  return (
    <Sidebar
      side="left"
      variant="framed"
      collapsible="offcanvas"
      className="border-r border-border"
      aria-label="Conversations"
    >
      <SidebarHeader className="gap-2">
        <Button
          variant="default"
          className="w-full justify-start gap-2"
          onClick={() => void navigate({ to: "/" })}
        >
          <PlusIcon aria-hidden className="size-4" />
          New conversation
        </Button>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu>
            {error ? (
              <p className="px-2 py-1 text-xs text-destructive">{error}</p>
            ) : null}
            {threads.length === 0 && !error ? (
              <p className="px-2 py-1 text-xs text-muted-foreground">
                No conversations yet.
              </p>
            ) : null}
            {threads.map((thread) => (
              <SidebarMenuItem key={thread.id}>
                <SidebarMenuButton
                  render={
                    <Link
                      to="/thread/$threadId"
                      params={{ threadId: thread.id }}
                      className={cn(thread.id === threadId && "bg-sidebar-row-selected")}
                    >
                      <MessageCircleIcon aria-hidden className="size-4 shrink-0" />
                      <span className="truncate">{thread.title || "Chat"}</span>
                    </Link>
                  }
                />
                <button
                  type="button"
                  className="absolute right-1 grid size-6 place-items-center rounded-md text-muted-foreground opacity-0 transition-opacity hover:bg-accent hover:text-foreground focus-visible:opacity-100 group-hover/menu-item:opacity-100"
                  aria-label={`Delete ${thread.title || "this conversation"}`}
                  onClick={() => void deleteThread(thread.id)}
                >
                  <TrashIcon aria-hidden className="size-3.5" />
                </button>
              </SidebarMenuItem>
            ))}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarRail />
    </Sidebar>
  );
}
