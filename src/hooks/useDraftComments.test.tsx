import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useDraftComments } from "./useDraftComments";

describe("useDraftComments structured finding publication", () => {
  it("does not write a structured finding when local publication tracking is unavailable", async () => {
    const publishFindingDraft = vi.fn().mockResolvedValue({
      id: "9223372036854775000",
      createdOn: "2026-07-27T20:00:00.000Z",
    });
    const { result } = renderHook(() =>
      useDraftComments("github", "publication-test", "untracked-payments", 42, {
        publishFindingDraft,
      }),
    );

    let localId = "";
    act(() => {
      localId =
        result.current.addDraft({
          path: "src/lib.ts",
          to: 12,
          from: null,
          raw: "Do not publish this without durable tracking.",
          parentId: null,
          source: "aiFinding",
          findingRef: {
            reviewRunId: "run-1",
            findingId: "finding-1",
            findingFingerprint: "fingerprint-1",
          },
          publicationMode: "inline",
          reviewBaseSha: "1111111111111111111111111111111111111111",
          reviewHeadSha: "2222222222222222222222222222222222222222",
        })?.localId ?? "";
    });

    await act(async () => {
      const published = await result.current.publishDraft(localId);
      expect(published.error).toContain("tracking is unavailable");
    });

    expect(publishFindingDraft).not.toHaveBeenCalled();
    expect(result.current.drafts).toHaveLength(1);
  });

  it("keeps a published draft retryable until local publication tracking succeeds", async () => {
    const publishFindingDraft = vi.fn().mockResolvedValue({
      id: "9223372036854775000",
      createdOn: "2026-07-27T20:00:00.000Z",
    });
    const onFindingDraftPublished = vi
      .fn()
      .mockRejectedValueOnce(new Error("publication store unavailable"))
      .mockResolvedValueOnce(undefined);
    const { result } = renderHook(() =>
      useDraftComments("github", "publication-test", "retryable-payments", 42, {
        publishFindingDraft,
        onFindingDraftPublished,
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
          reviewBaseSha: "1111111111111111111111111111111111111111",
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
    expect(onFindingDraftPublished).toHaveBeenCalledTimes(2);
  });

  it("removes manual drafts immediately after their non-idempotent provider write", async () => {
    const onFindingDraftPublished = vi
      .fn()
      .mockRejectedValue(new Error("must not run for manual comments"));
    const { result } = renderHook(() =>
      useDraftComments("github", "publication-test", "manual-payments", 42, {
        onFindingDraftPublished,
      }),
    );

    let localId = "";
    act(() => {
      localId =
        result.current.addDraft({
          path: "src/lib.ts",
          to: 12,
          from: null,
          raw: "A manual review comment.",
          parentId: null,
          source: "manual",
          findingRef: null,
          publicationMode: null,
          reviewBaseSha: null,
          reviewHeadSha: null,
        })?.localId ?? "";
    });

    await act(async () => {
      const published = await result.current.publishDraft(localId);
      expect(published.error).toBeNull();
    });
    expect(result.current.drafts).toHaveLength(0);
    expect(onFindingDraftPublished).not.toHaveBeenCalled();
  });
});
