import { useEffect, useRef, useState } from "react";
import { SendIcon, CircleStopIcon } from "~/components/solar";
import { Button } from "~/components/ui/button";
import { Textarea } from "~/components/ui/textarea";
import { cancelAnswer, sendMessage, useChatStreaming } from "~/state/chat";
import { cn } from "~/lib/utils";

/**
 * The composer: what the user writes and sends.
 *
 * Vendored from GOAT (`apps/web/src/components/chat/`, MIT). GOAT's composer
 * carries attachments, model picks, skills and modes; LocalAide's sends the
 * text and shows the model's answer.
 */
export function Composer({
  className,
  placeholder = "Ask anything…",
  onSent,
}: {
  className?: string;
  placeholder?: string;
  onSent?: () => void;
}) {
  const [text, setText] = useState("");
  const streaming = useChatStreaming();
  const area = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    if (!streaming) area.current?.focus();
  }, [streaming]);

  const send = async () => {
    const value = text.trim();
    if (!value || streaming) return;
    setText("");
    onSent?.();
    await sendMessage(value);
  };

  return (
    <div className={cn("px-4 pb-4", className)}>
      <div className="mx-auto w-full max-w-3xl">
        <div className="surface-glass flex flex-col gap-2 rounded-2xl border border-border p-2 shadow-sm">
          <Textarea
            ref={area}
            value={text}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                void send();
              }
            }}
            placeholder={placeholder}
            aria-label="Message"
            className="border-0 bg-transparent shadow-none before:hidden"
          />
          <div className="flex items-center justify-end gap-2">
            {streaming ? (
              <Button variant="secondary" size="sm" onClick={() => void cancelAnswer()}>
                <CircleStopIcon aria-hidden className="size-4" />
                Stop
              </Button>
            ) : (
              <Button size="sm" disabled={!text.trim()} onClick={() => void send()}>
                <SendIcon aria-hidden className="size-4" />
                Send
              </Button>
            )}
          </div>
        </div>
        <p className="mt-2 text-center text-xs text-muted-foreground">
          Enter sends, Shift+Enter starts a new line.
        </p>
      </div>
    </div>
  );
}
