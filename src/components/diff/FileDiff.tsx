import { CaretDown, CaretRight, ChatCircleText, Sparkle } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { useMemo } from "react";
import { type ChangeEventArgs, Diff, Hunk } from "react-diff-view";
import { Badge } from "@/components/ui/badge";
import {
  type ChangeData,
  changeNewLine,
  changeOldLine,
  fileAnchorId,
  fileDisplayPath,
} from "@/lib/diff";
import { tokenizeFile } from "@/lib/highlight";
import {
  countReviewFileChanges,
  type ImageDiffMetadata,
  type ReviewFileData,
} from "@/lib/imageDiff";
import type { DiffViewMode } from "@/types";

type RenderableDiffViewMode = Exclude<DiffViewMode, "conversation">;

const STATUS_LABEL: Record<string, string> = {
  add: "added",
  delete: "removed",
  modify: "modified",
  rename: "renamed",
  copy: "copied",
};

export interface FileDiffProps {
  file: ReviewFileData;
  viewType: RenderableDiffViewMode;
  viewed?: boolean;
  collapsed?: boolean;
  /** changeKey → node, rendered as a full-width row under the matching line. */
  widgets?: Record<string, ReactNode>;
  /** File-level comments rendered at the top of the file (not tied to a line). */
  fileComments?: ReactNode;
  /** Called when a line gutter is clicked, to open a comment composer. */
  onGutterClick?: (file: ReviewFileData, args: ChangeEventArgs) => void;
  /** Called when the AI gutter action is clicked for a specific diff line. */
  onAskLine?: (file: ReviewFileData, args: ChangeEventArgs) => void;
  onToggleViewed?: (file: ReviewFileData) => void;
  onToggleCollapsed?: (file: ReviewFileData) => void;
}

interface GutterRenderOptions {
  change: ChangeData;
  side: "old" | "new";
  renderDefault: () => ReactNode;
}

