import type { DraftComment } from "@/types";

function basename(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

export function draftCommentAnchorId(localId: string): string {
  return `draft-comment-${localId.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

export function draftCommentLocationLabel(
  draft: Pick<DraftComment, "path" | "to" | "from" | "parentId">,
): string {
  const line = draft.to ?? draft.from;
  const fileLabel = draft.path ? basename(draft.path) : null;

  if (draft.parentId != null) {
    if (fileLabel && line != null) return `Reply on ${fileLabel}:${line}`;
    if (fileLabel) return `Reply on ${fileLabel}`;
    return "Reply draft";
  }

  if (fileLabel && line != null) return `${fileLabel}:${line}`;
  if (fileLabel) return fileLabel;
  if (line != null) return `Line ${line}`;
  return "Draft comment";
}

export function draftCommentLocationTitle(
  draft: Pick<DraftComment, "path" | "to" | "from" | "parentId">,
): string {
  const line = draft.to ?? draft.from;
  if (!draft.path) {
    return draft.parentId != null ? "Reply draft" : "Draft comment";
  }
  if (line == null) return draft.path;
  return `${draft.path}:${line}`;
}
