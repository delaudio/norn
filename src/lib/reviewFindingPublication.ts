import type {
  AiReviewStore,
  DraftComment,
  FindingPublicationRequest,
  PullRequestDetail,
  ReviewFinding,
  ReviewFindingAnchor,
  ReviewProvider,
  ReviewPublicationMode,
  ReviewRun,
} from "@/types";
import type { LinkedAiReviewDraftComment } from "./aiReviewDraftComments";

export interface ReviewFindingPublicationSummary {
  findingId: string;
  findingFingerprint: string;
  publicationMode: ReviewPublicationMode | null;
  currentDraftCount: number;
  currentPublishedCount: number;
  historicalDraftCount: number;
  historicalPublishedCount: number;
  alreadyStaged: boolean;
  alreadyPublished: boolean;
  staleAnchor: boolean;
  latestPublishedAt: string | null;
}

export interface FilterStageableAiReviewDraftCommentsResult {
  stageable: LinkedAiReviewDraftComment[];
  skipped: number;
  skippedAlreadyStaged: number;
  skippedAlreadyPublished: number;
  skippedExistingDrafts: number;
}

interface FindingRunMatch {
  runId: string;
  finding: ReviewFinding;
}

export interface BuildFindingPublicationRequestInput {
  provider: ReviewProvider;
  workspace: string;
  repo: string;
  pr: PullRequestDetail;
  reviewRun: ReviewRun;
  draft: DraftComment;
}

export function buildFindingPublicationRequest({
  provider,
  workspace,
  repo,
  pr,
  reviewRun,
  draft,
}: BuildFindingPublicationRequestInput): FindingPublicationRequest {
  const findingRef = draft.findingRef;
  if (!findingRef || findingRef.reviewRunId !== reviewRun.id) {
    throw new Error("The draft is not linked to this review run.");
  }
  const finding = reviewRun.findings.find(
    (candidate) =>
      candidate.id === findingRef.findingId &&
      candidate.fingerprint === findingRef.findingFingerprint,
  );
  if (!finding) {
    throw new Error("The structured finding linked to this draft is no longer available.");
  }
  const headSha = draft.reviewHeadSha?.trim();
  if (!headSha) {
    throw new Error("The reviewed head commit is unavailable; refresh and restage the finding.");
  }
  const line = draft.to ?? draft.from;
  const side = draft.to != null ? "new" : draft.from != null ? "old" : null;
  if (!draft.path.trim() || line == null || side == null) {
    throw new Error("The structured finding draft no longer has a valid inline anchor.");
  }
  const findingAnchor = finding.anchor;
  const findingEndLine = findingAnchor?.endLine ?? findingAnchor?.startLine;
  if (
    !findingAnchor ||
    findingEndLine == null ||
    draft.path !== findingAnchor.path ||
    side !== findingAnchor.side ||
    line < findingAnchor.startLine ||
    line > findingEndLine
  ) {
    throw new Error("The staged draft no longer matches the structured finding anchor.");
  }
  if (!draft.raw.trim()) {
    throw new Error("The structured finding draft cannot be empty.");
  }

  return {
    schemaVersion: "v1",
    tenantId: "local",
    provider,
    workspace,
    repository: repo,
    pullRequestId: pr.id,
    headSha,
    findingFingerprint: finding.fingerprint,
    anchor: {
      path: findingAnchor.path,
      startLine: findingAnchor.startLine,
      endLine: findingEndLine,
      side: findingAnchor.side,
    },
    title: finding.title,
    body: draft.raw,
    severity: finding.severity,
    ...(finding.suggestedFix ? { suggestedFix: finding.suggestedFix } : {}),
  };
}

function anchorKey(anchor: ReviewFindingAnchor | null): string | null {
  if (!anchor) return null;
  return `${anchor.path}:${anchor.side}:${anchor.startLine}:${anchor.endLine ?? anchor.startLine}`;
}

function parseTimestamp(value: string | null | undefined): number {
  if (!value) return 0;
  if (/^\d+$/.test(value)) {
    const parsed = Number.parseInt(value, 10);
    return Number.isNaN(parsed) ? 0 : parsed;
  }
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? 0 : parsed;
}

function findingPublicationMode(finding: ReviewFinding): ReviewPublicationMode | null {
  return finding.publication?.mode ?? (finding.anchor ? "inline" : null);
}

function draftCommentKey(draft: Pick<DraftComment, "path" | "to" | "from" | "raw">): string {
  return [draft.path, draft.to ?? "", draft.from ?? "", draft.raw.trim()].join("|");
}

