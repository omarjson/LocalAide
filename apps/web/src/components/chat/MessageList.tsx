import type { ChatMessage, MessagePart } from "@goat/client-runtime/localaide";
import {
  CheckIcon,
  CircleAlertIcon,
  LoaderCircleIcon,
  WrenchIcon,
} from "~/components/solar";
import { cn } from "~/lib/utils";

/**
 * The transcript.
 *
 * Vendored from GOAT (`apps/web/src/components/ChatView.tsx`, MIT). GOAT draws
 * one row per turn item; LocalAide draws one message per turn, with the
 * answer's thinking, tool calls and text in the order they happened.
 */
export function MessageList({ messages }: { messages: ChatMessage[] }) {
  return (
    <div className="messages-timeline-scroll flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-4 py-6">
      <div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
        {messages.map((message, index) => (
          <MessageRow key={`${message.ts ?? index}-${index}`} message={message} />
        ))}
      </div>
    </div>
  );
}

function MessageRow({ message }: { message: ChatMessage }) {
  if (message.role === "user") {
    return (
      <div className="flex justify-end">
        <div className="max-w-[85%] rounded-2xl rounded-br-md bg-accent px-4 py-2.5 text-sm text-accent-foreground">
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {message.parts && message.parts.length > 0 ? (
        message.parts.map((part, index) => <Part key={index} part={part} />)
      ) : (
        <p className="text-sm whitespace-pre-wrap">{message.content}</p>
      )}
    </div>
  );
}

function Part({ part }: { part: MessagePart }) {
  switch (part.type) {
    case "text":
      return part.text ? (
        <p className="text-sm leading-relaxed whitespace-pre-wrap">{part.text}</p>
      ) : null;
    case "reasoning":
      return (
        <details className="rounded-lg border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
          <summary className="cursor-pointer select-none font-medium">Thinking</summary>
          <p className="mt-2 whitespace-pre-wrap">{part.text}</p>
        </details>
      );
    case "tool":
      return <ToolCard part={part} />;
    case "question":
      return (
        <div className="rounded-lg border border-border bg-muted/40 px-3 py-2 text-sm">
          <p className="font-medium">{part.question}</p>
        </div>
      );
    case "todos":
      return (
        <ul className="flex flex-col gap-1 rounded-lg border border-border bg-muted/40 px-3 py-2 text-xs">
          {part.items.map((item) => (
            <li key={item.text} className="flex items-center gap-2">
              <CheckIcon
                aria-hidden
                className={cn(
                  "size-3.5",
                  item.status === "done" ? "text-foreground" : "text-muted-foreground/50",
                )}
              />
              <span className={cn(item.status === "done" && "text-muted-foreground line-through")}>
                {item.text}
              </span>
            </li>
          ))}
        </ul>
      );
  }
}

function ToolCard({ part }: { part: Extract<MessagePart, { type: "tool" }> }) {
  return (
    <div className="flex items-start gap-2 rounded-lg border border-border bg-muted/40 px-3 py-2 text-xs">
      <span className="mt-0.5 text-muted-foreground">
        {part.status === "running" ? (
          <LoaderCircleIcon aria-hidden className="size-3.5 animate-spin" />
        ) : part.status === "failed" || part.status === "denied" ? (
          <CircleAlertIcon aria-hidden className="size-3.5" />
        ) : (
          <WrenchIcon aria-hidden className="size-3.5" />
        )}
      </span>
      <span className="min-w-0">
        <span className="font-medium">{part.title || part.name}</span>
        {part.summary ? (
          <span className="text-muted-foreground"> — {part.summary}</span>
        ) : null}
      </span>
    </div>
  );
}
