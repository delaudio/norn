import { describe, expect, it } from "vitest";
import { mockRawDiff } from "@/mock-tauri/fixtures";
import type { AiReviewThread, ReviewRun } from "@/types";
import {
  buildAiReviewCommentDraftPayload,
  linkAiReviewDraftCommentsToFindings,
  normalizeAiReviewDraftComments,
} from "./aiReviewDraftComments";

const thread: AiReviewThread = {
  id: "thread-1",
  title: "Review",
  createdAt: "1750076400000",
  updatedAt: "1750076400000",
  claudeSessionId: "session-1",
  messages: [
    {
      id: "assistant-1",
      role: "assistant",
      content:
        "Bug: if `filterId` can be missing, this path should either guard or have a regression test.",
      createdAt: "1750076400000",
    },
    {
      id: "user-1",
      role: "user",
      content: "Focus only on actionable PR comments.",
      createdAt: "1750077400000",
    },
  ],
};

const reviewRun: ReviewRun = {
  id: "run-1",
  schemaVersion: "v0.1",
  provider: "bitbucket",
  workspace: "example-workspace",
  repo: "frontend-app",
  prId: 1731,
  sourceBranch: "APP-2066-saved-view-filter",
  destinationBranch: "develop",
  status: "succeeded",
  turnKind: "initial",
  reviewProfile: null,
  createdAt: "1750076400000",
  finishedAt: "1750076500000",
  diffFingerprint: "abc123",
  threadId: "thread-1",
  summaryMarkdown:
    "## Review\n\n🔴 Bugs / High Risk\n\n1. `buildRecordsUrlFromSavedView.ts:17` should guard when `filterId` is missing.\n\n## Resources\n\n- [TypeScript Utility Types](https://www.typescriptlang.org/docs/handbook/utility-types.html) — Utility types reference.",
  evidence: [
    {
      id: "run-1-evidence-conversation",
      kind: "conversation",
      source: "claude",
      title: "Assistant review output",
      summary: "Canonical assistant markdown captured for this review turn.",
      payload: "## Review",
    },
    {
      id: "run-1-evidence-doc-1",
      kind: "doc",
      source: "other",
      title: "TypeScript Utility Types",
      summary: "Utility types reference.",
      payload: "https://www.typescriptlang.org/docs/handbook/utility-types.html",
    },
  ],
  findings: [
    {
      id: "run-1-finding-1",
      fingerprint: "fingerprint-1",
      title: "Guard missing filterId before building the records URL",
      severity: "high",
      confidence: "high",
      category: "bug",
      status: "new",
      summary:
        "`buildRecordsUrlFromSavedView.ts:17` should guard when `filterId` is missing, otherwise the saved-view drill-down can still render empty results for valid records.",
      rationale: null,
      ruleId: null,
      source: "llm",
      anchor: {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        startLine: 17,
        endLine: null,
        side: "new",
      },
      suggestedFix: null,
      evidenceIds: ["run-1-evidence-conversation"],
      publication: null,
    },
  ],
};

