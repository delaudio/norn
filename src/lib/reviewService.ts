import { tauriCall } from "@/lib/tauri";
import type {
  AiReviewJob,
  AiReviewJobStatus,
  FindingPublicationRequest,
  PublishedCommentIdentity,
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
  return tauriCall<PublishedCommentIdentity>("publish_review_finding", { request });
}
