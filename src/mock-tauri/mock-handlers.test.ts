import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  AiReviewStore,
  FindingPublicationRequest,
  FindingReconciliationSummary,
} from "@/types";
import { mockHandlers, publishMockReviewFinding } from "./mock-handlers";

function publicationRequest(
  overrides: Partial<FindingPublicationRequest> = {},
): FindingPublicationRequest {
  return {
    schemaVersion: "v1",
    tenantId: "tenant-test",
    provider: "bitbucket",
    workspace: "Example-Workspace",
    repository: "Frontend-App",
    pullRequestId: 1731,
    baseSha: "1111111111111111111111111111111111111111",
    headSha: "2222222222222222222222222222222222222222",
    findingFingerprint: `fingerprint-${crypto.randomUUID()}`,
    anchor: {
      path: "src/App.tsx",
      startLine: 12,
      endLine: 14,
      side: "new",
    },
    title: "Finding title",
    body: "Finding body",
    severity: "high",
    ...overrides,
  };
}

describe("publishMockReviewFinding", () => {
  it("returns the same provider identity for a repeated finding publication", () => {
    const request = publicationRequest();

    const first = publishMockReviewFinding(request);
    const repeated = publishMockReviewFinding({
      ...request,
      workspace: request.workspace.toLowerCase(),
      repository: request.repository.toLowerCase(),
      baseSha: request.baseSha.toUpperCase(),
      headSha: request.headSha.toUpperCase(),
    });

    expect(repeated).toEqual(first);
  });

  it("rejects publication against a stale reviewed head", () => {
    expect(() =>
      publishMockReviewFinding(
        publicationRequest({
          headSha: "3333333333333333333333333333333333333333",
        }),
      ),
    ).toThrow("pull request changed");
  });

  it("rejects publication against a stale reviewed base", () => {
    expect(() =>
      publishMockReviewFinding(
        publicationRequest({
          baseSha: "4444444444444444444444444444444444444444",
        }),
      ),
    ).toThrow("pull request changed");
  });

  it("rejects oversized rendered markdown before creating a mock comment", () => {
    expect(() =>
      publishMockReviewFinding(
        publicationRequest({
          body: "x".repeat(32 * 1024),
        }),
      ),
    ).toThrow("markdown is too long");
  });

  it("reconciles a tracked finding through the mock IPC surface", () => {
    const request = publicationRequest();
    const summary = mockHandlers.reconcile_review_findings({
      request: {
        schemaVersion: "v1",
        tenantId: request.tenantId,
        provider: request.provider,
        workspace: request.workspace,
        repository: request.repository,
        pullRequestId: request.pullRequestId,
        baseSha: request.baseSha,
        headSha: request.headSha,
        trackedComments: [
          {
            findingFingerprint: request.findingFingerprint,
            commentId: "comment-existing",
          },
        ],
        currentFindings: [request],
      },
    }) as FindingReconciliationSummary;

    expect(summary.counts.updated).toBe(1);
    expect(summary.actions[0]).toMatchObject({
      findingFingerprint: request.findingFingerprint,
      previousCommentId: "comment-existing",
      commentId: "comment-existing",
    });
  });
});

describe("mock structured review runs", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("preserves reviewed base and head revisions for initial and reply runs", async () => {
    vi.useFakeTimers();
    const target = {
      workspace: "mock-revision-test",
      repo: "frontend",
      id: 91,
    };
    const reviewArgs = {
      ...target,
      title: "Revision-aware review",
      sourceBranch: "feature/revision-aware",
      destinationBranch: "main",
      reviewedBaseSha: "1111111111111111111111111111111111111111",
      reviewedHeadSha: "2222222222222222222222222222222222222222",
      displayMessage: "Review this pull request.",
    };

    mockHandlers.start_inline_review(reviewArgs);
    await vi.advanceTimersByTimeAsync(1_300);
    let store = mockHandlers.load_ai_review_store(target) as AiReviewStore;
    const initialRuns = store.reviewRuns ?? [];
    const initialRun = initialRuns[initialRuns.length - 1];
    expect(initialRun).toMatchObject({
      reviewedBaseSha: reviewArgs.reviewedBaseSha,
      reviewedHeadSha: reviewArgs.reviewedHeadSha,
      sourceBranch: reviewArgs.sourceBranch,
      destinationBranch: reviewArgs.destinationBranch,
      turnKind: "initial",
    });
    expect(initialRun?.findings).toHaveLength(1);

    mockHandlers.reply_inline_review({
      ...reviewArgs,
      threadId: store.activeThreadId,
      userMessage: "Check the filter path again.",
    });
    await vi.advanceTimersByTimeAsync(1_300);
    store = mockHandlers.load_ai_review_store(target) as AiReviewStore;
    const replyRuns = store.reviewRuns ?? [];
    expect(replyRuns[replyRuns.length - 1]).toMatchObject({
      reviewedBaseSha: reviewArgs.reviewedBaseSha,
      reviewedHeadSha: reviewArgs.reviewedHeadSha,
      turnKind: "reply",
    });

    mockHandlers.delete_saved_review(target);
  });
});
