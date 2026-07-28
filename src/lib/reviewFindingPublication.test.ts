import { describe, expect, it } from "vitest";
import type { AiReviewStore, DraftComment, PullRequestDetail, ReviewRun } from "@/types";
import type { LinkedAiReviewDraftComment } from "./aiReviewDraftComments";
import {
  assertPullRequestMatchesReviewRun,
  buildFindingPublicationRequest,
  filterStageableAiReviewDraftComments,
  latestReviewFindingFingerprintsForRevision,
  latestTrackedFindingCommentId,
  latestTrackedFindingComments,
  selectTrackedFindingCommentsForBatch,
  summarizeActiveReviewFindings,
} from "./reviewFindingPublication";

const previousRun: ReviewRun = {
  id: "run-0",
  schemaVersion: "v0.1",
  provider: "bitbucket",
  workspace: "example-workspace",
  repo: "backend-api",
  prId: 1020,
  sourceBranch: "feature/invoice-lines-v2-bff-mock",
  destinationBranch: "develop",
  reviewedBaseSha: "1111111111111111111111111111111111111111",
  reviewedHeadSha: "2222222222222222222222222222222222222222",
  status: "succeeded",
  turnKind: "initial",
  reviewProfile: null,
  createdAt: "1750076300000",
  finishedAt: "1750076350000",
  diffFingerprint: "prev",
  threadId: "thread-0",
  summaryMarkdown: null,
  evidence: [],
  findings: [
    {
      id: "run-0-finding-1",
      fingerprint: "fingerprint-1",
      title: "Add @HttpCode(HttpStatus.OK) to the POST handler",
      severity: "high",
      confidence: "high",
      category: "bug",
      status: "published",
      summary: "Bitbucket already has a published comment for the earlier anchor.",
      rationale: null,
      ruleId: null,
      source: "llm",
      anchor: {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        startLine: 28,
        endLine: null,
        side: "new",
      },
      suggestedFix: null,
      evidenceIds: [],
      publication: {
        mode: "inline",
        draftIds: [],
        remoteCommentIds: ["101"],
        publishedAt: "2026-06-22T20:10:00.000Z",
      },
    },
  ],
};

const activeRun: ReviewRun = {
  id: "run-1",
  schemaVersion: "v0.1",
  provider: "bitbucket",
  workspace: "example-workspace",
  repo: "backend-api",
  prId: 1020,
  sourceBranch: "feature/invoice-lines-v2-bff-mock",
  destinationBranch: "develop",
  reviewedBaseSha: "1111111111111111111111111111111111111111",
  reviewedHeadSha: "2222222222222222222222222222222222222222",
  status: "succeeded",
  turnKind: "reply",
  reviewProfile: null,
  createdAt: "1750076400000",
  finishedAt: "1750076500000",
  diffFingerprint: "current",
  threadId: "thread-1",
  summaryMarkdown: null,
  evidence: [],
  findings: [
    {
      id: "run-1-finding-1",
      fingerprint: "fingerprint-1",
      title: "Add @HttpCode(HttpStatus.OK) to the POST handler",
      severity: "high",
      confidence: "high",
      category: "bug",
      status: "new",
      summary: "The same logical finding reran on a nearby line in the current diff.",
      rationale: null,
      ruleId: null,
      source: "llm",
      anchor: {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        startLine: 29,
        endLine: 30,
        side: "new",
      },
      suggestedFix: null,
      evidenceIds: [],
      publication: null,
    },
    {
      id: "run-1-finding-2",
      fingerprint: "fingerprint-2",
      title: "Sort the first page deterministically",
      severity: "medium",
      confidence: "high",
      category: "bug",
      status: "new",
      summary: "A pending draft already exists for this finding in the current run.",
      rationale: null,
      ruleId: null,
      source: "llm",
      anchor: {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        startLine: 44,
        endLine: null,
        side: "new",
      },
      suggestedFix: null,
      evidenceIds: [],
      publication: {
        mode: "inline",
        draftIds: ["draft-2"],
        remoteCommentIds: [],
        publishedAt: null,
      },
    },
  ],
};

const store: AiReviewStore = {
  activeThreadId: "thread-1",
  threads: [],
  reviewRuns: [previousRun, activeRun],
};

