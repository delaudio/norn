import {
  ChatCircleText,
  CheckCircle,
  CircleNotch,
  GitPullRequest,
  Sparkle,
} from "@phosphor-icons/react";
import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import type { ChangeEventArgs } from "react-diff-view";
import { CommentComposer } from "@/components/comments/CommentComposer";
import { DraftCommentRow } from "@/components/comments/DraftCommentRow";
import { PendingReviewBar } from "@/components/comments/PendingReviewBar";
import { ReviewThread } from "@/components/comments/ReviewThread";
import { DiffViewer } from "@/components/diff/DiffViewer";
import { DiffViewToggle } from "@/components/diff/DiffViewToggle";
import { Markdown } from "@/components/Markdown";
import { ReviewActions } from "@/components/review/ReviewActions";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useBranchStatus } from "@/hooks/useBranchStatus";
import { useBranchSync } from "@/hooks/useBranchSync";
import { useComments } from "@/hooks/useComments";
import { useDiff } from "@/hooks/useDiff";
import type { NewDraft, PublishDraftResult, PublishResult } from "@/hooks/useDraftComments";
import { usePullRequest } from "@/hooks/usePullRequest";
import { useReviewContext } from "@/hooks/useReviewContext";
import type { ReviewReferenceInput } from "@/hooks/useReviewReferences";
import { groupComments } from "@/lib/comments";
import {
  type ChangeData,
  changeKeyForAnchor,
  changeNewLine,
  changeOldLine,
  type FileData,
  fileAnchorId,
  fileDisplayPath,
  fileKey,
  getChangeKey,
} from "@/lib/diff";
import {
  draftCommentAnchorId,
  draftCommentLocationLabel,
  draftCommentLocationTitle,
} from "@/lib/draftComments";
import { formatRelative } from "@/lib/format";
import { extractIssueKeys } from "@/lib/jira";
import { shouldIgnoreShortcut } from "@/lib/keyboard";
import { approvePullRequest } from "@/lib/providerService";
import { loadViewedFiles, saveViewedFiles, viewedFilesStorageKey } from "@/lib/viewedFilesStorage";
import type {
  AiLineQuestionContext,
  AiReviewContext,
  DiffViewMode,
  DraftComment,
  InlineAnchor,
  PrComment,
  PullRequestSummary,
  RepoRef,
  ReviewProvider,
  ReviewReference,
} from "@/types";
import { PrHeader } from "./PrHeader";
import { ReviewReferencesPanel } from "./ReviewReferencesPanel";

export interface PrDetailPanelProps {
  provider: ReviewProvider;
  workspace: string | null;
  repo: string | null;
  prId: number | null;
  currentUserAccountId?: string | null;
  currentUserDisplayName?: string | null;
  defaultViewMode?: DiffViewMode;
  jiraBaseUrl: string | null;
  jiraContextEnabled: boolean;
  availablePullRequests: PullRequestSummary[];
  availableRepositories: RepoRef[];
  reviewReferences: ReviewReference[];
  addReviewReference: (input: ReviewReferenceInput) => void;
  updateReviewReference: (id: string, input: ReviewReferenceInput) => void;
  removeReviewReference: (id: string) => void;
  /** Opens the AI chat panel for this PR. */
  onOpenAiReview?: () => void;
  /** Resolve merge conflicts for this PR branch using Claude and show output in the AI panel. */
  onResolveBranchConflicts?: (
    sourceBranch: string,
    destinationBranch: string,
    tips: string,
  ) => Promise<void>;
  /** Latest PR context used to rebuild review/fix payloads in sibling panels. */
  onAiReviewContextChange?: (context: AiReviewContext | null) => void;
  /** Ask the selected AI provider a focused question about a changed line. */
  onAskAiLine?: (context: AiLineQuestionContext, question: string) => void;
  drafts: DraftComment[];
  publishing: boolean;
  publishingDraftId: string | null;
  addDraft: (draft: NewDraft) => void;
  updateDraft: (localId: string, patch: { raw?: string }) => void;
  removeDraft: (localId: string) => void;
  discardAll: () => void;
  publishDraft: (localId: string) => Promise<PublishDraftResult>;
  publishAll: () => Promise<PublishResult>;
}

interface ComposerTarget {
  fileKeyStr: string;
  changeKey: string;
  path: string;
  to: number | null;
  from: number | null;
}

interface LineAskTarget {
  context: AiLineQuestionContext;
  label: string;
}

interface ConversationThreadItem {
  id: string;
  sortValue: number;
  path: string | null;
  lineLabel: string;
  snippet: string | null;
  comments: PrComment[] | null;
  draft: DraftComment | null;
}

function aiLineQuestionLabel(context: Pick<AiLineQuestionContext, "path" | "to" | "from">): string {
  const line = context.to ?? context.from;
  return line == null ? context.path : `${context.path}:${line}`;
}

