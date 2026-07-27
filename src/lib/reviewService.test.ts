import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriCall } from "@/lib/tauri";
import type { FindingPublicationRequest, PublishedCommentIdentity } from "@/types";
import { publishReviewFinding } from "./reviewService";

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
});