describe("buildFindingPublicationRequest", () => {
  it("builds the explicit provider-neutral request from the staged finding draft", () => {
    const draft: DraftComment = {
      localId: "draft-1",
      prId: 1020,
      path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
      to: 29,
      from: null,
      raw: "Publish the edited reviewer-facing explanation.",
      parentId: null,
      createdAt: 0,
      source: "aiFinding",
      findingRef: {
        reviewRunId: "run-1",
        findingId: "run-1-finding-1",
        findingFingerprint: "fingerprint-1",
      },
      publicationMode: "inline",
      reviewBaseSha: "1111111111111111111111111111111111111111",
      reviewHeadSha: "2222222222222222222222222222222222222222",
    };

    const request = buildFindingPublicationRequest({
      provider: "bitbucket",
      workspace: "example-workspace",
      repo: "backend-api",
      pr: {
        id: 1020,
        title: "Invoice lines",
        descriptionRaw: "",
        state: "OPEN",
        draft: false,
        authorDisplayName: "Reviewer",
        reviewers: [],
        sourceBranch: "feature/invoice-lines-v2-bff-mock",
        destinationBranch: "develop",
        sourceCommitHash: "2222222222222222222222222222222222222222",
        destinationCommitHash: "1111111111111111111111111111111111111111",
        createdOn: "",
        updatedOn: "",
      },
      reviewRun: activeRun,
      draft,
    });

    expect(request).toMatchObject({
      schemaVersion: "v1",
      tenantId: "local",
      provider: "bitbucket",
      workspace: "example-workspace",
      repository: "backend-api",
      pullRequestId: 1020,
      baseSha: "1111111111111111111111111111111111111111",
      headSha: "2222222222222222222222222222222222222222",
      findingFingerprint: "fingerprint-1",
      anchor: {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        startLine: 29,
        endLine: 30,
        side: "new",
      },
      body: "Publish the edited reviewer-facing explanation.",
      severity: "high",
    });
  });

  it("rejects draft head and active target metadata that diverge from the review run", () => {
    const draft: DraftComment = {
      localId: "draft-1",
      prId: 1020,
      path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
      to: 29,
      from: null,
      raw: "Publish the edited reviewer-facing explanation.",
      parentId: null,
      createdAt: 0,
      source: "aiFinding",
      findingRef: {
        reviewRunId: "run-1",
        findingId: "run-1-finding-1",
        findingFingerprint: "fingerprint-1",
      },
      publicationMode: "inline",
      reviewBaseSha: "1111111111111111111111111111111111111111",
      reviewHeadSha: "3333333333333333333333333333333333333333",
    };
    const pr = {
      id: 1020,
      title: "Invoice lines",
      descriptionRaw: "",
      state: "OPEN" as const,
      draft: false,
      authorDisplayName: "Reviewer",
      reviewers: [],
      sourceBranch: "feature/invoice-lines-v2-bff-mock",
      destinationBranch: "develop",
      sourceCommitHash: "2222222222222222222222222222222222222222",
      destinationCommitHash: "1111111111111111111111111111111111111111",
      createdOn: "",
      updatedOn: "",
    };

    expect(() =>
      buildFindingPublicationRequest({
        provider: "bitbucket",
        workspace: "example-workspace",
        repo: "backend-api",
        pr,
        reviewRun: activeRun,
        draft,
      }),
    ).toThrow("does not belong to the reviewed head");

    expect(() =>
      buildFindingPublicationRequest({
        provider: "bitbucket",
        workspace: "example-workspace",
        repo: "backend-api",
        pr,
        reviewRun: activeRun,
        draft: {
          ...draft,
          reviewBaseSha: "4444444444444444444444444444444444444444",
          reviewHeadSha: activeRun.reviewedHeadSha ?? null,
        },
      }),
    ).toThrow("does not belong to the reviewed base");

    expect(() =>
      buildFindingPublicationRequest({
        provider: "github",
        workspace: "other-workspace",
        repo: "backend-api",
        pr,
        reviewRun: activeRun,
        draft: {
          ...draft,
          reviewBaseSha: activeRun.reviewedBaseSha ?? null,
          reviewHeadSha: activeRun.reviewedHeadSha ?? null,
        },
      }),
    ).toThrow("does not match the structured review run");
  });
});

describe("assertPullRequestMatchesReviewRun", () => {
  const pr: PullRequestDetail = {
    id: 1020,
    title: "Invoice lines",
    descriptionRaw: "",
    state: "OPEN",
    draft: false,
    authorDisplayName: "Reviewer",
    reviewers: [],
    sourceBranch: activeRun.sourceBranch,
    destinationBranch: activeRun.destinationBranch,
    sourceCommitHash: activeRun.reviewedHeadSha ?? null,
    destinationCommitHash: activeRun.reviewedBaseSha ?? null,
    createdOn: "",
    updatedOn: "",
  };

  it("accepts only the exact provider revision used by the review run", () => {
    expect(() => assertPullRequestMatchesReviewRun(activeRun, pr)).not.toThrow();
    expect(() =>
      assertPullRequestMatchesReviewRun(activeRun, {
        ...pr,
        destinationCommitHash: "4444444444444444444444444444444444444444",
      }),
    ).toThrow("changed after this review");
  });
});

