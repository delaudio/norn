import { Check, PaperPlaneTilt, PencilSimple, Trash, X } from "@phosphor-icons/react";
import { useState } from "react";
import { Markdown } from "@/components/Markdown";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  draftCommentAnchorId,
  draftCommentLocationLabel,
  draftCommentLocationTitle,
} from "@/lib/draftComments";
import { cn } from "@/lib/utils";
import type { DraftComment } from "@/types";

export interface DraftCommentRowProps {
  draft: DraftComment;
  active?: boolean;
  publishing?: boolean;
  onFocus?: () => void;
  onRemove: () => void;
  onUpdate: (raw: string) => void;
  onPublish: () => void;
}

export function DraftCommentRow({
  draft,
  active = false,
  publishing = false,
  onFocus,
  onRemove,
  onUpdate,
  onPublish,
}: DraftCommentRowProps) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState("");

  const trimmed = (editing ? value : draft.raw).trim();
  const locationLabel = draftCommentLocationLabel(draft);
  const locationTitle = draftCommentLocationTitle(draft);

  return (
    <div
      id={draftCommentAnchorId(draft.localId)}
      className={cn(
        "m-2 scroll-mt-24 rounded-md border border-dashed border-border bg-background/60 p-3 font-sans text-sm transition-colors",
        active && "border-primary bg-primary/5 shadow-[inset_0_0_0_1px_var(--primary)]",
      )}
    >
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <Badge variant="muted">Pending</Badge>
        {draft.findingRef && <Badge variant="secondary">AI finding</Badge>}
        <button
          type="button"
          onClick={onFocus}
          className="truncate text-left font-medium text-foreground hover:text-primary hover:underline"
          title={locationTitle}
        >
          {locationLabel}
        </button>
        {active && <Badge variant="secondary">Selected</Badge>}
        <div className="ml-auto flex flex-wrap items-center gap-1">
          {editing ? (
            <>
              <Button
                size="sm"
                variant="secondary"
                disabled={!trimmed || publishing}
                onClick={() => {
                  onUpdate(trimmed);
                  setEditing(false);
                }}
              >
                <Check size={14} />
                Save
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={publishing}
                onClick={() => {
                  setValue("");
                  setEditing(false);
                }}
              >
                <X size={14} />
                Cancel
              </Button>
            </>
          ) : (
            <>
              <Button
                size="sm"
                variant="ghost"
                disabled={publishing}
                onClick={() => {
                  setValue(draft.raw);
                  setEditing(true);
                }}
              >
                <PencilSimple size={14} />
                Edit
              </Button>
              <Button size="sm" variant="secondary" disabled={publishing} onClick={onPublish}>
                <PaperPlaneTilt size={14} />
                {publishing ? "Publishing…" : "Publish"}
              </Button>
              <Button size="sm" variant="ghost" disabled={publishing} onClick={onRemove}>
                <Trash size={14} />
                Remove
              </Button>
            </>
          )}
        </div>
      </div>

      {editing ? (
        <div className="mt-2">
          <textarea
            aria-label="Edit draft comment"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            rows={4}
            className="w-full resize-y rounded-md border border-input bg-background px-2 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && trimmed) {
                onUpdate(trimmed);
                setEditing(false);
              }
              if (event.key === "Escape") {
                setValue("");
                setEditing(false);
              }
            }}
          />
          <div className="mt-2 text-right text-[11px] text-muted-foreground">
            Cmd/Ctrl + Enter to save
          </div>
        </div>
      ) : (
        <Markdown className="mt-2">{draft.raw}</Markdown>
      )}
    </div>
  );
}
