import {
  ArrowsClockwise,
  ArrowsIn,
  ArrowsOut,
  CaretRight,
  Check,
  CircleNotch,
  ClipboardText,
  GitCommit,
  GitPullRequest,
  Sparkle,
  Trash,
  WarningCircle,
  X,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Markdown } from "@/components/Markdown";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { parseReviewPromptDisplayMessage } from "@/lib/aiReviewPromptDisplay";
import { summarizeActiveReviewFindings } from "@/lib/reviewFindingPublication";
import type {
  AiProvider,
  AiReviewFixPhase,
  AiReviewFixState,
  AiReviewRunState,
  AiReviewStore,
  AiReviewThread,
  ReviewEvidenceArtifact,
  ReviewFinding,
  ReviewFindingCategory,
  ReviewFindingSeverity,
  ReviewFindingStatus,
  ReviewPublicationMode,
  ReviewRun,
} from "@/types";

type PanelView = "review" | "output";

interface StageCommentsResult {
  added: number;
  skipped: number;
  skippedUnanchored: number;
  skippedExistingDrafts: number;
  skippedAlreadyStaged: number;
  skippedAlreadyPublished: number;
}

export interface AiReviewPanelProps {
  store: AiReviewStore | null;
  activeThread: AiReviewThread | null;
  activeRun?: ReviewRun | null;
  reviewState?: AiReviewRunState | null;
  aiProvider?: AiProvider;
  loading: boolean;
  error: string | null;
  reviewProfiles?: string[];
  selectedReviewProfile?: string;
  onReviewProfileChange?: (profile: string) => void;
  onRun?: () => void;
  onAsk?: (userMessage: string) => Promise<void> | void;
  onReply?: (threadId: string, userMessage: string) => Promise<void> | void;
  onCancelReview?: () => Promise<void>;
  onSelectThread?: (threadId: string) => Promise<void> | void;
  onClearThread?: (threadId: string) => void;
  onClose: () => void;
  onOpenFile?: (path: string, line?: number | null) => void;
  expanded?: boolean;
  onToggleExpand?: () => void;
  onStageComments?: () => Promise<StageCommentsResult>;
  fixState?: AiReviewFixState | null;
  fixBusy?: boolean;
  onStartFix?: () => Promise<void>;
  onCommit?: (message: string) => Promise<void>;
  onPush?: () => Promise<void>;
}

function formatGeneratedAt(generatedAt: string): string {
  const ms = parseInt(generatedAt, 10);
  if (Number.isNaN(ms)) return generatedAt;
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function parseMs(value: string | null | undefined): number | null {
  if (!value) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isNaN(parsed) ? null : parsed;
}

function formatDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(durationMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) return `${seconds}s`;
  return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
}

function phaseLabel(phase: AiReviewFixPhase): string {
  switch (phase) {
    case "preflight":
      return "Preparing repo";
    case "stashing":
      return "Stashing";
    case "switchingBranch":
      return "Switching branch";
    case "syncing":
      return "Syncing branch";
    case "mergingDestination":
      return "Preparing merge";
    case "restoringStash":
      return "Restoring stash";
    case "resolvingConflicts":
      return "Resolving conflicts";
    case "runningClaude":
      return "Running Claude";
    case "verifyingChanges":
      return "Verifying changes";
    case "readyToCommit":
      return "Ready to commit";
    case "committing":
      return "Creating commit";
    case "readyToPush":
      return "Ready to push";
    case "pushing":
      return "Pushing branch";
    case "completed":
      return "Done";
    default:
      return "Idle";
  }
}

function statusVariant(
  state: AiReviewFixState | null | undefined,
): "muted" | "success" | "secondary" {
  if (!state) return "muted";
  if (state.status === "running") return "secondary";
  if (state.status === "succeeded") return "success";
  return "muted";
}

function buildThreadTranscript(thread: AiReviewThread): string {
  return thread.messages
    .map(
      (message) =>
        `${message.role === "assistant" ? "Assistant" : "Reviewer"}:\n${message.content}`,
    )
    .join("\n\n");
}