function anchorForDiffLine(
  file: FileData,
  args: ChangeEventArgs,
): {
  change: ChangeData;
  path: string;
  to: number | null;
  from: number | null;
  side: "old" | "new";
} | null {
  const change = args.change as ChangeData | null;
  if (!change) return null;
  const side = args.side === "old" ? "old" : "new";
  let path: string;
  let to: number | null = null;
  let from: number | null = null;
  if (side === "old") {
    path = file.oldPath || file.newPath;
    from = changeOldLine(change) ?? null;
  } else {
    path = file.newPath || file.oldPath;
    to = changeNewLine(change) ?? null;
  }
  if (to == null && from == null) return null;
  return { change, path, to, from, side };
}

function cycleViewMode(mode: DiffViewMode): DiffViewMode {
  if (mode === "unified") return "split";
  if (mode === "split") return "conversation";
  return "unified";
}

function commentSortValue(value: string): number {
  const parsed = new Date(value).getTime();
  return Number.isNaN(parsed) ? 0 : parsed;
}

function groupThreads(comments: PrComment[]): PrComment[][] {
  const roots: PrComment[] = [];
  const repliesByParent = new Map<number, PrComment[]>();
  const seenRootIds = new Set<number>();

  for (const comment of comments) {
    if (comment.parentId == null) {
      roots.push(comment);
      seenRootIds.add(comment.id);
      continue;
    }
    const replies = repliesByParent.get(comment.parentId) ?? [];
    replies.push(comment);
    repliesByParent.set(comment.parentId, replies);
  }

  const threads = roots.map((root) => [
    root,
    ...(repliesByParent.get(root.id) ?? []).sort((a, b) => a.createdOn.localeCompare(b.createdOn)),
  ]);

  for (const comment of comments) {
    if (comment.parentId == null || seenRootIds.has(comment.parentId)) continue;
    threads.push([comment]);
  }

  return threads.sort((a, b) => a[0].createdOn.localeCompare(b[0].createdOn));
}

function changeContent(change: ChangeData): string {
  const content = "content" in change && typeof change.content === "string" ? change.content : "";
  return content.replace(/^[+\-\s]/, "").trimEnd();
}

function findChangeForAnchor(
  files: FileData[],
  path: string,
  to: number | null,
  from: number | null,
): ChangeData | null {
  for (const file of files) {
    if (file.newPath !== path && file.oldPath !== path) continue;
    for (const hunk of file.hunks) {
      for (const change of hunk.changes) {
        if (to != null && changeNewLine(change) === to) return change;
        if (from != null && changeOldLine(change) === from) return change;
      }
    }
  }
  return null;
}

function lineLabel(anchor: InlineAnchor | null): string {
  if (!anchor) return "General PR conversation";
  if (anchor.to != null) return `line ${anchor.to}`;
  if (anchor.from != null) return `old line ${anchor.from}`;
  return "file-level or outdated comment";
}

function snippetForAnchor(files: FileData[], anchor: InlineAnchor | null): string | null {
  if (!anchor?.path) return null;
  const change = findChangeForAnchor(files, anchor.path, anchor.to, anchor.from);
  return change ? changeContent(change) : null;
}

function conversationItems(
  files: FileData[],
  grouped: ReturnType<typeof groupComments>,
  drafts: DraftComment[],
): ConversationThreadItem[] {
  const items: ConversationThreadItem[] = [];
  const fileByKey = new Map<string, FileData>();
  const fileByPath = new Map<string, FileData>();
  for (const file of files) {
    fileByKey.set(fileKey(file), file);
    if (file.newPath) fileByPath.set(file.newPath, file);
    if (file.oldPath) fileByPath.set(file.oldPath, file);
  }

  for (const [fileKeyStr, byChange] of Object.entries(grouped.inlineByFile)) {
    for (const [changeKey, comments] of Object.entries(byChange)) {
      for (const thread of groupThreads(comments)) {
        const root = thread[0];
        items.push({
          id: `comment:${root.id}`,
          sortValue: commentSortValue(root.createdOn),
          path: root.inline?.path ?? null,
          lineLabel: lineLabel(root.inline),
          snippet: snippetForAnchor(files, root.inline),
          comments: thread,
          draft: null,
        });
      }
      const inlineDrafts = drafts.filter((draft) => {
        if (draft.parentId != null) return false;
        const file = fileByPath.get(draft.path);
        if (!file || fileKey(file) !== fileKeyStr) return false;
        return changeKeyForAnchor(file, draft.to, draft.from) === changeKey;
      });
      for (const draft of inlineDrafts) {
        items.push({
          id: `draft:${draft.localId}`,
          sortValue: draft.createdAt,
          path: draft.path,
          lineLabel: lineLabel({ path: draft.path, to: draft.to, from: draft.from }),
          snippet: snippetForAnchor(files, { path: draft.path, to: draft.to, from: draft.from }),
          comments: null,
          draft,
        });
      }
    }
  }

  for (const [fileKeyStr, comments] of Object.entries(grouped.fileLevelByFile)) {
    const file = fileByKey.get(fileKeyStr);
    const path = file ? fileDisplayPath(file) : null;
    for (const thread of groupThreads(comments)) {
      const root = thread[0];
      items.push({
        id: `file-comment:${root.id}`,
        sortValue: commentSortValue(root.createdOn),
        path: root.inline?.path ?? path,
        lineLabel: "file-level or outdated comment",
        snippet: null,
        comments: thread,
        draft: null,
      });
    }
  }

  for (const thread of groupThreads(grouped.unanchored)) {
    const root = thread[0];
    items.push({
      id: `general-comment:${root.id}`,
      sortValue: commentSortValue(root.createdOn),
      path: null,
      lineLabel: "General PR conversation",
      snippet: null,
      comments: thread,
      draft: null,
    });
  }

  for (const draft of drafts) {
    if (draft.parentId != null) continue;
    const alreadyIncluded = items.some((item) => item.draft?.localId === draft.localId);
    if (alreadyIncluded) continue;
    items.push({
      id: `draft:${draft.localId}`,
      sortValue: draft.createdAt,
      path: draft.path || null,
      lineLabel: draft.path
        ? lineLabel({ path: draft.path, to: draft.to, from: draft.from })
        : "Draft comment",
      snippet: snippetForAnchor(files, { path: draft.path, to: draft.to, from: draft.from }),
      comments: null,
      draft,
    });
  }

  return items.sort((a, b) => a.sortValue - b.sortValue);
}

