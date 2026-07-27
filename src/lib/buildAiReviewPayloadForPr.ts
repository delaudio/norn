import { buildReviewPayload } from "@/lib/buildReviewPayload";
import { extractIssueKeys } from "@/lib/jira";
import { resolveReviewPrompt } from "@/lib/reviewPrompt";
import { loadReviewReferences } from "@/lib/reviewReferencesStorage";
import { tauriCall } from "@/lib/tauri";
import type {
  BranchStatus,
  JiraIssue,
  NotionPage,
  PullRequestDetail,
  RepoRef,
  ReviewProvider,
} from "@/types";

export interface AiReviewPayloadForPr {
  payload: string;
  pr: PullRequestDetail;
  branchStatus: BranchStatus | null;
  jiraKeys: string[];
  rawDiff: string;
  reviewProfile: string | null;
}

function normalizedSha(value: string | null | undefined): string | null {
  const normalized = value?.trim().toLowerCase();
  return normalized || null;
}

export function assertStablePullRequestSnapshot(
  before: PullRequestDetail,
  after: PullRequestDetail,
): void {
  const beforeHead = normalizedSha(before.sourceCommitHash);
  const afterHead = normalizedSha(after.sourceCommitHash);
  const stable =
    before.id === after.id &&
    beforeHead != null &&
    beforeHead === afterHead &&
    normalizedSha(before.destinationCommitHash) === normalizedSha(after.destinationCommitHash) &&
    before.sourceBranch === after.sourceBranch &&
    before.destinationBranch === after.destinationBranch;
  if (!stable) {
    throw new Error(
      "The pull request changed while its review snapshot was loading; rerun the review.",
    );
  }
}

async function fetchReviewContext(jiraKeys: string[], enabled: boolean): Promise<string | null> {
  if (!enabled || jiraKeys.length === 0) return null;
  const parts: string[] = [];
  for (const key of jiraKeys) {
    try {
      const issue = await tauriCall<JiraIssue>("get_jira_issue", { key });
      parts.push(`### ${issue.key} — ${issue.summary}${issue.status ? ` (${issue.status})` : ""}`);
      if (issue.descriptionText) parts.push(issue.descriptionText);
      for (const url of issue.notionUrls) {
        try {
          const page = await tauriCall<NotionPage>("get_notion_page", { url });
          parts.push(`#### Notion: ${page.title || url}`);
          if (page.text) parts.push(page.text);
        } catch {
          // Skip pages the user has not configured or cannot access.
        }
      }
    } catch {
      // Skip Jira issues the user has not configured or cannot access.
    }
  }
  return parts.length > 0 ? parts.join("\n\n") : null;
}

export async function buildAiReviewPayloadForPr({
  workspace,
  repo,
  provider = "bitbucket",
  prId,
  repoConfig,
  jiraBaseUrl,
  jiraContextEnabled,
  reviewProfile,
}: {
  workspace: string;
  repo: string;
  provider?: ReviewProvider;
  prId: number;
  repoConfig?: RepoRef | null;
  jiraBaseUrl: string | null;
  jiraContextEnabled: boolean;
  reviewProfile?: string | null;
}): Promise<AiReviewPayloadForPr> {
  const pr = await tauriCall<PullRequestDetail>("get_pull_request", {
    provider,
    workspace,
    repo,
    id: prId,
  });
  const [rawDiff, branchStatus] = await Promise.all([
    tauriCall<string>("get_pr_diff", { provider, workspace, repo, id: prId }),
    tauriCall<BranchStatus>("get_branch_status", {
      provider,
      workspace,
      repo,
      source: pr.sourceBranch,
      destination: pr.destinationBranch,
    }).catch(() => null),
  ]);
  const jiraKeys = extractIssueKeys(pr.sourceBranch, pr.title);
  const jiraContext = await fetchReviewContext(jiraKeys, jiraContextEnabled);
  const { prompt, warnings } = await resolveReviewPrompt(
    `${workspace}/${repo}`,
    repoConfig?.localPath,
    reviewProfile,
  );
  if (warnings.length > 0) {
    console.warn("Lachesi repo config warnings:", warnings);
  }
  const verifiedPr = await tauriCall<PullRequestDetail>("get_pull_request", {
    provider,
    workspace,
    repo,
    id: prId,
  });
  assertStablePullRequestSnapshot(pr, verifiedPr);
  const payload = buildReviewPayload({
    prompt,
    pr: verifiedPr,
    branchStatus,
    rawDiff,
    jiraKeys,
    jiraBaseUrl,
    jiraContext,
    reviewReferences: loadReviewReferences(workspace, repo, prId),
  });
  return {
    payload,
    pr: verifiedPr,
    branchStatus,
    jiraKeys,
    rawDiff,
    reviewProfile: reviewProfile ?? null,
  };
}
