import type { DraftComment, InlineAnchor, PrComment } from "@/types";
import { CommentThread } from "./CommentThread";

export interface ReviewThreadProps {
  comments: PrComment[];
  replyDraftsByParent: Map<string, DraftComment[]>;
  onAddReply: (rootId: string, anchor: InlineAnchor | null, raw: string) => void;
  activeDraftId?: string | null;
  publishingDraftId?: string | null;
  onFocusDraft?: (localId: string) => void;
  onUpdateDraft?: (localId: string, raw: string) => void;
  onPublishDraft?: (localId: string) => void;
  onRemoveDraft: (localId: string) => void;
}

/** A CommentThread wired for replies: resolves the thread root and its staged replies. */
export function ReviewThread({
  comments,
  replyDraftsByParent,
  onAddReply,
  activeDraftId,
  publishingDraftId,
  onFocusDraft,
  onUpdateDraft,
  onPublishDraft,
  onRemoveDraft,
}: ReviewThreadProps) {
  const root = comments.find((c) => c.parentId == null) ?? comments[0];
  if (!root) return null;

  return (
    <CommentThread
      comments={comments}
      replyDrafts={replyDraftsByParent.get(root.id) ?? []}
      onReply={(raw) => onAddReply(root.id, root.inline, raw)}
      activeDraftId={activeDraftId}
      publishingDraftId={publishingDraftId}
      onFocusDraft={onFocusDraft}
      onUpdateReplyDraft={onUpdateDraft}
      onPublishReplyDraft={onPublishDraft}
      onRemoveReplyDraft={onRemoveDraft}
    />
  );
}
