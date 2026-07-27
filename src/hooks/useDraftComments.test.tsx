import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useDraftComments } from "./useDraftComments";

describe("useDraftComments structured finding publication", () => {
  it("keeps a published draft retryable until local publication tracking succeeds", async () => {
    const publishFindingDraft = vi.fn().mockResolvedValue({
      id: "9223372036854775000",
      createdOn: "2026-07-27T20:00:00.000Z",
    });
    const onDraftPublished = vi
      .fn()
      .mockRejectedValueOnce(new Error("publication store unavailable"))
      .mockResolvedValueOnce(undefined);
    const { result } = renderHook(() =>
      useDraftComments("github", "publication-test", "retryable-payments", 42, {
        publishFindingDraft,
        onDraftPublished,
      }),
    );

    let localId = "";
    act(() => {
      localId =
        result.current.addDraft({
          path: "src/lib.ts",
          to: 12,
          from: null,
          raw: "Keep this finding actionable.",
          parentId: null,
          source: "aiFinding",
          findingRef: {
            reviewRunId: "run-1",
            findingId: "finding-1",
            findingFingerprint: "fingerprint-1",
          },
          publicationMode: "inline",
          reviewHeadSha: "2222222222222222222222222222222222222222",
        })?.localId ?? "";
    });

    await act(async () => {
      const first = await result.current.publishDraft(localId);
      expect(first.error).toBe("publication store unavailable");
    });
    expect(result.current.drafts).toHaveLength(1);

    await act(async () => {
      const retry = await result.current.publishDraft(localId);
      expect(retry.error).toBeNull();
      expect(retry.comment?.id).toBe("9223372036854775000");
    });
    expect(result.current.drafts).toHaveLength(0);
    expect(publishFindingDraft).toHaveBeenCalledTimes(2);
    expect(onDraftPublished).toHaveBeenCalledTimes(2);
  });
});
