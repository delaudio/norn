// Owned synthetic PR metadata for native tests and Storybook. No network or credentials.
import type {
  ProviderApprovalEntry,
  ProviderApprovalProof,
  ProviderPrDetailParams,
  ProviderPrListParams,
  ProviderPrSummary,
  ProviderSnapshotRef,
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
  fault: "list" | "detail" | "user" | "snapshot" | null = null;
  private snapshotCounter = 0;
  approvalFault: "uncertain" | "rejected" | null = null;
  approvalEntries = new Map<string, ProviderApprovalEntry>();
  private approvalTokens = new Map<string, ProviderApprovalProof>();
  private approvalCounter = 0;
  private snapshots = new Map<
    string,
    { snapshot: ProviderSnapshotRef; patch: string; images: boolean }
  >();
  empty = false;
  images = false;
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
    return {
      ...ready,
      capabilities: [
        ...ready.capabilities,
        "provider-pr-reads",
        "provider-external-browser",
        "provider-pr-approval",
        "provider-snapshots",
        "provider-snapshot-preview",
      ],
    };
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
    if (method === "provider.browser") {
      const repository = params.repository as PrRepositoryIdentity;
      return {
        request: params,
        repository,
        url: `https://${repository.provider === "github" ? "github.com" : "bitbucket.org"}/${repository.workspace}/${repository.repo}/${repository.provider === "github" ? "pull" : "pull-requests"}/${params.prId}`,
        launchRequested: false,
        launchError: null,
      };
    }
    if (!method.startsWith("provider.")) return super.request(method, params);
    this.activeReads++;
    this.maxReads = Math.max(this.maxReads, this.activeReads);
    try {
      if (this.gate) await this.gate;
      if (method.startsWith("provider.approval.")) {
        const request = (
          method.endsWith("confirm") ? params.request : params
        ) as ProviderPrDetailParams;
        const key = JSON.stringify(request);
        const proof: ProviderApprovalProof = {
          request,
          repositoryId: `owned-${request.repository.provider}-repo`,
          actor: { accountId: "owned-reviewer", displayName: "Owned reviewer" },
          title: `${request.repository.provider} change ${request.prId} 世界`,
          headSha: "a".repeat(40),
          destinationSha: "b".repeat(40),
          sourceBranch: "feature",
          destinationBranch: "main",
          sourceRepository: null,
          state: "OPEN",
          draft: false,
          alreadyApproved: this.approvalEntries.get(key)?.state === "approved",
        };
        let entry = this.approvalEntries.get(key) ?? null;
        if (method.endsWith("preview")) {
          const approvalId =
            !proof.alreadyApproved && (!entry || entry.state === "rejected")
              ? (++this.approvalCounter).toString(16).padStart(64, "0")
              : null;
          if (approvalId) this.approvalTokens.set(approvalId, structuredClone(proof));
          return {
            request,
            proof,
            approvalId,
            expiresAt: approvalId ? Math.floor(Date.now() / 1000) + 300 : null,
            entry,
          };
        }
        if (method.endsWith("confirm")) {
          const token = this.approvalTokens.get(String(params.approvalId));
          this.approvalTokens.delete(String(params.approvalId));
          if (!token || JSON.stringify(token.request) !== key)
            throw new Error("Synthetic approval expired.");
          entry = {
            formatVersion: 1,
            proof,
            state: this.approvalFault ?? "approved",
            remoteReviewId: this.approvalFault ? null : "owned-receipt-1",
            updatedAt: Math.floor(Date.now() / 1000),
            attempt: (entry?.attempt ?? 0) + 1,
          };
          this.approvalEntries.set(key, entry);
        }
        return {
          request,
          proof,
          entry,
          observedApproved: entry?.state === "approved",
          outdated: false,
          localSaved: true,
        };
      }
      if (method === "provider.snapshot") {
        if (this.fault === "snapshot") throw new Error("Synthetic pinned snapshot unavailable.");
        const repository = params.repository as PrRepositoryIdentity;
        const snapshot = {
          repository,
          prId: Number(params.prId),
          snapshotId: (++this.snapshotCounter).toString(16).padStart(64, "0"),
        };
        const patch = `diff --git a/readme.ts b/readme.ts\n--- a/readme.ts\n+++ b/readme.ts\n@@ -1 +1 @@\n-old\n+${repository.provider} PR ${snapshot.prId} 世界\n`;
        this.snapshots.set(snapshot.snapshotId, { snapshot, patch, images: this.images });
        return {
          request: params,
          snapshot,
          title: `${repository.provider} preview #${snapshot.prId}`,
          sourceBranch: "feature",
          destinationBranch: "main",
          headSha: "a".repeat(40),
          destinationSha: "b".repeat(40),
          baseSha: "c".repeat(40),
          patchSha256: new Bun.CryptoHasher("sha256")
            .update(
              patch +
                (this.images
                  ? "diff --git a/old.png b/picture.png\nsimilarity index 100%\nrename from old.png\nrename to picture.png\n"
                  : ""),
            )
            .digest("hex"),
          fileCount: this.images ? 2 : 1,
          patchBytes:
            Buffer.byteLength(patch) +
            (this.images
              ? Buffer.byteLength(
                  "diff --git a/old.png b/picture.png\nsimilarity index 100%\nrename from old.png\nrename to picture.png\n",
                )
              : 0),
          expiresAt: 1900000000000,
        };
      }
      if (method.startsWith("provider.snapshot.")) {
        const snapshot = params.snapshot as ProviderSnapshotRef;
        const cached = this.snapshots.get(snapshot.snapshotId);
        if (!cached || JSON.stringify(cached.snapshot) !== JSON.stringify(snapshot))
          throw new Error("Synthetic snapshot expired or belongs to another PR.");
        if (method === "provider.snapshot.files")
          return {
            request: params,
            files: [
              {
                path: "readme.ts",
                oldPath: null,
                status: "modified",
                additions: 1,
                deletions: 1,
                binary: false,
              },
              ...(cached.images
                ? [
                    {
                      path: "picture.png",
                      oldPath: "old.png",
                      status: "renamed",
                      additions: 0,
                      deletions: 0,
                      binary: true,
                    },
                  ]
                : []),
            ],
            total: cached.images ? 2 : 1,
            nextOffset: null,
          };
        if (method === "provider.snapshot.diff") {
          const patch =
            params.path === "picture.png" && cached.images
              ? "diff --git a/old.png b/picture.png\nsimilarity index 100%\nrename from old.png\nrename to picture.png\n"
              : cached.patch;
          return {
            request: params,
            text: patch,
            totalBytes: Buffer.byteLength(patch),
            contentSha256: new Bun.CryptoHasher("sha256").update(patch).digest("hex"),
            nextOffset: null,
          };
        }
        if (
          method === "provider.snapshot.image" &&
          cached.images &&
          params.path === "picture.png"
        ) {
          const data =
            params.side === "old"
              ? "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGOoCLD5D8IMMAYARXwIDRVgxqcAAAAASUVORK5CYII="
              : "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAEUlEQVR4nGPQyOv5D8IMMAYAR3AIhdwoG5cAAAAASUVORK5CYII=";
          const bytes = Buffer.from(data, "base64");
          return {
            request: params,
            available: true,
            mimeType: "image/png",
            size: bytes.length,
            dataBase64: data,
            nextOffset: null,
            contentSha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
            width: 2,
            height: 2,
          };
        }
        if (method === "provider.snapshot.browser")
          return {
            request: params,
            url: `http://127.0.0.1:1234/session/${"a".repeat(64)}/`,
            launchRequested: false,
            launchError: null,
          };
        throw new Error("Synthetic image path is not in this snapshot.");
      }
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