describe("latestReviewFindingFingerprintsForRevision", () => {
  it("uses the newest successful run for a reviewed revision", () => {
    const latestRun: ReviewRun = {
      ...activeRun,
      id: "run-latest",
      findings: [
        {
          ...activeRun.findings[0]!,
          fingerprint: "latest-finding",
        },
      ],
    };

    expect(
      latestReviewFindingFingerprintsForRevision(
        {
          activeThreadId: null,
          threads: [],
          reviewRuns: [activeRun, latestRun],
        },
        activeRun.reviewedBaseSha ?? "",
        activeRun.reviewedHeadSha ?? "",
      ),
    ).toEqual(new Set(["latest-finding"]));
  });
});

describe("summarizeActiveReviewFindings", () => {
  it("projects current and historical publication state onto the active run", () => {
    const summary = summarizeActiveReviewFindings(store, activeRun);

    expect(summary.get("run-1-finding-1")).toMatchObject({
      alreadyPublished: false,
      historicalPublishedCount: 1,
      currentPublishedCount: 0,
      currentDraftCount: 0,
      staleAnchor: true,
      publicationMode: "inline",
      latestPublishedAt: "2026-06-22T20:10:00.000Z",
    });
    expect(latestTrackedFindingCommentId(store, "fingerprint-1")).toBe("101");
    expect(summary.get("run-1-finding-2")).toMatchObject({
      alreadyStaged: true,
      currentDraftCount: 1,
      historicalDraftCount: 0,
      alreadyPublished: false,
      staleAnchor: false,
    });
  });

  it("batches staged and absent history without resolving current unstaged findings", () => {
    const selected = selectTrackedFindingCommentsForBatch(
      [
        { findingFingerprint: "staged", commentId: "comment-1" },
        { findingFingerprint: "current-unstaged", commentId: "comment-2" },
        { findingFingerprint: "absent", commentId: "comment-3" },
      ],
      new Set(["staged", "current-unstaged"]),
      new Set(["staged"]),
    );

    expect(selected).toEqual([
      { findingFingerprint: "staged", commentId: "comment-1" },
      { findingFingerprint: "absent", commentId: "comment-3" },
    ]);
  });

  it("tracks only inline provider comments for reconciliation", () => {
    const generalRun: ReviewRun = {
      ...previousRun,
      id: "run-general",
      findings: [
        {
          ...previousRun.findings[0]!,
          fingerprint: "general-finding",
          publication: {
            mode: "general",
            draftIds: [],
            remoteCommentIds: ["general-comment"],
            publishedAt: "2026-06-22T20:11:00.000Z",
          },
        },
      ],
    };

    expect(
      latestTrackedFindingComments({
        activeThreadId: null,
        threads: [],
        reviewRuns: [previousRun, generalRun],
      }),
    ).toEqual([{ findingFingerprint: "fingerprint-1", commentId: "101" }]);
  });
});

describe("filterStageableAiReviewDraftComments", () => {
  it("allows historical findings to reconcile while skipping current drafts and local duplicates", () => {
    const publicationSummary = summarizeActiveReviewFindings(store, activeRun);
    const existingDrafts: Pick<DraftComment, "path" | "to" | "from" | "raw">[] = [
      {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        to: 77,
        from: null,
        raw: "Duplicate general note already staged locally.",
      },
    ];
    const comments: LinkedAiReviewDraftComment[] = [
      {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        to: 29,
        from: null,
        raw: "Add @HttpCode(HttpStatus.OK) here so the runtime matches the documented 200 response.",
        findingRef: {
          reviewRunId: "run-1",
          findingId: "run-1-finding-1",
          findingFingerprint: "fingerprint-1",
        },
        publicationMode: "inline",
      },
      {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        to: 44,
        from: null,
        raw: "Avoid dropping pages silently when hasMore and firstPage.pages disagree.",
        findingRef: {
          reviewRunId: "run-1",
          findingId: "run-1-finding-2",
          findingFingerprint: "fingerprint-2",
        },
        publicationMode: "inline",
      },
      {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        to: 77,
        from: null,
        raw: "Duplicate general note already staged locally.",
        findingRef: null,
        publicationMode: null,
      },
      {
        path: "src/app/modules/invoice-lines/invoice-lines-v2.controller.ts",
        to: 53,
        from: null,
        raw: "Consider validating the DTO example so it matches the runtime payload shape.",
        findingRef: null,
        publicationMode: null,
      },
    ];

    expect(
      filterStageableAiReviewDraftComments(comments, existingDrafts, publicationSummary),
    ).toEqual({
      stageable: [comments[0], comments[3]],
      skipped: 2,
      skippedAlreadyStaged: 1,
      skippedAlreadyPublished: 0,
      skippedExistingDrafts: 1,
    });
  });
});