export function summarizeActiveReviewFindings(
  store: AiReviewStore | null | undefined,
  activeRun: ReviewRun | null | undefined,
): Map<string, ReviewFindingPublicationSummary> {
  const summary = new Map<string, ReviewFindingPublicationSummary>();
  if (!activeRun) return summary;

  const matchesByFingerprint = new Map<string, FindingRunMatch[]>();
  for (const run of store?.reviewRuns ?? []) {
    for (const finding of run.findings) {
      const current = matchesByFingerprint.get(finding.fingerprint) ?? [];
      current.push({ runId: run.id, finding });
      matchesByFingerprint.set(finding.fingerprint, current);
    }
  }

  for (const finding of activeRun.findings) {
    const matches = [...(matchesByFingerprint.get(finding.fingerprint) ?? [])];
    if (!matches.some((match) => match.runId === activeRun.id && match.finding.id === finding.id)) {
      matches.push({ runId: activeRun.id, finding });
    }

    let currentDraftCount = 0;
    let currentPublishedCount = 0;
    let historicalDraftCount = 0;
    let historicalPublishedCount = 0;
    let staleAnchor = false;
    let latestPublishedAt: string | null = finding.publication?.publishedAt ?? null;
    let latestPublishedMs = parseTimestamp(latestPublishedAt);
    let publicationMode = findingPublicationMode(finding);
    const currentAnchor = anchorKey(finding.anchor);

    for (const match of matches) {
      const draftCount = match.finding.publication?.draftIds.length ?? 0;
      const publishedCount = match.finding.publication?.remoteCommentIds.length ?? 0;
      const isCurrent = match.runId === activeRun.id && match.finding.id === finding.id;

      if (isCurrent) {
        currentDraftCount += draftCount;
        currentPublishedCount += publishedCount;
      } else {
        historicalDraftCount += draftCount;
        historicalPublishedCount += publishedCount;
      }

      if (!publicationMode && match.finding.publication?.mode) {
        publicationMode = match.finding.publication.mode;
      }

      if ((draftCount > 0 || publishedCount > 0) && !isCurrent) {
        staleAnchor ||= anchorKey(match.finding.anchor) !== currentAnchor;
      }

      const publishedAt = match.finding.publication?.publishedAt ?? null;
      const publishedAtMs = parseTimestamp(publishedAt);
      if (publishedAtMs > latestPublishedMs) {
        latestPublishedMs = publishedAtMs;
        latestPublishedAt = publishedAt;
      }
    }

    summary.set(finding.id, {
      findingId: finding.id,
      findingFingerprint: finding.fingerprint,
      publicationMode,
      currentDraftCount,
      currentPublishedCount,
      historicalDraftCount,
      historicalPublishedCount,
      alreadyStaged: currentDraftCount + historicalDraftCount > 0,
      alreadyPublished: currentPublishedCount + historicalPublishedCount > 0,
      staleAnchor,
      latestPublishedAt,
    });
  }

  return summary;
}

export function filterStageableAiReviewDraftComments(
  comments: LinkedAiReviewDraftComment[],
  existingDrafts: Pick<DraftComment, "path" | "to" | "from" | "raw">[],
  publicationSummary: Map<string, ReviewFindingPublicationSummary>,
): FilterStageableAiReviewDraftCommentsResult {
  const existingDraftKeys = new Set(existingDrafts.map(draftCommentKey));
  const stageable: LinkedAiReviewDraftComment[] = [];
  let skippedAlreadyStaged = 0;
  let skippedAlreadyPublished = 0;
  let skippedExistingDrafts = 0;

  for (const comment of comments) {
    if (comment.findingRef) {
      const findingSummary = publicationSummary.get(comment.findingRef.findingId);
      if (findingSummary?.alreadyStaged) {
        skippedAlreadyStaged += 1;
        continue;
      }
      if (findingSummary?.alreadyPublished) {
        skippedAlreadyPublished += 1;
        continue;
      }
    }

    const key = draftCommentKey(comment);
    if (existingDraftKeys.has(key)) {
      skippedExistingDrafts += 1;
      continue;
    }

    existingDraftKeys.add(key);
    stageable.push(comment);
  }

  return {
    stageable,
    skipped: skippedAlreadyStaged + skippedAlreadyPublished + skippedExistingDrafts,
    skippedAlreadyStaged,
    skippedAlreadyPublished,
    skippedExistingDrafts,
  };
}