function ReviewerMessage({ content }: { content: string }) {
  const parsedPrompt = parseReviewPromptDisplayMessage(content);
  const [promptOpen, setPromptOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  if (!parsedPrompt) {
    return <p className="whitespace-pre-wrap text-sm text-foreground">{content}</p>;
  }

  const copyPrompt = async () => {
    await navigator.clipboard.writeText(parsedPrompt.prompt);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="space-y-2 text-sm text-foreground">
      <p>{parsedPrompt.intro}</p>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" variant="secondary" onClick={() => setPromptOpen((open) => !open)}>
          {promptOpen ? "Hide full prompt" : "Show full prompt"}
        </Button>
        <Button size="sm" variant="ghost" onClick={copyPrompt}>
          <ClipboardText size={14} />
          {copied ? "Copied" : "Copy prompt"}
        </Button>
        <span className="text-[11px] text-muted-foreground">
          {parsedPrompt.prompt.length.toLocaleString()} chars sent to Claude
        </span>
      </div>
      {promptOpen && (
        <pre className="max-h-96 overflow-auto whitespace-pre-wrap rounded-md border border-border bg-background px-3 py-2 font-mono text-[11px] leading-relaxed text-muted-foreground">
          {parsedPrompt.prompt}
        </pre>
      )}
    </div>
  );
}

function turnLabel(state: AiReviewRunState | null | undefined): string {
  return state?.turnKind === "reply" ? "Replying in chat…" : "Running review…";
}

function titleCase(value: string): string {
  return value.replace(/(^|[\s-])([a-z])/g, (_match, prefix: string, char: string) => {
    return `${prefix}${char.toUpperCase()}`;
  });
}

function findingSeverityBadgeClass(severity: ReviewFindingSeverity): string {
  switch (severity) {
    case "critical":
      return "inline-flex items-center rounded-full border border-transparent bg-rose-600 px-2 py-0.5 text-xs font-medium text-white";
    case "high":
      return "inline-flex items-center rounded-full border border-transparent bg-orange-500 px-2 py-0.5 text-xs font-medium text-white";
    case "medium":
      return "inline-flex items-center rounded-full border border-transparent bg-amber-400 px-2 py-0.5 text-xs font-medium text-amber-950";
    case "low":
      return "inline-flex items-center rounded-full border border-border bg-muted px-2 py-0.5 text-xs font-medium text-foreground";
    default:
      return "inline-flex items-center rounded-full border border-border bg-background px-2 py-0.5 text-xs font-medium text-muted-foreground";
  }
}

function findingCategoryLabel(category: ReviewFindingCategory): string {
  return titleCase(category);
}

function findingStatusLabel(status: ReviewFindingStatus): string {
  return titleCase(status);
}

function publicationModeLabel(mode: ReviewPublicationMode): string {
  switch (mode) {
    case "general":
      return "General";
    case "file":
      return "File";
    case "localOnly":
      return "Local only";
    default:
      return "Inline";
  }
}

type EvidenceStatus = "passed" | "failed" | "skipped" | "timed out" | "errored" | "unknown";

function evidenceStatus(evidence: ReviewEvidenceArtifact): EvidenceStatus {
  if (evidence.payload) {
    try {
      const parsed = JSON.parse(evidence.payload) as { status?: unknown };
      if (typeof parsed.status === "string") {
        const status = parsed.status.toLowerCase();
        if (
          status === "passed" ||
          status === "failed" ||
          status === "skipped" ||
          status === "timed out" ||
          status === "errored"
        ) {
          return status;
        }
      }
    } catch {
      // Non-JSON evidence payloads are still valid, just not status-aware.
    }
  }
  const summary = evidence.summary?.toLowerCase() ?? "";
  if (summary.includes("passed")) return "passed";
  if (summary.includes("failed")) return "failed";
  if (summary.includes("skipped")) return "skipped";
  if (summary.includes("timed out")) return "timed out";
  if (summary.includes("errored")) return "errored";
  return "unknown";
}

function evidenceBadgeVariant(status: EvidenceStatus): "success" | "secondary" | "muted" {
  if (status === "passed") return "success";
  if (status === "failed" || status === "timed out" || status === "errored") return "secondary";
  return "muted";
}

function EvidenceSection({ evidence }: { evidence: ReviewEvidenceArtifact[] }) {
  const analyzerEvidence = evidence.filter((item) => item.kind === "analyzer");
  const [openPayloads, setOpenPayloads] = useState<Record<string, boolean>>({});

  if (analyzerEvidence.length === 0) return null;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
          Evidence
        </div>
        <span className="text-[11px] text-muted-foreground">
          {analyzerEvidence.length} analyzer{analyzerEvidence.length === 1 ? "" : "s"}
        </span>
      </div>
      <div className="space-y-2">
        {analyzerEvidence.map((item) => {
          const status = evidenceStatus(item);
          const payloadOpen = openPayloads[item.id] ?? false;
          return (
            <div key={item.id} className="rounded-md border border-border bg-background px-3 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant={evidenceBadgeVariant(status)}>{titleCase(status)}</Badge>
                <span className="text-sm font-medium text-foreground">{item.title}</span>
                <span className="font-mono text-[11px] text-muted-foreground">{item.source}</span>
              </div>
              {item.summary && <p className="mt-2 text-sm text-muted-foreground">{item.summary}</p>}
              {item.payload && (
                <div className="mt-2">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() =>
                      setOpenPayloads((current) => ({
                        ...current,
                        [item.id]: !payloadOpen,
                      }))
                    }
                  >
                    {payloadOpen ? "Hide output" : "Show output"}
                  </Button>
                  {payloadOpen && (
                    <pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap rounded-md border border-border bg-muted/40 px-3 py-2 font-mono text-[11px] text-muted-foreground">
                      {item.payload}
                    </pre>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function findingAnchorLabel(
  finding: ReviewFinding,
  publicationMode: ReviewPublicationMode | null = null,
): string {
  if (!finding.anchor) {
    if (publicationMode === "general" || finding.publication?.mode === "general") {
      return "General PR comment";
    }
    if (publicationMode === "localOnly" || finding.publication?.mode === "localOnly") {
      return "Local only";
    }
    return "Unanchored finding";
  }
  const endLine =
    finding.anchor.endLine != null && finding.anchor.endLine !== finding.anchor.startLine
      ? `-${finding.anchor.endLine}`
      : "";
  return `${finding.anchor.path}:${finding.anchor.startLine}${endLine} (${finding.anchor.side})`;
}

function formatStageCommentsFeedback(result: StageCommentsResult): string {
  const skippedParts: string[] = [];
  if (result.skippedAlreadyStaged > 0) {
    skippedParts.push(
      `${result.skippedAlreadyStaged} already staged finding${
        result.skippedAlreadyStaged === 1 ? "" : "s"
      }`,
    );
  }
  if (result.skippedAlreadyPublished > 0) {
    skippedParts.push(
      `${result.skippedAlreadyPublished} previously published finding${
        result.skippedAlreadyPublished === 1 ? "" : "s"
      }`,
    );
  }
  if (result.skippedExistingDrafts > 0) {
    skippedParts.push(
      `${result.skippedExistingDrafts} duplicate local draft${
        result.skippedExistingDrafts === 1 ? "" : "s"
      }`,
    );
  }
  if (result.skippedUnanchored > 0) {
    skippedParts.push(
      `${result.skippedUnanchored} unanchored suggestion${
        result.skippedUnanchored === 1 ? "" : "s"
      }`,
    );
  }

  if (result.added > 0) {
    return skippedParts.length > 0
      ? `Staged ${result.added} PR comment(s). Skipped ${skippedParts.join(", ")}.`
      : `Staged ${result.added} PR comment(s) in the pending review.`;
  }

  if (skippedParts.length > 0) {
    return `No PR comments were staged. Skipped ${skippedParts.join(", ")}.`;
  }

  return "No PR comments were staged.";
}

function ReviewActivityLog({
  logs,
  title,
  compact = false,
}: {
  logs: string[];
  title: string;
  compact?: boolean;
}) {
  const [open, setOpen] = useState(!compact);
  const latestLog = logs[logs.length - 1];

  return (
    <div className="min-h-0 rounded-md border border-border bg-muted/40">
      <button
        type="button"
        className="flex w-full items-center justify-between gap-2 border-b border-border px-3 py-2 text-left"
        onClick={() => setOpen((current) => !current)}
      >
        <span className="shrink-0 text-xs font-medium text-foreground">{title}</span>
        <span className="min-w-0 flex-1 truncate text-[11px] text-muted-foreground">
          {latestLog}
        </span>
        <CaretRight
          size={12}
          className={[
            "shrink-0 text-muted-foreground transition-transform",
            open ? "rotate-90" : "",
          ].join(" ")}
        />
      </button>
      {open && (
        <pre className="max-h-64 overflow-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11px] text-muted-foreground">
          {logs.join("\n")}
        </pre>
      )}
    </div>
  );
}

export function AiReviewPanel({
  store,
  activeThread,
  activeRun,
  reviewState,
  aiProvider = "claude",
  loading,
  error,
  reviewProfiles = [],
  selectedReviewProfile = "",
  onReviewProfileChange,
  onRun,
  onAsk,
  onReply,
  onCancelReview,
  onSelectThread,
  onClearThread,
  onClose,
  expanded = false,
  onToggleExpand,
  onStageComments,
  fixState,
  fixBusy = false,
  onStartFix,
  onCommit,
  onPush,
  onOpenFile,
}: AiReviewPanelProps) {
  const [copied, setCopied] = useState(false);
  const [clockMs, setClockMs] = useState(() => Date.now());
  const [panelView, setPanelView] = useState<PanelView>("review");
  const [commitMessage, setCommitMessage] = useState(fixState?.suggestedCommitMessage ?? "");
  const [replyState, setReplyState] = useState({
    threadId: activeThread?.id ?? null,
    value: "",
  });
  const [newThreadMessage, setNewThreadMessage] = useState("");
  const [stageCommentsBusy, setStageCommentsBusy] = useState(false);
  const [stageCommentsFeedback, setStageCommentsFeedback] = useState<string | null>(null);
  const commitDraftKeyRef = useRef(fixState?.suggestedCommitMessage ?? "");

  const fixRunning = fixState?.status === "running";
  const providerLabel =
    aiProvider === "codex" ? "Codex" : aiProvider === "opencode" ? "OpenCode" : "Claude";
  const hasThreads = Boolean(store?.threads.length);
  const hasReview = Boolean(hasThreads || loading || error || onRun);
  const canCommit =
    Boolean(fixState?.suggestedCommitMessage && !fixState?.commitSha && onCommit) && !fixRunning;
  const canPush = Boolean(fixState?.commitSha && onPush) && !fixRunning;
  const startedAtMs = parseMs(fixState?.startedAt);
  const finishedAtMs = parseMs(fixState?.finishedAt);
  const totalDurationMs = startedAtMs == null ? null : (finishedAtMs ?? clockMs) - startedAtMs;
  const reviewStartedAtMs = parseMs(reviewState?.startedAt);
  const reviewFinishedAtMs = parseMs(reviewState?.finishedAt);
  const reviewDurationMs =
    reviewStartedAtMs == null ? null : (reviewFinishedAtMs ?? clockMs) - reviewStartedAtMs;
  const reviewCancelled = reviewState?.status === "cancelled";
  const reviewRunning = reviewState?.status === "running";
  const replyInputDisabled = !activeThread || !onReply;
  const replySendDisabled = loading || !activeThread || !onReply;
  const newThreadDisabled = loading || !onAsk;
  const reviewLogs = reviewState?.logs ?? [];
  const showViewTabs = (fixState || onStartFix) && hasReview;
  const showOutputActions = canCommit || canPush;
  const activeThreadId = activeThread?.id ?? null;
  const showActiveThreadLogs =
    reviewLogs.length > 0 &&
    Boolean(activeThreadId && reviewState?.threadId && reviewState.threadId === activeThreadId);
  const showActiveThreadRunStatus = Boolean(
    activeThreadId &&
      reviewState?.threadId &&
      reviewState.threadId === activeThreadId &&
      (reviewRunning || reviewCancelled || error),
  );
  const sortedThreads = useMemo(
    () =>
      [...(store?.threads ?? [])].sort((a, b) => {
        const aMs = parseMs(a.updatedAt) ?? 0;
        const bMs = parseMs(b.updatedAt) ?? 0;
        return bMs - aMs;
      }),
    [store],
  );
  const findingPublication = useMemo(
    () => summarizeActiveReviewFindings(store, activeRun),
    [store, activeRun],
  );
  const findingItems = useMemo(
    () =>
      activeRun?.findings.map((finding) => ({
        finding,
        publication: findingPublication.get(finding.id) ?? null,
      })) ?? [],
    [activeRun, findingPublication],
  );
  const findingCount = findingItems.length;
  const stagedFindingCount = findingItems.filter((item) => item.publication?.alreadyStaged).length;
  const publishedFindingCount = findingItems.filter(
    (item) => item.publication?.alreadyPublished,
  ).length;

  useEffect(() => {
    if (!fixRunning && !reviewRunning) return;
    const timer = window.setInterval(() => setClockMs(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [fixRunning, reviewRunning]);

  const suggestedCommitMessage = fixState?.suggestedCommitMessage ?? "";
  if (commitDraftKeyRef.current !== suggestedCommitMessage) {
    commitDraftKeyRef.current = suggestedCommitMessage;
    setCommitMessage(suggestedCommitMessage);
  }

  if (replyState.threadId !== (activeThread?.id ?? null)) {
    setReplyState({ threadId: activeThread?.id ?? null, value: "" });
  }
  if (stageCommentsFeedback && replyState.threadId !== (activeThread?.id ?? null)) {
    setStageCommentsFeedback(null);
  }

  const replyValue = replyState.value;
  const sendReply = () => {
    const trimmed = replyValue.trim();
    if (!trimmed || replySendDisabled) return;
    onReply(activeThread.id, trimmed);
    setReplyState((current) => ({ ...current, value: "" }));
  };
  const sendNewThreadMessage = () => {
    const trimmed = newThreadMessage.trim();
    if (!trimmed || !onAsk) return;
    onAsk(trimmed);
    setNewThreadMessage("");
  };

  const handleCopy = () => {
    if (!activeThread) return;
    void navigator.clipboard.writeText(buildThreadTranscript(activeThread)).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const handleDeleteThread = () => {
    if (!activeThread || !onClearThread) return;
    const confirmed = window.confirm("Delete the active AI review thread?");
    if (!confirmed) return;
    onClearThread(activeThread.id);
  };

  const activeThreadHasAssistant = activeThread?.messages.some(
    (message) => message.role === "assistant",
  );
  const canStageComments =
    Boolean(onStageComments && activeThreadHasAssistant) &&
    !loading &&
    !fixBusy &&
    !fixRunning &&
    !stageCommentsBusy;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex shrink-0 flex-col gap-2 border-b border-border px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <Sparkle size={14} className="shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1 truncate text-sm font-semibold">AI Review</span>
          {activeThread && !loading && (
            <span className="shrink-0 text-xs text-muted-foreground">
              {formatGeneratedAt(activeThread.updatedAt)}
            </span>
          )}
          <Button
            size="icon"
            variant="ghost"
            className="size-7 shrink-0"
            onClick={onClose}
            title="Close AI review panel"
            aria-label="Close AI review panel"
          >
            <X size={14} />
          </Button>
        </div>
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          {findingCount > 0 && <Badge variant="secondary">{findingCount} findings</Badge>}
          <Badge variant="muted">{providerLabel}</Badge>
          {activeRun?.reviewProfile && <Badge variant="muted">{activeRun.reviewProfile}</Badge>}
          {reviewProfiles.length > 0 && (
            <select
              value={selectedReviewProfile}
              onChange={(event) => onReviewProfileChange?.(event.target.value)}
              className="h-7 max-w-40 rounded-md border border-border bg-secondary px-2 text-xs font-medium text-foreground outline-none transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
              title="Review profile"
              aria-label="Review profile"
              disabled={loading}
            >
              <option value="">Base profile</option>
              {reviewProfiles.map((profile) => (
                <option key={profile} value={profile}>
                  {profile}
                </option>
              ))}
            </select>
          )}
          {stagedFindingCount > 0 && <Badge variant="muted">{stagedFindingCount} staged</Badge>}
          {publishedFindingCount > 0 && (
            <Badge variant="success">{publishedFindingCount} published</Badge>
          )}
          <div className="ml-auto flex items-center gap-1">
            {loading && onCancelReview ? (
              <Button
                size="sm"
                variant="secondary"
                className="h-7 shrink-0 px-2 text-xs"
                onClick={() => void onCancelReview()}
              >
                <X size={14} />
                Cancel
              </Button>
            ) : onRun ? (
              <Button
                size="icon"
                variant="ghost"
                className="size-7 shrink-0"
                onClick={onRun}
                disabled={fixRunning}
                title="Run AI review"
                aria-label="Run AI review"
              >
                <ArrowsClockwise size={14} />
              </Button>
            ) : null}
            {activeThread && !loading && (
              <Button
                size="icon"
                variant="ghost"
                className="size-7 shrink-0"
                onClick={handleCopy}
                title="Copy active thread"
                aria-label="Copy active thread"
              >
                {copied ? (
                  <Check size={14} className="text-green-500" />
                ) : (
                  <ClipboardText size={14} />
                )}
              </Button>
            )}
            {activeThread && onClearThread && (
              <Button
                size="icon"
                variant="ghost"
                className="size-7 shrink-0"
                onClick={handleDeleteThread}
                disabled={loading || fixRunning}
                title="Delete active review thread"
                aria-label="Delete active review thread"
              >
                <Trash size={14} />
              </Button>
            )}
            {onToggleExpand && (
              <Button
                size="icon"
                variant="ghost"
                className="size-7 shrink-0"
                onClick={onToggleExpand}
                title={expanded ? "Collapse panel" : "Expand panel to full width"}
                aria-label={expanded ? "Collapse panel" : "Expand panel to full width"}
              >
                {expanded ? <ArrowsIn size={14} /> : <ArrowsOut size={14} />}
              </Button>
            )}
          </div>
        </div>
      </div>

      {showViewTabs && (
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border px-3 py-2 text-xs">
          <div className="flex min-w-0 items-center gap-2 text-muted-foreground">
            <span className={panelView === "review" ? "font-medium text-foreground" : undefined}>
              Review
            </span>
            <CaretRight size={12} />
            <span className={panelView === "output" ? "font-medium text-foreground" : undefined}>
              Output
            </span>
          </div>
          {panelView === "review" ? (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setPanelView("output")}
              disabled={!fixState}
            >
              Go to output
            </Button>
          ) : (
            <Button size="sm" variant="ghost" onClick={() => setPanelView("review")}>
              Back to review
            </Button>
          )}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {!loading && !error && panelView === "output" && (
          <div className="min-h-full p-4">
            {!fixState ? (
              <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
                <Sparkle size={28} weight="thin" className="text-muted-foreground" />
                <p className="text-sm text-muted-foreground">No output available.</p>
                <p className="text-xs text-muted-foreground">
                  Run Claude fix to see logs, summary, and a suggested commit message.
                </p>
              </div>
            ) : (
              <div className="flex min-h-full flex-col gap-3">
                <div className="flex items-start gap-2">
                  <Badge variant={statusVariant(fixState)}>
                    {phaseLabel(fixState?.phase ?? "idle")}
                  </Badge>
                  {fixState?.repoPath && (
                    <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
                      {fixState.repoPath}
                    </span>
                  )}
                  {fixRunning && (
                    <CircleNotch size={14} className="mt-0.5 animate-spin text-muted-foreground" />
                  )}
                </div>

                {(totalDurationMs != null || fixState?.claudeDurationMs != null) && (
                  <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                    {totalDurationMs != null && (
                      <span>Total: {formatDuration(totalDurationMs)}</span>
                    )}
                    {fixState?.claudeDurationMs != null && (
                      <span>Claude: {formatDuration(fixState.claudeDurationMs)}</span>
                    )}
                  </div>
                )}

                {showOutputActions && (
                  <div className="flex flex-wrap items-center gap-2">
                    {canCommit && onCommit && (
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => void onCommit(commitMessage)}
                        disabled={fixBusy || !commitMessage.trim()}
                      >
                        <GitCommit size={14} />
                        Commit
                      </Button>
                    )}
                    {canPush && onPush && (
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => void onPush()}
                        disabled={fixBusy}
                      >
                        <GitPullRequest size={14} />
                        Push
                      </Button>
                    )}
                  </div>
                )}

                {fixState?.summary && (
                  <div className="rounded-md border border-border bg-muted/40 px-3 py-2">
                    <p className="text-xs font-medium text-foreground">Summary</p>
                    <Markdown className="mt-2 text-sm">{fixState.summary}</Markdown>
                  </div>
                )}

                {fixState?.error && (
                  <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                    <WarningCircle size={14} className="mt-0.5 shrink-0" />
                    <span className="min-w-0 break-words">{fixState.error}</span>
                  </div>
                )}

                {fixState?.suggestedCommitMessage && !fixState.commitSha && (
                  <div className="space-y-2 rounded-md border border-border bg-muted/40 px-3 py-3">
                    <label
                      htmlFor="ai-review-commit-message"
                      className="block text-xs font-medium text-foreground"
                    >
                      Commit message
                    </label>
                    <textarea
                      id="ai-review-commit-message"
                      value={commitMessage}
                      onChange={(event) => setCommitMessage(event.target.value)}
                      rows={3}
                      disabled={fixRunning}
                      className="w-full resize-y rounded-md border border-input bg-background px-2 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                    />
                  </div>
                )}

                {fixState?.commitSha && (
                  <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                    Commit created:{" "}
                    <span className="font-mono text-foreground">{fixState.commitSha}</span>
                  </div>
                )}

                {fixState?.tests.length ? (
                  <div className="rounded-md border border-border bg-muted/40 px-3 py-2">
                    <p className="text-xs font-medium text-foreground">Suggested checks</p>
                    <ul className="mt-1 text-xs text-muted-foreground">
                      {fixState.tests.map((test) => (
                        <li key={test} className="font-mono">
                          {test}
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}

                {fixState?.logs.length ? (
                  <div className="min-h-0 rounded-md border border-border bg-muted/40">
                    <div className="border-b border-border px-3 py-2 text-xs font-medium text-foreground">
                      Activity
                    </div>
                    <pre className="overflow-x-auto whitespace-pre-wrap px-3 py-2 font-mono text-[11px] text-muted-foreground">
                      {fixState.logs.join("\n")}
                    </pre>
                  </div>
                ) : null}
              </div>
            )}
          </div>
        )}

        {loading && !activeThread && panelView === "review" && (
          <div className="min-h-full p-4">
            <div className="flex flex-col gap-4">
              {hasThreads && (
                <div className="space-y-2">
                  <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Threads
                  </div>
                  <div className="flex gap-2 overflow-x-auto pb-1">
                    {sortedThreads.map((thread) => {
                      const isActive = thread.id === activeThreadId;
                      return (
                        <button
                          key={thread.id}
                          type="button"
                          onClick={() => onSelectThread?.(thread.id)}
                          className={[
                            "min-w-[10rem] rounded-md border px-3 py-2 text-left transition-colors",
                            isActive
                              ? "border-primary bg-primary/10 text-foreground"
                              : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:text-foreground",
                          ].join(" ")}
                        >
                          <p className="text-xs font-medium text-foreground">{thread.title}</p>
                          <p className="mt-1 text-[11px]">{formatGeneratedAt(thread.updatedAt)}</p>
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              <div className="flex max-w-sm flex-col items-center gap-3 text-center">
                <div className="rounded-md border border-dashed border-border px-6 py-8">
                  <div className="flex flex-col items-center gap-3">
                    <CircleNotch size={18} className="animate-spin text-muted-foreground" />
                    <div className="space-y-1">
                      <p className="text-sm font-medium text-foreground">
                        {turnLabel(reviewState)}
                      </p>
                      {reviewState?.prTitle && (
                        <p className="text-sm text-muted-foreground">{reviewState.prTitle}</p>
                      )}
                      {reviewDurationMs != null && (
                        <p className="text-xs text-muted-foreground">
                          Running for {formatDuration(reviewDurationMs)}
                        </p>
                      )}
                    </div>
                    {reviewState?.status === "running" && onCancelReview && (
                      <Button size="sm" variant="secondary" onClick={() => void onCancelReview()}>
                        Cancel review
                      </Button>
                    )}
                  </div>
                </div>
              </div>
              {reviewLogs.length > 0 && (
                <ReviewActivityLog logs={reviewLogs} title="Live activity" />
              )}
            </div>
          </div>
        )}

        {!activeThread && error && panelView === "review" && (
          <div className="min-h-full p-4">
            <div className="flex flex-col gap-3">
              {hasThreads && (
                <div className="space-y-2">
                  <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Threads
                  </div>
                  <div className="flex gap-2 overflow-x-auto pb-1">
                    {sortedThreads.map((thread) => {
                      const isActive = thread.id === activeThreadId;
                      return (
                        <button
                          key={thread.id}
                          type="button"
                          onClick={() => onSelectThread?.(thread.id)}
                          className={[
                            "min-w-[10rem] rounded-md border px-3 py-2 text-left transition-colors",
                            isActive
                              ? "border-primary bg-primary/10 text-foreground"
                              : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:text-foreground",
                          ].join(" ")}
                        >
                          <p className="text-xs font-medium text-foreground">{thread.title}</p>
                          <p className="mt-1 text-[11px]">{formatGeneratedAt(thread.updatedAt)}</p>
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              <div className="flex items-start gap-2 text-sm text-destructive">
                <WarningCircle size={16} className="mt-0.5 shrink-0" />
                <span className="min-w-0 break-words">{error}</span>
              </div>
              {onRun && (
                <Button size="sm" variant="secondary" onClick={onRun} className="self-start">
                  Retry
                </Button>
              )}
              {reviewLogs.length > 0 && <ReviewActivityLog logs={reviewLogs} title="Activity" />}
            </div>
          </div>
        )}

        {!activeThread && !error && reviewCancelled && panelView === "review" && (
          <div className="min-h-full p-4">
            <div className="flex flex-col gap-3">
              {hasThreads && (
                <div className="space-y-2">
                  <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Threads
                  </div>
                  <div className="flex gap-2 overflow-x-auto pb-1">
                    {sortedThreads.map((thread) => {
                      const isActive = thread.id === activeThreadId;
                      return (
                        <button
                          key={thread.id}
                          type="button"
                          onClick={() => onSelectThread?.(thread.id)}
                          className={[
                            "min-w-[10rem] rounded-md border px-3 py-2 text-left transition-colors",
                            isActive
                              ? "border-primary bg-primary/10 text-foreground"
                              : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:text-foreground",
                          ].join(" ")}
                        >
                          <p className="text-xs font-medium text-foreground">{thread.title}</p>
                          <p className="mt-1 text-[11px]">{formatGeneratedAt(thread.updatedAt)}</p>
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}

              <div className="flex items-start gap-2 text-sm text-muted-foreground">
                <WarningCircle size={16} className="mt-0.5 shrink-0" />
                <div className="min-w-0">
                  <p className="font-medium text-foreground">Review cancelled</p>
                  {reviewState?.prTitle && <p>{reviewState.prTitle}</p>}
                  {reviewDurationMs != null && (
                    <p className="text-xs">Stopped after {formatDuration(reviewDurationMs)}</p>
                  )}
                </div>
              </div>
              {reviewLogs.length > 0 && <ReviewActivityLog logs={reviewLogs} title="Activity" />}
            </div>
          </div>
        )}

        {!loading && !error && !hasThreads && panelView === "review" && (
          <div className="flex min-h-full flex-col justify-end p-4">
            <div className="mx-auto mb-6 flex max-w-xl flex-col items-center gap-3 text-center">
              <Sparkle size={32} weight="thin" className="text-muted-foreground" />
              <p className="text-sm text-muted-foreground">
                Ask {providerLabel} about this pull request.
              </p>
              <p className="text-xs text-muted-foreground">
                Type a question freely, or use the AI review shortcut to start a full review.
              </p>
            </div>
            <div className="rounded-md border border-border bg-background px-4 py-3">
              <div className="space-y-2">
                <label
                  htmlFor="ai-review-new-message"
                  className="block text-xs font-medium text-foreground"
                >
                  Message {providerLabel}
                </label>
                <textarea
                  id="ai-review-new-message"
                  value={newThreadMessage}
                  onChange={(event) => setNewThreadMessage(event.target.value)}
                  rows={3}
                  disabled={newThreadDisabled}
                  placeholder="Ask for a risk assessment, explain a file, or provide review instructions..."
                  className="w-full resize-y rounded-md border border-input bg-background px-2 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                  onKeyDown={(event) => {
                    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                      event.preventDefault();
                      if (newThreadDisabled || !newThreadMessage.trim()) return;
                      sendNewThreadMessage();
                    }
                  }}
                />
                <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-muted-foreground">
                  <span>Cmd/Ctrl + Enter to send</span>
                  <div className="grid w-full grid-cols-2 gap-2">
                    {onRun && (
                      <Button
                        size="sm"
                        variant="secondary"
                        className="w-full min-w-0 px-2"
                        onClick={() => {
                          setPanelView("review");
                          onRun();
                        }}
                        disabled={loading || fixRunning}
                      >
                        <Sparkle size={14} />
                        AI review
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="secondary"
                      className="w-full min-w-0 px-2"
                      disabled={newThreadDisabled || !newThreadMessage.trim()}
                      onClick={sendNewThreadMessage}
                    >
                      Send
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {activeThread && panelView === "review" && (
          <div className="min-h-full p-4">
            <div className="flex flex-col gap-4">
              {hasThreads && (
                <div className="space-y-2">
                  <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Threads
                  </div>
                  <div className="flex gap-2 overflow-x-auto pb-1">
                    {sortedThreads.map((thread) => {
                      const isActive = thread.id === activeThread.id;
                      return (
                        <button
                          key={thread.id}
                          type="button"
                          onClick={() => onSelectThread?.(thread.id)}
                          className={[
                            "min-w-[10rem] rounded-md border px-3 py-2 text-left transition-colors",
                            isActive
                              ? "border-primary bg-primary/10 text-foreground"
                              : "border-border bg-muted/30 text-muted-foreground hover:border-primary/40 hover:text-foreground",
                          ].join(" ")}
                        >
                          <p className="text-xs font-medium text-foreground">{thread.title}</p>
                          <p className="mt-1 text-[11px]">{formatGeneratedAt(thread.updatedAt)}</p>
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
              {showActiveThreadRunStatus && (
                <div
                  className={[
                    "flex items-center justify-between gap-3 rounded-md border px-3 py-2 text-xs",
                    error
                      ? "border-destructive/30 bg-destructive/10 text-destructive"
                      : "border-border bg-muted/40 text-muted-foreground",
                  ].join(" ")}
                >
                  <div className="flex min-w-0 items-center gap-2">
                    {reviewRunning ? (
                      <CircleNotch size={14} className="shrink-0 animate-spin" />
                    ) : (
                      <WarningCircle size={14} className="shrink-0" />
                    )}
                    <span className="shrink-0 font-medium text-foreground">
                      {error
                        ? "Review failed"
                        : reviewCancelled
                          ? "Review cancelled"
                          : turnLabel(reviewState)}
                    </span>
                    {reviewDurationMs != null && (
                      <span className="shrink-0">
                        {reviewRunning ? "Running for" : "Stopped after"}{" "}
                        {formatDuration(reviewDurationMs)}
                      </span>
                    )}
                    {error && <span className="min-w-0 truncate">{error}</span>}
                  </div>
                  {reviewRunning && onCancelReview && (
                    <Button size="sm" variant="ghost" onClick={() => void onCancelReview()}>
                      Cancel review
                    </Button>
                  )}
                </div>
              )}
              {showActiveThreadLogs && (
                <ReviewActivityLog
                  logs={reviewLogs}
                  title={reviewRunning ? "Live activity" : "Latest run activity"}
                  compact={reviewRunning}
                />
              )}
              {activeRun?.evidence && <EvidenceSection evidence={activeRun.evidence} />}
              {findingItems.length > 0 && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                      Findings
                    </div>
                    <span className="text-[11px] text-muted-foreground">{findingCount} total</span>
                  </div>
                  <div className="space-y-2">
                    {findingItems.map(({ finding, publication }) => (
                      <div
                        key={finding.id}
                        className="rounded-md border border-border bg-background px-3 py-3"
                      >
                        <div className="flex flex-wrap items-center gap-2">
                          <span className={findingSeverityBadgeClass(finding.severity)}>
                            {titleCase(finding.severity)}
                          </span>
                          <Badge variant="muted">{findingCategoryLabel(finding.category)}</Badge>
                          {finding.status !== "new" && finding.status !== "published" && (
                            <Badge variant="secondary">{findingStatusLabel(finding.status)}</Badge>
                          )}
                          {publication?.alreadyStaged && (
                            <Badge variant="muted">
                              {publication.currentDraftCount > 0 ? "Staged" : "Already staged"}
                            </Badge>
                          )}
                          {publication?.alreadyPublished && (
                            <Badge variant="success">
                              {publication.currentPublishedCount > 0
                                ? "Published"
                                : "Previously published"}
                            </Badge>
                          )}
                          {publication?.staleAnchor && (
                            <Badge variant="secondary">Anchor moved</Badge>
                          )}
                          {publication?.publicationMode &&
                            publication.publicationMode !== "inline" && (
                              <Badge variant="secondary">
                                {publicationModeLabel(publication.publicationMode)}
                              </Badge>
                            )}
                        </div>
                        <p className="mt-2 text-sm font-medium text-foreground">{finding.title}</p>
                        <p className="mt-1 text-sm text-muted-foreground">{finding.summary}</p>
                        <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
                          {finding.anchor && onOpenFile ? (
                            <button
                              type="button"
                              className="font-mono text-muted-foreground hover:text-foreground"
                              onClick={() =>
                                onOpenFile(finding.anchor?.path ?? "", finding.anchor?.startLine)
                              }
                            >
                              {findingAnchorLabel(finding, publication?.publicationMode ?? null)}
                            </button>
                          ) : (
                            <span>
                              {findingAnchorLabel(finding, publication?.publicationMode ?? null)}
                            </span>
                          )}
                          <span>{titleCase(finding.confidence)} confidence</span>
                          {publication?.latestPublishedAt &&
                            publication.alreadyPublished &&
                            publication.currentPublishedCount === 0 && (
                              <span>Published in an earlier run</span>
                            )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {activeThread.messages.length === 0 ? (
                <div className="rounded-md border border-dashed border-border px-4 py-6 text-center text-sm text-muted-foreground">
                  {providerLabel} is preparing this thread.
                </div>
              ) : (
                activeThread.messages.map((message) => (
                  <div
                    key={message.id}
                    className={[
                      "rounded-md border px-3 py-3",
                      message.role === "assistant"
                        ? "border-border bg-background"
                        : "border-primary/20 bg-primary/5",
                    ].join(" ")}
                  >
                    <div className="mb-2 flex items-center justify-between gap-2">
                      <span className="text-xs font-medium text-foreground">
                        {message.role === "assistant" ? "Assistant" : "Reviewer"}
                      </span>
                      <span className="text-[11px] text-muted-foreground">
                        {formatGeneratedAt(message.createdAt)}
                      </span>
                    </div>
                    {message.role === "assistant" ? (
                      <Markdown className="text-sm">{message.content}</Markdown>
                    ) : (
                      <ReviewerMessage content={message.content} />
                    )}
                  </div>
                ))
              )}
              <div className="rounded-md border border-border bg-background px-4 py-3">
                <div className="space-y-2">
                  <label
                    htmlFor="ai-review-reply"
                    className="block text-xs font-medium text-foreground"
                  >
                    Reply to {providerLabel}
                  </label>
                  <textarea
                    id="ai-review-reply"
                    value={replyValue}
                    onChange={(event) =>
                      setReplyState((current) => ({ ...current, value: event.target.value }))
                    }
                    rows={3}
                    disabled={replyInputDisabled}
                    placeholder="Challenge a finding, ask for clarification, or provide more context..."
                    className="w-full resize-y rounded-md border border-input bg-background px-2 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                    onKeyDown={(event) => {
                      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                        event.preventDefault();
                        if (replySendDisabled || !replyValue.trim()) return;
                        sendReply();
                      }
                    }}
                  />
                  <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-muted-foreground">
                    <span>Cmd/Ctrl + Enter to send</span>
                    <div className="grid w-full grid-cols-2 gap-2">
                      {onStageComments && (
                        <Button
                          size="sm"
                          variant="secondary"
                          className="w-full min-w-0 px-2"
                          disabled={!canStageComments}
                          onClick={() => {
                            if (!onStageComments || stageCommentsBusy) return;
                            setStageCommentsBusy(true);
                            setStageCommentsFeedback(null);
                            void onStageComments()
                              .then((result) => {
                                setStageCommentsFeedback(formatStageCommentsFeedback(result));
                              })
                              .catch((stageError) => {
                                setStageCommentsFeedback(
                                  stageError instanceof Error
                                    ? stageError.message
                                    : String(stageError),
                                );
                              })
                              .finally(() => {
                                setStageCommentsBusy(false);
                              });
                          }}
                        >
                          {stageCommentsBusy ? (
                            <>
                              <CircleNotch size={14} className="animate-spin" />
                              Commenting…
                            </>
                          ) : (
                            "Comment on PR"
                          )}
                        </Button>
                      )}
                      {onStartFix && (
                        <Button
                          size="sm"
                          className="w-full min-w-0 px-2"
                          onClick={() => {
                            setPanelView("output");
                            void onStartFix();
                          }}
                          disabled={!activeThreadHasAssistant || loading || fixBusy || fixRunning}
                        >
                          {fixBusy || fixRunning ? (
                            <CircleNotch size={14} className="animate-spin" />
                          ) : (
                            <Sparkle size={14} />
                          )}
                          {fixState ? "Run fix again" : "Fix with Claude"}
                        </Button>
                      )}
                      {onRun && (
                        <Button
                          size="sm"
                          variant="secondary"
                          className="w-full min-w-0 px-2"
                          onClick={() => {
                            setPanelView("review");
                            onRun();
                          }}
                          disabled={loading || fixRunning}
                        >
                          <Sparkle size={14} />
                          AI review
                        </Button>
                      )}
                      <Button
                        size="sm"
                        variant="secondary"
                        className="w-full min-w-0 px-2"
                        disabled={replySendDisabled || !replyValue.trim()}
                        onClick={sendReply}
                      >
                        Send
                      </Button>
                    </div>
                  </div>
                  {stageCommentsFeedback && (
                    <p className="text-[11px] text-muted-foreground">{stageCommentsFeedback}</p>
                  )}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
