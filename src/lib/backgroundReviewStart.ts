import type {
  AiProvider,
  ClaudeReviewEffort,
  ClaudeReviewModel,
  CodexReviewEffort,
  OpenCodeReviewEffort,
  PullRequestDetail,
} from "@/types";

export interface BackgroundReviewStartInput {
  workspace: string;
  repo: string;
  prId: number;
  detail: PullRequestDetail;
  payload: string;
  aiProvider: AiProvider;
  claudeModel: ClaudeReviewModel | null;
  claudeEffort: ClaudeReviewEffort | null;
  codexModel: string | null;
  codexEffort: CodexReviewEffort | null;
  opencodeModel: string | null;
  opencodeEffort: OpenCodeReviewEffort | null;
}

export function buildBackgroundReviewStartArgs({
  workspace,
  repo,
  prId,
  detail,
  payload,
  aiProvider,
  claudeModel,
  claudeEffort,
  codexModel,
  codexEffort,
  opencodeModel,
  opencodeEffort,
}: BackgroundReviewStartInput) {
  return {
    workspace,
    repo,
    id: prId,
    title: detail.title || `PR #${prId}`,
    payload,
    sourceBranch: detail.sourceBranch,
    destinationBranch: detail.destinationBranch,
    reviewedBaseSha: detail.destinationCommitHash ?? null,
    reviewedHeadSha: detail.sourceCommitHash ?? null,
    aiProvider,
    claudeModel,
    claudeEffort,
    codexModel,
    codexEffort,
    opencodeModel,
    opencodeEffort,
    reviewProfile: null,
    skipAnalyzers: true,
  };
}