interface ConversationReviewViewProps {
  files: FileData[];
  grouped: ReturnType<typeof groupComments>;
  drafts: DraftComment[];
  viewMode: DiffViewMode;
  onViewModeChange: (mode: DiffViewMode) => void;
  replyDraftsByParent: Map<number, DraftComment[]>;
  onAddReply: (rootId: number, anchor: InlineAnchor | null, raw: string) => void;
  activeDraftId: string | null;
  publishingDraftId: string | null;
  onFocusDraft: (localId: string) => void;
  onUpdateDraft: (localId: string, raw: string) => void;
  onPublishDraft: (localId: string) => void;
  onRemoveDraft: (localId: string) => void;
}

function ConversationReviewView({
  files,
  grouped,
  drafts,
  viewMode,
  onViewModeChange,
  replyDraftsByParent,
  onAddReply,
  activeDraftId,
  publishingDraftId,
  onFocusDraft,
  onUpdateDraft,
  onPublishDraft,
  onRemoveDraft,
}: ConversationReviewViewProps) {
  const items = useMemo(() => conversationItems(files, grouped, drafts), [files, grouped, drafts]);
  const inlineCount = items.filter((item) => item.path).length;

  return (
    <div>
      <div className="sticky top-0 z-20 border-b border-border bg-background">
        <div className="flex items-center gap-3 border-t border-border px-4 py-2 text-xs">
          <div className="flex items-center gap-1 font-medium">
            <ChatCircleText size={14} />
            {items.length} conversation thread{items.length === 1 ? "" : "s"}
          </div>
          <span className="text-muted-foreground">
            {inlineCount} anchored to changed line{inlineCount === 1 ? "" : "s"}
          </span>
          <div className="ml-auto">
            <DiffViewToggle value={viewMode} onChange={onViewModeChange} />
          </div>
        </div>
      </div>

      {items.length === 0 ? (
        <div className="flex min-h-60 flex-col items-center justify-center gap-2 p-8 text-center text-sm text-muted-foreground">
          <ChatCircleText size={36} weight="thin" />
          <p>No review conversation yet.</p>
        </div>
      ) : (
        <div className="mx-auto flex max-w-5xl flex-col gap-3 p-4">
          {items.map((item) => (
            <section
              key={item.id}
              className="overflow-hidden rounded-lg border border-border bg-card"
            >
              <header className="border-b border-border bg-muted/30 px-3 py-2">
                <div className="flex flex-wrap items-center gap-2 text-xs">
                  <span className="font-mono text-foreground">
                    {item.path ? `${item.path}:${item.lineLabel}` : item.lineLabel}
                  </span>
                  {item.draft && (
                    <span className="rounded-full border border-primary/30 bg-primary/10 px-2 py-0.5 text-primary">
                      Pending
                    </span>
                  )}
                </div>
                {item.snippet && (
                  <pre className="mt-2 overflow-x-auto rounded-md border border-border bg-background px-3 py-2 font-mono text-xs text-foreground">
                    {item.snippet}
                  </pre>
                )}
              </header>
              {item.comments && (
                <ReviewThread
                  comments={item.comments}
                  replyDraftsByParent={replyDraftsByParent}
                  onAddReply={onAddReply}
                  activeDraftId={activeDraftId}
                  publishingDraftId={publishingDraftId}
                  onFocusDraft={onFocusDraft}
                  onUpdateDraft={onUpdateDraft}
                  onPublishDraft={onPublishDraft}
                  onRemoveDraft={onRemoveDraft}
                />
              )}
              {item.draft && (
                <ConversationDraftRow
                  draft={item.draft}
                  activeDraftId={activeDraftId}
                  publishingDraftId={publishingDraftId}
                  onFocusDraft={onFocusDraft}
                  onUpdateDraft={onUpdateDraft}
                  onPublishDraft={onPublishDraft}
                  onRemoveDraft={onRemoveDraft}
                />
              )}
            </section>
          ))}
        </div>
      )}
    </div>
  );
}

