import { ShieldQuestionIcon } from "~/components/solar";
import { Button } from "~/components/ui/button";
import { answerApproval, useChatApproval } from "~/state/chat";

/** The card that asks the user to allow a tool call. */
export function ApprovalCard() {
  const approval = useChatApproval();
  if (!approval) return null;

  return (
    <div className="mx-auto w-full max-w-3xl rounded-xl border border-border bg-muted/40 p-3">
      <div className="flex items-start gap-2">
        <ShieldQuestionIcon aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium">{approval.title}</p>
          {approval.detail ? (
            <p className="mt-1 text-xs text-muted-foreground">{approval.detail}</p>
          ) : null}
          {approval.preview ? (
            <pre className="mt-2 max-h-40 overflow-auto rounded-md bg-background/60 p-2 text-xs whitespace-pre-wrap">
              {approval.preview}
            </pre>
          ) : null}
          <div className="mt-3 flex flex-wrap gap-2">
            <Button size="sm" onClick={() => void answerApproval("allow")}>
              Allow once
            </Button>
            <Button size="sm" variant="secondary" onClick={() => void answerApproval("deny")}>
              Deny
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
