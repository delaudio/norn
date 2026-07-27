import { describe, expect, it } from "vitest";
import type { FindingPublicationRequest } from "@/types";
import { publishMockReviewFinding } from "./mock-handlers";

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
    ).toThrow("head changed");
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
});