function hasLineForSide(change: ChangeData, side: "old" | "new"): boolean {
  return side === "old" ? changeOldLine(change) != null : changeNewLine(change) != null;
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function ImagePreviewPanel({ image }: { image: ImageDiffMetadata }) {
  const path = image.preview.preview?.path ?? image.path;
  return (
    <div className="border-b border-border bg-background px-4 py-4">
      <div className="mb-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <Badge variant="outline">{image.mimeType}</Badge>
        <span>{image.previewSide === "old" ? "base image" : "new image"}</span>
        {image.preview.status === "ready" && image.preview.preview.size > 0 && (
          <span>{formatBytes(image.preview.preview.size)}</span>
        )}
      </div>
      {image.preview.status === "loading" ? (
        <div className="rounded-md border border-dashed border-border bg-muted/30 px-4 py-8 text-center text-sm text-muted-foreground">
          Loading image preview...
        </div>
      ) : image.preview.status === "failed" ? (
        <div className="rounded-md border border-dashed border-border bg-muted/30 px-4 py-8 text-center">
          <p className="text-sm font-medium text-foreground">Image preview unavailable</p>
          <p className="mt-1 text-xs text-muted-foreground">{image.preview.error}</p>
        </div>
      ) : image.preview.status === "ready" ? (
        <div className="overflow-auto rounded-md border border-border bg-muted/20 p-3">
          <img
            src={image.preview.preview.dataUrl}
            alt={path}
            className="mx-auto max-h-[70vh] max-w-full object-contain"
            loading="lazy"
            decoding="async"
          />
        </div>
      ) : (
        <div className="rounded-md border border-dashed border-border bg-muted/30 px-4 py-8 text-center text-sm text-muted-foreground">
          Image preview is not loaded yet.
        </div>
      )}
    </div>
  );
}

export function FileDiff({
  file,
  viewType,
  viewed = false,
  collapsed = false,
  widgets,
  fileComments,
  onGutterClick,
  onAskLine,
  onToggleViewed,
  onToggleCollapsed,
}: FileDiffProps) {
  const { additions, deletions } = countReviewFileChanges(file);
  const tokens = useMemo(() => (collapsed ? undefined : tokenizeFile(file)), [file, collapsed]);
  const renderGutter =
    onGutterClick || onAskLine
      ? ({ change, side, renderDefault }: GutterRenderOptions) => {
          const canTargetLine = hasLineForSide(change, side);
          const args = { change, side };
          return (
            <div className="group/gutter flex min-w-[4.5rem] items-center justify-end gap-0.5">
              <button
                type="button"
                className="min-w-6 rounded px-1 text-right hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                disabled={!canTargetLine || !onGutterClick}
                onClick={(event) => {
                  event.stopPropagation();
                  if (canTargetLine) onGutterClick?.(file, args);
                }}
                title="Add PR comment"
                aria-label="Add PR comment"
              >
                {renderDefault()}
              </button>
              {canTargetLine && (
                <span className="inline-flex opacity-0 transition-opacity group-hover/gutter:opacity-100 group-focus-within/gutter:opacity-100">
                  {onGutterClick && (
                    <button
                      type="button"
                      className="inline-flex size-5 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      onClick={(event) => {
                        event.stopPropagation();
                        onGutterClick(file, args);
                      }}
                      title="Add PR comment"
                      aria-label="Add PR comment"
                    >
                      <ChatCircleText size={12} />
                    </button>
                  )}
                  {onAskLine && (
                    <button
                      type="button"
                      className="inline-flex size-5 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      onClick={(event) => {
                        event.stopPropagation();
                        onAskLine(file, args);
                      }}
                      title="Ask AI about this line"
                      aria-label="Ask AI about this line"
                    >
                      <Sparkle size={12} />
                    </button>
                  )}
                </span>
              )}
            </div>
          );
        }
      : undefined;
  const handleToggleViewed = () => {
    onToggleViewed?.(file);
  };

  return (
    <section id={fileAnchorId(file)} className="scroll-mt-10 border-b border-border">
      <header className="sticky top-9 z-10 flex items-center gap-2 border-b border-border bg-secondary px-3 py-1.5 text-xs">
        <button
          type="button"
          onClick={() => onToggleCollapsed?.(file)}
          className="flex min-w-0 items-center gap-1.5 hover:text-foreground"
          aria-expanded={!collapsed}
        >
          {collapsed ? <CaretRight size={12} /> : <CaretDown size={12} />}
          <span className="truncate font-mono">{fileDisplayPath(file)}</span>
        </button>
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <span className="text-[var(--success)]">+{additions}</span>
          <span className="text-destructive">-{deletions}</span>
          <Badge variant="muted">{STATUS_LABEL[file.type] ?? file.type}</Badge>
          {onToggleViewed && (
            <label className="flex cursor-pointer select-none items-center gap-1.5 rounded-md border border-border bg-background px-2 py-0.5 text-[11px] font-medium text-muted-foreground hover:text-foreground">
              <input
                type="checkbox"
                checked={viewed}
                onChange={handleToggleViewed}
                className="size-3 accent-primary"
              />
              Viewed
            </label>
          )}
        </span>
      </header>
      {!collapsed && fileComments}
      {!collapsed && file.imageDiff && <ImagePreviewPanel image={file.imageDiff} />}
      {!collapsed &&
        (file.hunks.length === 0 && !file.imageDiff ? (
          <p className="px-3 py-2 text-xs text-muted-foreground">
            No textual changes. Binary preview is not supported for this file type.
          </p>
        ) : file.hunks.length > 0 ? (
          <Diff
            viewType={viewType}
            diffType={file.type}
            hunks={file.hunks}
            tokens={tokens}
            widgets={widgets}
            renderGutter={renderGutter}
          >
            {(hunks) => hunks.map((hunk) => <Hunk key={hunk.content} hunk={hunk} />)}
          </Diff>
        ) : null)}
    </section>
  );
}
