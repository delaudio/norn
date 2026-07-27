import { tauriCall } from "@/lib/tauri";
import type {
  AiReviewJob,
  AiReviewJobStatus,
  AiReviewStore,
  FindingPublicationRequest,
  PublishedCommentIdentity,
  ReviewFindingPublicationEvent,
} from "@/types";

export interface ListAiReviewJobsInput {
  limit: number;
}

export interface CancelInlineReviewInput {
  workspace: string;
  repo: string;
  id: number;
}

export interface UpdateAiReviewJobStatusInput {
  jobId: string;
  status: AiReviewJobStatus;
  threadId: string | null;
  error: string | null;
}

export interface RecordReviewFindingPublicationEventsInput {
  workspace: string;
  repo: string;
  prId: number;
  events: ReviewFindingPublicationEvent[];
}

export class ReviewFindingPublicationError extends Error {
  readonly code: string | null;
  readonly retryable: boolean | null;

  constructor(message: string, code: string | null, retryable: boolean | null) {
    super(message);
    this.name = "ReviewFindingPublicationError";
    this.code = code;
    this.retryable = retryable;
  }
}

function normalizePublicationError(error: unknown): Error {
  if (error instanceof Error) return error;
  if (typeof error === "object" && error != null) {
    const record = error as Record<string, unknown>;
    if (typeof record.message === "string" && record.message.trim()) {
      return new ReviewFindingPublicationError(
        record.message,
        typeof record.code === "string" ? record.code : null,
        typeof record.retryable === "boolean" ? record.retryable : null,
      );
    }
  }
  return new ReviewFindingPublicationError(
    typeof error === "string" && error.trim() ? error : "Structured finding publication failed.",
    null,
    null,
  );
}

function publicationEventApplied(
  store: AiReviewStore,
  event: ReviewFindingPublicationEvent,
): boolean {
  const finding = store.reviewRuns
    ?.find((run) => run.id === event.reviewRunId)
    ?.findings.find((candidate) => candidate.fingerprint === event.findingFingerprint);
  if (!finding) return false;
  const publication = finding.publication;
  if (event.kind === "stageDraft") {
    return (
      event.draftId != null &&
      publication?.mode === event.mode &&
      publication.draftIds.includes(event.draftId)
    );
  }
  if (event.kind === "removeDraft") {
    return event.draftId != null && !publication?.draftIds.includes(event.draftId);
  }
  return (
    event.remoteCommentId != null &&
    publication?.mode === event.mode &&
    publication.remoteCommentIds.includes(event.remoteCommentId) &&
    (event.draftId == null || !publication.draftIds.includes(event.draftId))
  );
}

export function listAiReviewJobs(input: ListAiReviewJobsInput): Promise<AiReviewJob[]> {
  return tauriCall<AiReviewJob[]>("list_ai_review_jobs", { limit: input.limit });
}

export function cancelInlineReview(input: CancelInlineReviewInput): Promise<void> {
  return tauriCall<void>("cancel_inline_review", {
    workspace: input.workspace,
    repo: input.repo,
    id: input.id,
  });
}

export function updateAiReviewJobStatus(input: UpdateAiReviewJobStatusInput): Promise<AiReviewJob> {
  return tauriCall<AiReviewJob>("update_ai_review_job_status", {
    jobId: input.jobId,
    status: input.status,
    threadId: input.threadId,
    error: input.error,
  });
}

export function publishReviewFinding(
  request: FindingPublicationRequest,
): Promise<PublishedCommentIdentity> {
  return tauriCall<PublishedCommentIdentity>("publish_review_finding", { request }).catch(
    (error: unknown) => {
      throw normalizePublicationError(error);
    },
  );
}

export async function recordReviewFindingPublicationEvents({
  workspace,
  repo,
  prId,
  events,
}: RecordReviewFindingPublicationEventsInput): Promise<AiReviewStore> {
  if (events.length === 0) {
    throw new Error("Finding publication tracking requires at least one event.");
  }
  const store = await tauriCall<AiReviewStore | null>("record_ai_review_finding_publication", {
    workspace,
    repo,
    id: prId,
    events,
  });
  if (!store || !events.every((event) => publicationEventApplied(store, event))) {
    throw new Error(
      "Finding publication tracking was not applied; refresh the review before retrying.",
    );
  }
  return store;
}
