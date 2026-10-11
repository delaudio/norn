// Owned synthetic PR metadata for native tests and Storybook. No network or credentials.
import type {
  ProviderPrListParams,
  ProviderPrSummary,
  PrRepositoryIdentity,
} from "../../../src/protocol/generated/protocol";
import { FakeBackendTransport } from "./fakeTransport";

export const providerFixtureRepositories: PrRepositoryIdentity[] = [
  { provider: "github", workspace: "synthetic", repo: "review" },
  { provider: "bitbucket", workspace: "synthetic", repo: "review" },
];
export function providerFixturePr(
  id: number,
  provider: "github" | "bitbucket",
  draft = false,
): ProviderPrSummary {
  return {
    id,
    title: `${provider} change ${id} 世界`,
    authorDisplayName: "Same Name",
    authorAccountId: id === 7 ? "actual-me" : "actual-other",
    sourceBranch: `feature/${id}`,
    destinationBranch: "main",
    state: id === 9 ? "MERGED" : "OPEN",
    draft,
    createdOn: "2026-10-12T00:00:00Z",
    updatedOn: "2026-10-12T00:01:00Z",
    reviewers: [
      {
        accountId: "actual-reviewer",
        displayName: "Reviewer",
        role: "REVIEWER",
        approved: provider === "github",
      },
    ],
    commentCount: 2,
  };
}
export class FakeProviderTransport extends FakeBackendTransport {
  readonly calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  gate: Promise<void> | null = null;
  fault: "list" | "detail" | "user" | null = null;
  empty = false;
  activeReads = 0;
  maxReads = 0;
  constructor() {
    super({
      repositories: providerFixtureRepositories.map((repo) => ({ ...repo, localPath: null })),
      targets: [
        {
          ...providerFixtureRepositories[0],
          prId: 7,
          runId: "real-saved-run",
          title: "Saved review",
          status: "succeeded",
        },
      ],
    });
  }
  async start() {
    const ready = await super.start();
    return { ...ready, capabilities: [...ready.capabilities, "provider-pr-reads"] };
  }
  async request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    this.calls.push({ method, params: structuredClone(params) });
    if (method === "review.files") {
      const target = params.target as { kind?: string; prId?: number; runId?: string };
      if (target.kind === "provider" && target.prId && !target.runId)
        throw new Error(
          "No stored reviewed range exists for this PR. A live pinned diff is not available yet.",
        );
    }
    if (!method.startsWith("provider.")) return super.request(method, params);
    this.activeReads++;
    this.maxReads = Math.max(this.maxReads, this.activeReads);
    try {
      if (this.gate) await this.gate;
      const repository = params.repository as PrRepositoryIdentity;
      if (method === "provider.user") {
        if (this.fault === "user") throw new Error("Synthetic identity unavailable.");
        return { repository, accountId: "actual-me", displayName: "Same Name" };
      }
      if (method === "provider.prs") {
        if (this.fault === "list") throw new Error("Synthetic repository unavailable.");
        const request = params as unknown as ProviderPrListParams;
        const prs = (
          request.page === 1
            ? [
                providerFixturePr(7, repository.provider),
                providerFixturePr(8, repository.provider, true),
              ]
            : [providerFixturePr(9, repository.provider)]
        ).filter(
          (pr) =>
            (request.filter === "ALL" ||
              (request.filter === "DRAFT" && pr.draft) ||
              (request.filter === "MERGED" && pr.state === "MERGED") ||
              (request.filter === "OPEN" && pr.state === "OPEN")) &&
            (!request.authorId || pr.authorAccountId === request.authorId) &&
            (!request.query || pr.title.toLowerCase().includes(request.query.toLowerCase())),
        );
        return {
          request: params,
          values: this.empty ? [] : prs,
          nextPage: !this.empty && request.page === 1 ? 2 : null,
        };
      }
      if (method === "provider.pr.detail") {
        if (this.fault === "detail") throw new Error("Synthetic detail unavailable.");
        const pr = providerFixturePr(Number(params.prId), repository.provider, params.prId === 8);
        const { authorAccountId, commentCount: _comments, ...metadata } = pr;
        return {
          request: params,
          authorId: authorAccountId,
          browserUrl:
            repository.provider === "github"
              ? `https://github.com/${repository.workspace}/${repository.repo}/pull/${pr.id}`
              : `https://bitbucket.org/${repository.workspace}/${repository.repo}/pull-requests/${pr.id}`,
          detail: {
            ...metadata,
            descriptionRaw:
              "Actual synthetic description\n\nUnicode 世界 and reviewer evidence remain visible.\nLong descriptions scroll independently of the PR list.",
            sourceCommitHash: "b".repeat(40),
            destinationCommitHash: "a".repeat(40),
          },
        };
      }
      throw new Error("Unsupported synthetic provider read.");
    } finally {
      this.activeReads--;
    }
  }
}