describe("buildAiReviewCommentDraftPayload", () => {
  it("includes the JSON contract, transcript, and diff context", () => {
    const payload = buildAiReviewCommentDraftPayload({
      pr: {
        id: 1731,
        title: "APP-2066 - fix saved-view filter returning empty results",
        descriptionRaw: "Use the stable filter id in the generated records URL.",
        state: "OPEN",
        draft: false,
        authorDisplayName: "Alex Reviewer",
        reviewers: [],
        sourceBranch: "APP-2066-saved-view-filter",
        destinationBranch: "develop",
        createdOn: "2026-06-16T09:00:00.000Z",
        updatedOn: "2026-06-16T15:40:00.000Z",
      },
      thread,
      branchStatus: { behind: 1, ahead: 2, behindCapped: false, aheadCapped: false },
      rawDiff: mockRawDiff,
      jiraKeys: ["APP-2066"],
      jiraBaseUrl: "https://example.atlassian.net",
      jiraContext: "Ticket says the stable filter id is the source of truth.",
    });

    expect(payload).toContain("Return ONLY JSON matching this shape");
    expect(payload).toContain("## Review conversation");
    expect(payload).toContain("### Assistant");
    expect(payload).toContain("### Reviewer");
    expect(payload).toContain("## Diff");
    expect(payload).toContain("APP-2066: https://example.atlassian.net/browse/APP-2066");
  });

  it("prefers normalized findings when a structured review run is available", () => {
    const payload = buildAiReviewCommentDraftPayload({
      pr: {
        id: 1731,
        title: "APP-2066 - fix saved-view filter returning empty results",
        descriptionRaw: "Use the stable filter id in the generated records URL.",
        state: "OPEN",
        draft: false,
        authorDisplayName: "Alex Reviewer",
        reviewers: [],
        sourceBranch: "APP-2066-saved-view-filter",
        destinationBranch: "develop",
        createdOn: "2026-06-16T09:00:00.000Z",
        updatedOn: "2026-06-16T15:40:00.000Z",
      },
      thread,
      reviewRun,
      rawDiff: mockRawDiff,
    });

    expect(payload).toContain("## Structured review findings");
    expect(payload).toContain("Fingerprint: fingerprint-1");
    expect(payload).toContain(
      "Anchor: src/app/views/utils/buildRecordsUrlFromSavedView.ts:17 (new)",
    );
    expect(payload).toContain("## Assistant summary");
    expect(payload).not.toContain("## Review conversation");
  });
});

describe("normalizeAiReviewDraftComments", () => {
  it("keeps only suggestions that map to an actual changed line in the diff", () => {
    const result = normalizeAiReviewDraftComments(mockRawDiff, [
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 17,
        from: null,
        raw: "Please add a regression test showing `filterId` is always present here.",
      },
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 9999,
        from: null,
        raw: "This line does not exist in the diff.",
      },
      {
        path: "does/not/exist.ts",
        to: 17,
        from: null,
        raw: "Unknown file path.",
      },
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 17,
        from: null,
        raw: "Please add a regression test showing `filterId` is always present here.",
      },
    ]);

    expect(result.comments).toEqual([
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 17,
        from: null,
        raw: "Please add a regression test showing `filterId` is always present here.",
      },
    ]);
    expect(result.skipped).toBe(3);
  });
});

describe("linkAiReviewDraftCommentsToFindings", () => {
  it("links normalized draft comments back to the matching structured finding", () => {
    const linked = linkAiReviewDraftCommentsToFindings(reviewRun, [
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 17,
        from: null,
        raw: "Guard the `filterId` path here or add a regression test for the empty-results case.",
      },
    ]);

    expect(linked).toEqual([
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 17,
        from: null,
        raw: "Guard the `filterId` path here or add a regression test for the empty-results case.",
        findingRef: {
          reviewRunId: "run-1",
          findingId: "run-1-finding-1",
          findingFingerprint: "fingerprint-1",
        },
        publicationMode: "inline",
      },
    ]);
  });

  it("does not link or move a comment adjacent to a structured finding", () => {
    const linked = linkAiReviewDraftCommentsToFindings(reviewRun, [
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 16,
        from: null,
        raw: "This nearby comment may describe a different defect.",
      },
    ]);

    expect(linked).toEqual([
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 16,
        from: null,
        raw: "This nearby comment may describe a different defect.",
        findingRef: null,
        publicationMode: null,
      },
    ]);
  });

  it("leaves unmatched comments unlinked", () => {
    const linked = linkAiReviewDraftCommentsToFindings(reviewRun, [
      {
        path: "src/app/views/utils/buildRecordsUrlFromSavedView.ts",
        to: 42,
        from: null,
        raw: "This comment does not map cleanly to any stored finding.",
      },
    ]);

    expect(linked[0]?.findingRef).toBeNull();
    expect(linked[0]?.publicationMode).toBeNull();
  });
});
