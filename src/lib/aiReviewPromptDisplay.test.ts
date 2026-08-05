import { describe, expect, it } from "vitest";
import {
  buildReviewPromptDisplayMessage,
  parseReviewPromptDisplayMessage,
} from "@/lib/aiReviewPromptDisplay";

describe("aiReviewPromptDisplay", () => {
  it("round-trips the visible review request and full prompt", () => {
    const message = buildReviewPromptDisplayMessage("Full payload\nwith diff");

    expect(message).toContain("[[norn:ai-review-prompt]]");
    expect(message).not.toContain("[[lachesi:ai-review-prompt]]");
    expect(parseReviewPromptDisplayMessage(message)).toEqual({
      intro: "Run the standard AI review for this pull request.",
      prompt: "Full payload\nwith diff",
    });
  });

  it("reads the legacy prompt marker without emitting it", () => {
    expect(
      parseReviewPromptDisplayMessage(
        "Run the standard AI review.\n\n[[lachesi:ai-review-prompt]]\nLegacy payload",
      ),
    ).toEqual({
      intro: "Run the standard AI review.",
      prompt: "Legacy payload",
    });
  });

  it("ignores normal reviewer messages", () => {
    expect(parseReviewPromptDisplayMessage("Can you explain this finding?")).toBeNull();
  });
});
