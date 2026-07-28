import { beforeEach, describe, expect, it, vi } from "vitest";
import { loadReviewReferences } from "@/lib/reviewReferencesStorage";
import { tauriCall } from "@/lib/tauri";
import type { PullRequestDetail, ReviewReference } from "@/types";
import {
  assertStablePullRequestSnapshot,
  buildAiReviewPayloadForPr,
  resolveLineQuestionHunkFromReviewSnapshot,
} from "./buildAiReviewPayloadForPr";

vi.mock("@/lib/tauri", () => ({
  tauriCall: vi.fn(),
}));
vi.mock("@/lib/reviewReferencesStorage", () => ({
  loadReviewReferences: vi.fn(),
}));

const detail: PullRequestDetail = {
  id: 42,
  title: "Review publication",
  descriptionRaw: "",
  state: "OPEN",
  draft: false,
  authorDisplayName: "Reviewer",
  reviewers: [],
  sourceBranch: "feature/publication",
  destinationBranch: "main",
  sourceCommitHash: "2222222222222222222222222222222222222222",
  destinationCommitHash: "1111111111111111111111111111111111111111",
  createdOn: "",
  updatedOn: "",
};

describe("assertStablePullRequestSnapshot", () => {
  it("accepts the same immutable review target", () => {
    expect(() =>
      assertStablePullRequestSnapshot(detail, {
        ...detail,
        sourceCommitHash: detail.sourceCommitHash?.toUpperCase(),
      }),
    ).not.toThrow();
  });

  it("rejects source, destination, or branch drift around diff loading", () => {
    for (const changed of [
      { ...detail, sourceCommitHash: "3333333333333333333333333333333333333333" },
      { ...detail, destinationCommitHash: "4444444444444444444444444444444444444444" },
      { ...detail, destinationBranch: "release" },
    ]) {
      expect(() => assertStablePullRequestSnapshot(detail, changed)).toThrow(
        "changed while its review snapshot was loading",
      );
    }
  });

  it("rejects snapshots that omit either provider commit id", () => {
    for (const changed of [
      { ...detail, sourceCommitHash: null },
      { ...detail, destinationCommitHash: null },
    ]) {
      expect(() => assertStablePullRequestSnapshot(changed, changed)).toThrow(
        "changed while its review snapshot was loading",
      );
    }
  });
});

describe("resolveLineQuestionHunkFromReviewSnapshot", () => {
  const rawDiff = [
    "diff --git a/src/review.ts b/src/review.ts",
    "--- a/src/review.ts",
    "+++ b/src/review.ts",
    "@@ -10,2 +10,2 @@",
    "-const state = 'old';",
    "+const state = 'new';",
    " keep();",
  ].join("\n");

  it("accepts a selected line that still exists at the same anchor", () => {
    expect(
      resolveLineQuestionHunkFromReviewSnapshot(rawDiff, {
        path: "src/review.ts",
        side: "new",
        to: 10,
        from: null,
        lineText: "const state = 'new';",
      }),
    ).toContain(" keep();");
  });

  it("rebuilds surrounding context from the verified snapshot", () => {
    const currentDiff = rawDiff.replace(" keep();", " changedContext();");
    const resolved = resolveLineQuestionHunkFromReviewSnapshot(currentDiff, {
      path: "src/review.ts",
      side: "new",
      to: 10,
      from: null,
      lineText: "const state = 'new';",
    });
    expect(resolved).toContain(" changedContext();");
    expect(resolved).not.toContain(" keep();");
  });

  it("rejects a selected line whose content or anchor changed", () => {
    expect(() =>
      resolveLineQuestionHunkFromReviewSnapshot(rawDiff, {
        path: "src/review.ts",
        side: "new",
        to: 11,
        from: null,
        lineText: "const state = 'new';",
      }),
    ).toThrow("selected line changed");
  });
});

describe("buildAiReviewPayloadForPr", () => {
  beforeEach(() => {
    vi.mocked(tauriCall).mockReset();
    vi.mocked(loadReviewReferences).mockReset();
  });

  it("uses current review references instead of a stale persisted copy", async () => {
    const persistedReference: ReviewReference = {
      id: "persisted",
      type: "note",
      source: "manual",
      title: "Persisted reference",
      body: "Stale persisted guidance",
      createdAt: 1,
      updatedAt: 1,
    };
    const currentReference: ReviewReference = {
      ...persistedReference,
      id: "current",
      title: "Current reference",
      body: "Current unsaved guidance",
      updatedAt: 2,
    };
    vi.mocked(loadReviewReferences).mockReturnValue([persistedReference]);
    vi.mocked(tauriCall)
      .mockResolvedValueOnce(detail)
      .mockResolvedValueOnce(
        "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new",
      )
      .mockResolvedValueOnce({ ahead: 1, behind: 0 })
      .mockResolvedValueOnce(detail)
      .mockResolvedValueOnce(detail);

    const result = await buildAiReviewPayloadForPr({
      workspace: "acme",
      repo: "frontend",
      prId: detail.id,
      jiraBaseUrl: null,
      jiraContextEnabled: false,
      reviewReferences: [currentReference],
    });

    expect(result.payload).toContain("Current unsaved guidance");
    expect(result.payload).not.toContain("Stale persisted guidance");
    expect(loadReviewReferences).not.toHaveBeenCalled();
  });
});
