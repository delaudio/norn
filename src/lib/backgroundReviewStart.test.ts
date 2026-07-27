import { describe, expect, it } from "vitest";
import type { PullRequestDetail } from "@/types";
import { buildBackgroundReviewStartArgs } from "./backgroundReviewStart";

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
  destinationCommitHash: null,
  createdOn: "",
  updatedOn: "",
};

describe("buildBackgroundReviewStartArgs", () => {
  it("pins direct menu-bar reviews to the head loaded with their diff", () => {
    expect(
      buildBackgroundReviewStartArgs({
        workspace: "acme",
        repo: "payments",
        prId: 42,
        detail,
        payload: "review payload",
        aiProvider: "codex",
        claudeModel: null,
        claudeEffort: null,
        codexModel: "gpt-5",
        codexEffort: "high",
      }),
    ).toMatchObject({
      workspace: "acme",
      repo: "payments",
      id: 42,
      reviewedHeadSha: "2222222222222222222222222222222222222222",
      sourceBranch: "feature/publication",
      destinationBranch: "main",
      skipAnalyzers: true,
    });
  });
});
