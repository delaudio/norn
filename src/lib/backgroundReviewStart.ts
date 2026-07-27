import type {
  AiProvider,
  ClaudeReviewEffort,
  ClaudeReviewModel,
  CodexReviewEffort,
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
}: BackgroundReviewStartInput) {
  return {
    workspace,
    repo,
    id: prId,
    title: detail.title || `PR #${prId}`,
    payload,
    sourceBranch: detail.sourceBranch,
    destinationBranch: detail.destinationBranch,
    reviewedHeadSha: detail.sourceCommitHash ?? null,
    aiProvider,
    claudeModel,
    claudeEffort,
    codexModel,
    codexEffort,
    reviewProfile: null,
    skipAnalyzers: true,
  };
}
