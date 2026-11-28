import { useEffect } from "react";
import { Composer } from "~/components/chat/Composer";
import { MessageList } from "~/components/chat/MessageList";
import { resetChat, useChatMessages } from "~/state/chat";

/** The home page: a new conversation. */
export function ChatHome() {
  const messages = useChatMessages();

  useEffect(() => {
    resetChat(null);
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {messages.length === 0 ? (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-6 px-4">
          <div className="text-center">
            <h1 className="text-2xl font-semibold tracking-tight">LocalAide</h1>
            <p className="mt-2 text-sm text-muted-foreground">
              Your private assistant. It runs on this computer and answers from your own notes and
              files.
            </p>
          </div>
          <Composer className="w-full" placeholder="Ask anything…" />
        </div>
      ) : (
        <>
          <MessageList messages={messages} />
          <Composer />
        </>
      )}
    </div>
  );
}
