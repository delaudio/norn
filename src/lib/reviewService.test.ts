import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriCall } from "@/lib/tauri";
import type {
  AiReviewStore,
  FindingPublicationRequest,
  FindingReconciliationRequest,
  FindingReconciliationSummary,
  PublishedCommentIdentity,
  ReviewFindingPublicationEvent,
} from "@/types";
import {
  publishReviewFinding,
  ReviewFindingPublicationError,
  reconcileReviewFindings,
  recordReviewFindingPublicationEvents,
} from "./reviewService";

vi.mock("@/lib/tauri", () => ({
  tauriCall: vi.fn(),
}));

const request: FindingPublicationRequest = {
  schemaVersion: "v1",
  tenantId: "local",
  provider: "bitbucket",
  workspace: "acme",
  repository: "payments",
  pullRequestId: 42,
  baseSha: "1111111111111111111111111111111111111111",
  headSha: "2222222222222222222222222222222222222222",
  findingFingerprint: "finding-1",
  anchor: {
    path: "src/lib.ts",
    startLine: 12,
    endLine: 12,
    side: "new",
  },
  title: "Finding title",
  body: "Finding body",
  severity: "high",
};

const publicationEvent: ReviewFindingPublicationEvent = {
  kind: "publishDraft",
  reviewRunId: "run-1",
  findingFingerprint: "finding-1",
  mode: "inline",
  draftId: "draft-1",
  remoteCommentId: "comment-1",
  publishedAt: "2026-07-27T20:00:00.000Z",
};

function trackedStore(): AiReviewStore {
  return {
    activeThreadId: null,
    threads: [],
    reviewRuns: [
      {
        id: "run-1",
        schemaVersion: "v0.1",
        provider: "bitbucket",
        workspace: "acme",
        repo: "payments",
        prId: 42,
        sourceBranch: "feature/review",
        destinationBranch: "main",
        reviewedBaseSha: request.baseSha,
        reviewedHeadSha: request.headSha,
        status: "succeeded",
        turnKind: "initial",
        reviewProfile: null,
        createdAt: "0",
        finishedAt: "1",
        diffFingerprint: "diff",
        threadId: null,
        summaryMarkdown: null,
        evidence: [],
        findings: [
          {
            id: "finding-id-1",
            fingerprint: publicationEvent.findingFingerprint,
            title: request.title,
            severity: request.severity,
            confidence: "high",
            category: "bug",
            status: "published",
            summary: request.body,
            rationale: null,
            ruleId: null,
            source: "llm",
            anchor: request.anchor,
            suggestedFix: null,
            evidenceIds: [],
            publication: {
              mode: "inline",
              draftIds: [],
              remoteCommentIds: ["comment-1"],
              publishedAt: publicationEvent.publishedAt,
            },
          },
        ],
      },
    ],
  };
}

describe("publishReviewFinding", () => {
  beforeEach(() => {
    vi.mocked(tauriCall).mockReset();
  });

  it("owns the typed IPC command and request envelope", async () => {
    const published: PublishedCommentIdentity = {
      tenantId: request.tenantId,
      provider: request.provider,
      workspace: request.workspace,
      repository: request.repository,
      pullRequestId: request.pullRequestId,
      commentId: "comment-1",
      findingMarker: "<!-- lachesi:finding:test -->",
      path: request.anchor.path,
      startLine: request.anchor.startLine,
      endLine: request.anchor.endLine,
      side: request.anchor.side,
    };
    vi.mocked(tauriCall).mockResolvedValue(published);

    await expect(publishReviewFinding(request)).resolves.toEqual(published);
    expect(tauriCall).toHaveBeenCalledWith("publish_review_finding", { request });
  });

  it("normalizes serialized Tauri publication failures", async () => {
    vi.mocked(tauriCall).mockRejectedValue(
      JSON.stringify({
        code: "outdated_anchor",
        retryable: false,
        message: "The pull request head changed.",
      }),
    );

    const error = await publishReviewFinding(request).catch((caught: unknown) => caught);

    expect(error).toBeInstanceOf(ReviewFindingPublicationError);
    expect(error).toMatchObject({
      message: "The pull request head changed.",
      code: "outdated_anchor",
      retryable: false,
    });
  });
});

describe("reconcileReviewFindings", () => {
  beforeEach(() => {
    vi.mocked(tauriCall).mockReset();
  });

  it("owns the typed reconciliation IPC command and request envelope", async () => {
    const reconciliationRequest: FindingReconciliationRequest = {
      schemaVersion: "v1",
      tenantId: request.tenantId,
      provider: request.provider,
      workspace: request.workspace,
      repository: request.repository,
      pullRequestId: request.pullRequestId,
      baseSha: request.baseSha,
      headSha: request.headSha,
      trackedComments: [],
      currentFindings: [request],
    };
    const summary: FindingReconciliationSummary = {
      schemaVersion: "v1",
      status: "succeeded",
      tenantId: request.tenantId,
      provider: request.provider,
      workspace: request.workspace,
      repository: request.repository,
      pullRequestId: request.pullRequestId,
      baseSha: request.baseSha,
      headSha: request.headSha,
      counts: {
        unchanged: 0,
        created: 1,
        updated: 0,
        resolved: 0,
        reopened: 0,
        failed: 0,
      },
      actions: [
        {
          findingFingerprint: request.findingFingerprint,
          kind: "created",
          previousCommentId: null,
          commentId: "comment-1",
          providerMutated: true,
          error: null,
        },
      ],
    };
    vi.mocked(tauriCall).mockResolvedValue(summary);

    await expect(reconcileReviewFindings(reconciliationRequest)).resolves.toEqual(summary);
    expect(tauriCall).toHaveBeenCalledWith("reconcile_review_findings", {
      request: reconciliationRequest,
    });
  });
});

describe("recordReviewFindingPublicationEvents", () => {
  beforeEach(() => {
    vi.mocked(tauriCall).mockReset();
  });

  it("requires the returned store to contain every applied event", async () => {
    const store = trackedStore();
    vi.mocked(tauriCall).mockResolvedValue(store);

    await expect(
      recordReviewFindingPublicationEvents({
        workspace: "acme",
        repo: "payments",
        prId: 42,
        events: [publicationEvent],
      }),
    ).resolves.toEqual(store);
  });

  it("rejects missing or unchanged tracking state", async () => {
    vi.mocked(tauriCall)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce({
        ...trackedStore(),
        reviewRuns: [],
      });
    const input = {
      workspace: "acme",
      repo: "payments",
      prId: 42,
      events: [publicationEvent],
    };

    await expect(recordReviewFindingPublicationEvents(input)).rejects.toThrow("was not applied");
    await expect(recordReviewFindingPublicationEvents(input)).rejects.toThrow("was not applied");
  });
});