function ConversationDraftRow({
  draft,
  activeDraftId,
  publishingDraftId,
  onFocusDraft,
  onUpdateDraft,
  onPublishDraft,
  onRemoveDraft,
}: {
  draft: DraftComment;
  activeDraftId: string | null;
  publishingDraftId: string | null;
  onFocusDraft: (localId: string) => void;
  onUpdateDraft: (localId: string, raw: string) => void;
  onPublishDraft: (localId: string) => void;
  onRemoveDraft: (localId: string) => void;
}) {
  return (
    <div className="p-3">
      <DraftCommentRow
        draft={draft}
        active={draft.localId === activeDraftId}
        publishing={draft.localId === publishingDraftId}
        onFocus={() => onFocusDraft(draft.localId)}
        onUpdate={(raw) => onUpdateDraft(draft.localId, raw)}
        onPublish={() => onPublishDraft(draft.localId)}
        onRemove={() => onRemoveDraft(draft.localId)}
      />
    </div>
  );
}

interface ConflictResolutionActionsProps {
  sourceBranch: string;
  destinationBranch: string;
  onResolveBranchConflicts: (
    sourceBranch: string,
    destinationBranch: string,
    tips: string,
  ) => Promise<void>;
}

function ConflictResolutionActions({
  sourceBranch,
  destinationBranch,
  onResolveBranchConflicts,
}: ConflictResolutionActionsProps) {
  const [tips, setTips] = useState("");

  return (
    <div className="mt-3 space-y-2">
      <label
        htmlFor="conflict-resolution-tips"
        className="block text-[11px] font-medium text-foreground"
      >
        Tips for AI resolution
      </label>
      <textarea
        id="conflict-resolution-tips"
        value={tips}
        onChange={(event) => setTips(event.target.value)}
        rows={3}
        placeholder="Optional: prefer destination branch for analytics filters, keep new API typing, preserve existing tests..."
        className="w-full resize-y rounded-md border border-input bg-background px-2 py-1.5 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
      <p className="text-[11px] text-muted-foreground">
        These instructions are appended to the prompt sent to Claude.
      </p>
      <Button
        size="sm"
        onClick={() => void onResolveBranchConflicts(sourceBranch, destinationBranch, tips.trim())}
      >
        Resolve with AI
      </Button>
    </div>
  );
}

interface ApprovePullRequestButtonProps {
  provider: ReviewProvider;
  workspace: string;
  repo: string;
  prId: number;
  approved: boolean;
  onApproved: () => Promise<void>;
}

