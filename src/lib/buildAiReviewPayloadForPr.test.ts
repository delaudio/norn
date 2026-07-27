import { describe, expect, it } from "vitest";
import type { PullRequestDetail } from "@/types";
import {
  assertStablePullRequestSnapshot,
  resolveLineQuestionHunkFromReviewSnapshot,
} from "./buildAiReviewPayloadForPr";

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
