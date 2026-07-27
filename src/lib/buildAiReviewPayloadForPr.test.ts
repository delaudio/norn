import { describe, expect, it } from "vitest";
import type { PullRequestDetail } from "@/types";
import { assertStablePullRequestSnapshot } from "./buildAiReviewPayloadForPr";

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