function ApprovePullRequestButton({
  workspace,
  provider,
  repo,
  prId,
  approved,
  onApproved,
}: ApprovePullRequestButtonProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleApprove = async () => {
    if (busy || approved) return;
    setBusy(true);
    setError(null);
    try {
      await approvePullRequest({ provider, workspace, repo, id: prId });
      await onApproved();
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col items-end gap-1">
      <Button
        size="sm"
        variant="ghost"
        onClick={() => void handleApprove()}
        disabled={busy || approved}
        className="h-9 rounded-full border border-[#10b981] bg-[#10b981]/10 px-3 text-[13px] font-bold text-[#10b981] hover:bg-[#10b981]/15 hover:text-[#10b981] disabled:opacity-70"
        title={
          approved
            ? "You have already approved this pull request"
            : `Approve this pull request on ${provider === "github" ? "GitHub" : "Bitbucket"}`
        }
      >
        {busy ? <CircleNotch size={14} className="animate-spin" /> : <CheckCircle size={14} />}
        {approved ? "Approved" : busy ? "Approving…" : "Approve"}
      </Button>
      {error && (
        <span className="max-w-56 truncate text-[11px] text-destructive" title={error}>
          Approval failed
        </span>
      )}
    </div>
  );
}

/** Loads + renders a PR's header, diff, existing comments, and the review composer. */
export function PrDetailPanel({
  provider,
  workspace,
  repo,
  prId,
  currentUserAccountId,
  currentUserDisplayName,
  defaultViewMode = "unified",
  jiraBaseUrl,
  jiraContextEnabled,
  availablePullRequests,
  availableRepositories,
  reviewReferences,
  addReviewReference,
  updateReviewReference,
  removeReviewReference,
  onOpenAiReview,
  onResolveBranchConflicts,
  onAiReviewContextChange,
  onAskAiLine,
  drafts,
  publishing,
  publishingDraftId,
  addDraft,
  updateDraft,
  removeDraft,
  discardAll,
  publishDraft,
  publishAll,
}: PrDetailPanelProps) {
  const {
    pr,
    loading,
    error,
    refresh: refreshPullRequest,
  } = usePullRequest(provider, workspace, repo, prId);
  const {
    files,
    raw: rawDiff,
    loading: diffLoading,
    error: diffError,
  } = useDiff(provider, workspace, repo, prId);
  const { comments, refresh: refreshComments } = useComments(provider, workspace, repo, prId);
  const { status: branchStatus, refresh: refreshBranchStatus } = useBranchStatus(
    provider,
    workspace,
    repo,
    pr?.sourceBranch ?? null,
    pr?.destinationBranch ?? null,
  );
  const {
    loading: branchSyncLoading,
    result: branchSyncResult,
    error: branchSyncError,
    sync: syncBranch,
    clear: clearBranchSync,
  } = useBranchSync(workspace, repo, prId);
  const jiraKeys = useMemo(() => (pr ? extractIssueKeys(pr.sourceBranch, pr.title) : []), [pr]);
  const jiraContext = useReviewContext(jiraKeys, jiraContextEnabled);
  const [viewMode, setViewMode] = useState<DiffViewMode>(defaultViewMode);
  const [composer, setComposer] = useState<ComposerTarget | null>(null);
  const [lineAskTarget, setLineAskTarget] = useState<LineAskTarget | null>(null);
  const [lineAskQuestion, setLineAskQuestion] = useState("");
  const [publishError, setPublishError] = useState<string | null>(null);
  const [activeDraftId, setActiveDraftId] = useState<string | null>(null);
  const [, setFileIndex] = useState(0);
  const viewedStorageKey = viewedFilesStorageKey(workspace, repo, prId);
  const [viewedFileKeys, setViewedFileKeys] = useState<Set<string>>(() =>
    loadViewedFiles(viewedStorageKey),
  );

  useEffect(() => {
    setViewMode(defaultViewMode);
  }, [defaultViewMode]);

  if (drafts.length === 0 && activeDraftId !== null) {
    setActiveDraftId(null);
  } else if (drafts.length > 0 && !drafts.some((draft) => draft.localId === activeDraftId)) {
    setActiveDraftId(drafts[0]?.localId ?? null);
  }

  // Reset transient view state when switching PRs / repos (deps are intentional triggers).
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset when the selected PR/repo changes
  useEffect(() => {
    setComposer(null);
    setLineAskTarget(null);
    setLineAskQuestion("");
    setPublishError(null);
    setFileIndex(0);
    setViewedFileKeys(loadViewedFiles(viewedStorageKey));
    clearBranchSync();
  }, [workspace, repo, prId, viewedStorageKey, clearBranchSync]);

  useEffect(() => {
    setViewedFileKeys((prev) => {
      const currentFileKeys = new Set(files.map(fileKey));
      const next = new Set([...prev].filter((key) => currentFileKeys.has(key)));
      if (next.size === prev.size) return prev;
      saveViewedFiles(viewedStorageKey, next);
      return next;
    });
  }, [files, viewedStorageKey]);

  useEffect(() => {
    if (!workspace || !repo || !pr) {
      onAiReviewContextChange?.(null);
      return;
    }
    onAiReviewContextChange?.({
      workspace,
      repo,
      pr,
      branchStatus,
      rawDiff,
      jiraKeys,
      jiraBaseUrl,
      jiraContext,
    });
  }, [
    workspace,
    repo,
    pr,
    branchStatus,
    rawDiff,
    jiraKeys,
    jiraBaseUrl,
    jiraContext,
    onAiReviewContextChange,
  ]);

  // Keyboard: u = toggle diff view, ] / [ = next / previous changed file.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (shouldIgnoreShortcut(e)) return;
      if (e.key === "u") {
        e.preventDefault();
        setViewMode(cycleViewMode);
        return;
      }
      if ((e.key === "]" || e.key === "[") && files.length > 0) {
        e.preventDefault();
        setFileIndex((prev) => {
          const next = e.key === "]" ? Math.min(prev + 1, files.length - 1) : Math.max(prev - 1, 0);
          const target = files[next];
          if (target) {
            document
              .getElementById(fileAnchorId(target))
              ?.scrollIntoView({ behavior: "smooth", block: "start" });
          }
          return next;
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [files]);

  const grouped = useMemo(() => groupComments(files, comments), [files, comments]);

  const handleGutterClick = useCallback((file: FileData, args: ChangeEventArgs) => {
    const anchor = anchorForDiffLine(file, args);
    if (!anchor) return;
    setComposer({
      fileKeyStr: fileKey(file),
      changeKey: getChangeKey(anchor.change),
      path: anchor.path,
      to: anchor.to,
      from: anchor.from,
    });
  }, []);

  const handleAskLine = useCallback((file: FileData, args: ChangeEventArgs) => {
    const anchor = anchorForDiffLine(file, args);
    if (!anchor) return;
    const context: AiLineQuestionContext = {
      path: anchor.path,
      side: anchor.side,
      to: anchor.to,
      from: anchor.from,
      lineText: anchor.change.content,
    };
    setLineAskTarget({ context, label: aiLineQuestionLabel(context) });
    setLineAskQuestion("");
  }, []);

  const handleToggleFileViewed = useCallback(
    (file: FileData) => {
      const key = fileKey(file);
      setViewedFileKeys((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        saveViewedFiles(viewedStorageKey, next);
        return next;
      });
    },
    [viewedStorageKey],
  );

  const replyDraftsByParent = useMemo(() => {
    const map = new Map<number, DraftComment[]>();
    for (const d of drafts) {
      if (d.parentId == null) continue;
      const arr = map.get(d.parentId) ?? [];
      arr.push(d);
      map.set(d.parentId, arr);
    }
    return map;
  }, [drafts]);

  const addReply = useCallback(
    (rootId: number, anchor: InlineAnchor | null, raw: string) => {
      addDraft({
        parentId: rootId,
        path: anchor?.path ?? "",
        to: anchor?.to ?? null,
        from: anchor?.from ?? null,
        raw,
      });
    },
    [addDraft],
  );

  const focusDraft = useCallback((localId: string) => {
    setActiveDraftId(localId);
    window.requestAnimationFrame(() => {
      document
        .getElementById(draftCommentAnchorId(localId))
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    });
  }, []);

  const handleUpdateDraft = useCallback(
    (localId: string, raw: string) => {
      updateDraft(localId, { raw });
    },
    [updateDraft],
  );

  const handlePublishDraft = useCallback(
    async (localId: string) => {
      setPublishError(null);
      const result = await publishDraft(localId);
      if (result.error) {
        setPublishError(result.error);
        return;
      }
      await refreshComments();
    },
    [publishDraft, refreshComments],
  );

  const pendingReviewItems = useMemo(
    () =>
      drafts.map((draft) => ({
        id: draft.localId,
        label: draftCommentLocationLabel(draft),
        title: draftCommentLocationTitle(draft),
      })),
    [drafts],
  );

  const handleSelectPreviousDraft = useCallback(() => {
    if (drafts.length <= 1) return;
    const currentIndex = drafts.findIndex((draft) => draft.localId === activeDraftId);
    const nextIndex = currentIndex <= 0 ? drafts.length - 1 : currentIndex - 1;
    const nextDraft = drafts[nextIndex];
    if (nextDraft) focusDraft(nextDraft.localId);
  }, [drafts, activeDraftId, focusDraft]);

  const handleSelectNextDraft = useCallback(() => {
    if (drafts.length <= 1) return;
    const currentIndex = drafts.findIndex((draft) => draft.localId === activeDraftId);
    const nextIndex = currentIndex < 0 || currentIndex === drafts.length - 1 ? 0 : currentIndex + 1;
    const nextDraft = drafts[nextIndex];
    if (nextDraft) focusDraft(nextDraft.localId);
  }, [drafts, activeDraftId, focusDraft]);

  const widgetsByFile = useMemo(() => {
    interface Slot {
      existing?: typeof comments;
      drafts: typeof drafts;
      composer: boolean;
    }
    const slots: Record<string, Record<string, Slot>> = {};
    const ensure = (fk: string, ck: string): Slot => {
      const byKey = slots[fk] ?? {};
      slots[fk] = byKey;
      const slot = byKey[ck] ?? { drafts: [], composer: false };
      byKey[ck] = slot;
      return slot;
    };

    for (const [fk, byKey] of Object.entries(grouped.inlineByFile)) {
      for (const [ck, thread] of Object.entries(byKey)) {
        ensure(fk, ck).existing = thread;
      }
    }

    const fileByPath = new Map<string, FileData>();
    for (const file of files) {
      if (file.newPath) fileByPath.set(file.newPath, file);
      if (file.oldPath) fileByPath.set(file.oldPath, file);
    }
    for (const draft of drafts) {
      if (draft.parentId != null) continue; // replies render inside their thread
      const file = fileByPath.get(draft.path);
      if (!file) continue;
      const ck = changeKeyForAnchor(file, draft.to, draft.from);
      if (!ck) continue;
      ensure(fileKey(file), ck).drafts.push(draft);
    }

    if (composer) {
      ensure(composer.fileKeyStr, composer.changeKey).composer = true;
    }

    const out: Record<string, Record<string, ReactNode>> = {};
    for (const [fk, byKey] of Object.entries(slots)) {
      out[fk] = {};
      for (const [ck, slot] of Object.entries(byKey)) {
        out[fk][ck] = (
          <div>
            {slot.existing && (
              <ReviewThread
                comments={slot.existing}
                replyDraftsByParent={replyDraftsByParent}
                onAddReply={addReply}
                activeDraftId={activeDraftId}
                publishingDraftId={publishingDraftId}
                onFocusDraft={focusDraft}
                onUpdateDraft={handleUpdateDraft}
                onPublishDraft={handlePublishDraft}
                onRemoveDraft={removeDraft}
              />
            )}
            {slot.drafts.map((d) => (
              <DraftCommentRow
                key={d.localId}
                draft={d}
                active={d.localId === activeDraftId}
                publishing={d.localId === publishingDraftId}
                onFocus={() => focusDraft(d.localId)}
                onUpdate={(raw) => handleUpdateDraft(d.localId, raw)}
                onPublish={() => void handlePublishDraft(d.localId)}
                onRemove={() => removeDraft(d.localId)}
              />
            ))}
            {slot.composer && composer && (
              <CommentComposer
                autoFocus
                onCancel={() => setComposer(null)}
                onSubmit={(raw) => {
                  addDraft({
                    path: composer.path,
                    to: composer.to,
                    from: composer.from,
                    raw,
                    parentId: null,
                  });
                  setComposer(null);
                }}
              />
            )}
          </div>
        );
      }
    }
    return out;
  }, [
    grouped,
    drafts,
    composer,
    files,
    addDraft,
    removeDraft,
    replyDraftsByParent,
    addReply,
    activeDraftId,
    publishingDraftId,
    focusDraft,
    handleUpdateDraft,
    handlePublishDraft,
  ]);

  const fileWidgets = useMemo(() => {
    const out: Record<string, ReactNode> = {};
    for (const [fk, list] of Object.entries(grouped.fileLevelByFile)) {
      out[fk] = (
        <div className="border-b border-border">
          <div className="bg-muted/30 px-3 pt-1.5 font-sans text-[11px] text-muted-foreground">
            Comments on this file
          </div>
          <ReviewThread
            comments={list}
            replyDraftsByParent={replyDraftsByParent}
            onAddReply={addReply}
            activeDraftId={activeDraftId}
            publishingDraftId={publishingDraftId}
            onFocusDraft={focusDraft}
            onUpdateDraft={handleUpdateDraft}
            onPublishDraft={handlePublishDraft}
            onRemoveDraft={removeDraft}
          />
        </div>
      );
    }
    return out;
  }, [
    grouped,
    replyDraftsByParent,
    addReply,
    activeDraftId,
    publishingDraftId,
    focusDraft,
    handleUpdateDraft,
    handlePublishDraft,
    removeDraft,
  ]);

  const handlePublishAll = useCallback(async () => {
    setPublishError(null);
    const res = await publishAll();
    await refreshComments();
    if (res.failed.length > 0) {
      setPublishError(`${res.failed.length} comment(s) failed to publish: ${res.failed[0].error}`);
    }
  }, [publishAll, refreshComments]);

  const handleSyncBranch = useCallback(async () => {
    if (!pr) return;
    const result = await syncBranch(pr.sourceBranch, pr.destinationBranch);
    if (result?.status === "success") {
      await refreshBranchStatus();
    }
  }, [pr, refreshBranchStatus, syncBranch]);

  const handleSubmitLineAsk = useCallback(() => {
    const question = lineAskQuestion.trim();
    if (!lineAskTarget || !question) return;
    onAskAiLine?.(lineAskTarget.context, question);
    setLineAskTarget(null);
    setLineAskQuestion("");
  }, [lineAskQuestion, lineAskTarget, onAskAiLine]);

  const normalizedCurrentUserName = currentUserDisplayName?.trim().toLowerCase() ?? null;
  const currentUserApproved =
    pr?.reviewers.some((reviewer) => {
      if (!reviewer.approved) return false;
      if (currentUserAccountId && reviewer.accountId === currentUserAccountId) return true;
      if (!normalizedCurrentUserName) return false;
      return reviewer.displayName.trim().toLowerCase() === normalizedCurrentUserName;
    }) ?? false;

  const canApprove = Boolean(workspace && repo && pr && pr.state === "OPEN");

  if (prId === null) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
        <GitPullRequest size={48} weight="thin" />
        <p className="text-sm">Select a pull request to review</p>
      </div>
    );
  }

  if (loading) {
    return <div className="p-6 text-sm text-muted-foreground">Loading pull request #{prId}…</div>;
  }

  if (error) {
    return <div className="p-6 text-sm text-destructive">{error}</div>;
  }

  if (!pr) {
    return null;
  }

  return (
    <div className="flex h-full flex-col">
      <div className="min-h-0 flex-1 overflow-auto">
        <PrHeader
          pr={pr}
          branchStatus={branchStatus}
          jiraKeys={jiraKeys}
          jiraBaseUrl={jiraBaseUrl}
          onSyncBranch={branchStatus?.behind ? handleSyncBranch : undefined}
          syncBusy={branchSyncLoading}
          htmlUrl={
            workspace && repo
              ? provider === "github"
                ? `https://github.com/${workspace}/${repo}/pull/${pr.id}`
                : `https://bitbucket.org/${workspace}/${repo}/pull-requests/${pr.id}`
              : null
          }
          actions={
            workspace && repo ? (
              <>
                {canApprove && workspace && repo && (
                  <ApprovePullRequestButton
                    provider={provider}
                    workspace={workspace}
                    repo={repo}
                    prId={pr.id}
                    approved={Boolean(currentUserApproved)}
                    onApproved={async () => {
                      await refreshPullRequest();
                    }}
                  />
                )}
                <ReviewActions workspace={workspace} repo={repo} onOpenAiReview={onOpenAiReview} />
              </>
            ) : undefined
          }
        />
        <ReviewReferencesPanel
          jiraKeys={jiraKeys}
          jiraBaseUrl={jiraBaseUrl}
          availablePullRequests={availablePullRequests}
          availableRepositories={availableRepositories}
          references={reviewReferences}
          onAddReference={addReviewReference}
          onUpdateReference={updateReviewReference}
          onRemoveReference={removeReviewReference}
        />
        {branchSyncError && (
          <div className="border-b border-destructive/30 bg-destructive/10 px-6 py-2 text-xs text-destructive">
            {branchSyncError}
          </div>
        )}
        {branchSyncResult && !branchSyncError && (
          <div className="border-b border-border bg-muted/30 px-6 py-2 text-xs text-muted-foreground">
            <span className="font-medium text-foreground">{branchSyncResult.summary}</span>
            {branchSyncResult.status === "conflict" &&
              branchSyncResult.conflictFiles.length > 0 && (
                <div className="mt-2 rounded-md border border-[var(--warning)]/30 bg-[var(--warning)]/8 px-3 py-2">
                  <p className="font-medium text-[var(--warning)]">Conflicts detected</p>
                  <ul className="mt-1 space-y-1">
                    {branchSyncResult.conflictFiles.map((file) => (
                      <li key={file} className="font-mono text-[11px] text-foreground">
                        {file}
                      </li>
                    ))}
                  </ul>
                  {onResolveBranchConflicts && (
                    <ConflictResolutionActions
                      key={`${pr.id}:${branchSyncResult.sourceBranch}:${branchSyncResult.destinationBranch}`}
                      sourceBranch={pr.sourceBranch}
                      destinationBranch={pr.destinationBranch}
                      onResolveBranchConflicts={onResolveBranchConflicts}
                    />
                  )}
                </div>
              )}
            {branchSyncResult.warning && (
              <div className="mt-1 text-[var(--warning)]">{branchSyncResult.warning}</div>
            )}
            {branchSyncResult.logs.length > 0 && (
              <details className="mt-1">
                <summary className="cursor-pointer select-none">Sync log</summary>
                <pre className="mt-1 overflow-x-auto whitespace-pre-wrap font-mono text-[11px]">
                  {branchSyncResult.logs.join("\n")}
                </pre>
              </details>
            )}
          </div>
        )}
        {viewMode !== "conversation" && grouped.unanchored.length > 0 && (
          <details className="border-b border-border px-6 py-2 text-sm">
            <summary className="cursor-pointer text-muted-foreground">
              Conversation ({grouped.unanchored.length})
            </summary>
            <ul className="mt-2 flex flex-col gap-2">
              {grouped.unanchored.map((c) => (
                <li key={c.id}>
                  <span className="font-medium">{c.userDisplayName}</span>{" "}
                  <span className="text-xs text-muted-foreground">
                    {formatRelative(c.createdOn)}
                  </span>
                  <Markdown className="text-foreground/90">{c.contentRaw}</Markdown>
                </li>
              ))}
            </ul>
          </details>
        )}
        {viewMode === "conversation" ? (
          <ConversationReviewView
            files={files}
            grouped={grouped}
            drafts={drafts}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            replyDraftsByParent={replyDraftsByParent}
            onAddReply={addReply}
            activeDraftId={activeDraftId}
            publishingDraftId={publishingDraftId}
            onFocusDraft={focusDraft}
            onUpdateDraft={handleUpdateDraft}
            onPublishDraft={(localId) => void handlePublishDraft(localId)}
            onRemoveDraft={removeDraft}
          />
        ) : (
          <DiffViewer
            files={files}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            loading={diffLoading}
            error={diffError}
            widgetsByFile={widgetsByFile}
            fileWidgets={fileWidgets}
            viewedFileKeys={viewedFileKeys}
            onToggleFileViewed={handleToggleFileViewed}
            onGutterClick={handleGutterClick}
            onAskLine={onAskAiLine ? handleAskLine : undefined}
          />
        )}
      </div>
      <Dialog
        open={lineAskTarget != null}
        onOpenChange={(open) => {
          if (open) return;
          setLineAskTarget(null);
          setLineAskQuestion("");
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Ask AI about this line</DialogTitle>
            <DialogDescription>
              {lineAskTarget
                ? `The question will include context for ${lineAskTarget.label}.`
                : "The question will include the selected diff line."}
            </DialogDescription>
          </DialogHeader>
          {lineAskTarget && (
            <div className="rounded-md border border-border bg-muted/30 px-3 py-2">
              <div className="font-mono text-xs text-muted-foreground">{lineAskTarget.label}</div>
              <pre className="mt-1 whitespace-pre-wrap font-mono text-xs text-foreground">
                {lineAskTarget.context.lineText}
              </pre>
            </div>
          )}
          <textarea
            autoFocus
            value={lineAskQuestion}
            onChange={(event) => setLineAskQuestion(event.target.value)}
            rows={4}
            placeholder="Ask what this line does, whether it is safe, or how it relates to the PR..."
            className="w-full resize-y rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                event.preventDefault();
                handleSubmitLineAsk();
              }
            }}
          />
          <DialogFooter>
            <Button
              variant="ghost"
              onClick={() => {
                setLineAskTarget(null);
                setLineAskQuestion("");
              }}
            >
              Cancel
            </Button>
            <Button onClick={handleSubmitLineAsk} disabled={!lineAskQuestion.trim()}>
              <Sparkle size={14} />
              Ask AI
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {publishError && (
        <div className="shrink-0 border-t border-border bg-destructive/10 px-4 py-2 text-xs text-destructive">
          {publishError}
        </div>
      )}
      <PendingReviewBar
        items={pendingReviewItems}
        activeDraftId={activeDraftId}
        publishing={publishing}
        onSelectDraft={focusDraft}
        onSelectPreviousDraft={handleSelectPreviousDraft}
        onSelectNextDraft={handleSelectNextDraft}
        onPublishAll={handlePublishAll}
        onDiscardAll={discardAll}
      />
    </div>
  );
}
