import { useEffect } from "react";
import { useParams } from "@tanstack/react-router";
import { ApprovalCard } from "~/components/chat/ApprovalCard";
import { Composer } from "~/components/chat/Composer";
import { MessageList } from "~/components/chat/MessageList";
import { loadThread, useChatError, useChatMessages, useChatStatus } from "~/state/chat";

/** A saved conversation. */
export function ThreadPage() {
  const { threadId } = useParams({ from: "/thread/$threadId" });
  const messages = useChatMessages();
  const status = useChatStatus();
  const error = useChatError();

  useEffect(() => {
    if (threadId) void loadThread(threadId);
  }, [threadId]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <MessageList messages={messages} />
      {status ? (
        <p className="mx-auto w-full max-w-3xl px-4 pb-2 text-xs text-muted-foreground">{status}</p>
      ) : null}
      {error ? (
        <p className="mx-auto w-full max-w-3xl px-4 pb-2 text-xs text-destructive">{error}</p>
      ) : null}
      <ApprovalCard />
      <Composer />
    </div>
  );
}
