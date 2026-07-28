import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { mockReviewEffectivenessReport } from "@/mock-tauri/fixtures";
import { useReviewEffectiveness } from "./useReviewEffectiveness";

const { tauriCallMock } = vi.hoisted(() => ({
  tauriCallMock: vi.fn(),
}));

vi.mock("@/lib/tauri", () => ({
  tauriCall: tauriCallMock,
}));

describe("useReviewEffectiveness", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    tauriCallMock.mockReset();
  });

  it("sends the selected repository and refreshes the rolling time range", async () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000);
    tauriCallMock.mockResolvedValue(mockReviewEffectivenessReport);

    const { result } = renderHook(() =>
      useReviewEffectiveness({
        enabled: true,
        provider: "bitbucket",
        repository: {
          provider: "bitbucket",
          workspace: "example-workspace",
          repo: "frontend-app",
        },
        days: 7,
      }),
    );

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(tauriCallMock).toHaveBeenLastCalledWith("get_review_effectiveness_metrics", {
      filter: {
        tenantId: "local",
        provider: "bitbucket",
        workspace: "example-workspace",
        repo: "frontend-app",
        fromMs: 2_000_000_000 - 7 * 24 * 60 * 60 * 1000,
        toMs: 2_000_000_000,
      },
    });

    now.mockReturnValue(2_100_000_000);
    await act(async () => {
      await result.current.refresh();
    });

    expect(tauriCallMock).toHaveBeenLastCalledWith("get_review_effectiveness_metrics", {
      filter: expect.objectContaining({
        fromMs: 2_100_000_000 - 7 * 24 * 60 * 60 * 1000,
        toMs: 2_100_000_000,
      }),
    });
  });
});
