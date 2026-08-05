import { CheckCircle } from "@phosphor-icons/react";
import { normalizeReviewTerminals } from "@/lib/reviewTerminals";
import { cn } from "@/lib/utils";
import type { ReviewTerminal, ReviewTerminalOption } from "@/types";

export interface ReviewTerminalPickerProps {
  terminals: ReviewTerminalOption[];
  value: ReviewTerminal | null;
  onChange: (value: ReviewTerminal | null) => void;
  allowUnset?: boolean;
  unsetLabel?: string;
  unsetDescription?: string;
}

export function ReviewTerminalPicker({
  terminals,
  value,
  onChange,
  allowUnset = false,
  unsetLabel = "Ask on first use",
  unsetDescription = "If unset, Norn will ask when you click Review with Claude.",
}: ReviewTerminalPickerProps) {
  const choices = normalizeReviewTerminals(terminals);

  return (
    <div className="flex flex-col gap-2">
      {allowUnset && (
        <button
          type="button"
          onClick={() => onChange(null)}
          className={cn(
            "rounded-lg border px-3 py-3 text-left transition-colors",
            value === null ? "border-primary bg-primary/5" : "border-border hover:bg-muted/50",
          )}
        >
          <div className="flex items-center justify-between gap-3">
            <span className="font-medium">{unsetLabel}</span>
            {value === null && <CheckCircle size={16} className="text-primary" weight="fill" />}
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{unsetDescription}</p>
        </button>
      )}

      {choices.map((choice) => (
        <button
          key={choice.id}
          type="button"
          onClick={() => onChange(choice.id)}
          disabled={!choice.available}
          className={cn(
            "rounded-lg border px-3 py-3 text-left transition-colors",
            value === choice.id ? "border-primary bg-primary/5" : "border-border hover:bg-muted/50",
            !choice.available && "cursor-not-allowed opacity-50 hover:bg-transparent",
          )}
        >
          <div className="flex items-center justify-between gap-3">
            <span className="font-medium">{choice.label}</span>
            <span className="flex items-center gap-2 text-xs text-muted-foreground">
              {!choice.available && "Not installed"}
              {value === choice.id && (
                <CheckCircle size={16} className="text-primary" weight="fill" />
              )}
            </span>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{choice.description}</p>
        </button>
      ))}
    </div>
  );
}
