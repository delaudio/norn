import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { mockReviewEffectivenessReport } from "@/mock-tauri/fixtures";
import { useReviewEffectiveness } from "./useReviewEffectiveness";

const { getReviewEffectivenessMetricsMock } = vi.hoisted(() => ({
  getReviewEffectivenessMetricsMock: vi.fn(),
}));

vi.mock("@/lib/reviewMetricsService", () => ({
  getReviewEffectivenessMetrics: getReviewEffectivenessMetricsMock,
}));

describe("useReviewEffectiveness", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    getReviewEffectivenessMetricsMock.mockReset();
  });

  it("sends the selected repository and refreshes the rolling time range", async () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000);
    getReviewEffectivenessMetricsMock.mockResolvedValue(mockReviewEffectivenessReport);

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
    expect(getReviewEffectivenessMetricsMock).toHaveBeenLastCalledWith({
      tenantId: "local",
      provider: "bitbucket",
      workspace: "example-workspace",
      repo: "frontend-app",
      fromMs: 2_000_000_000 - 7 * 24 * 60 * 60 * 1000,
      toMs: 2_000_000_000,
    });

    now.mockReturnValue(2_100_000_000);
    await act(async () => {
      await result.current.refresh();
    });

    expect(getReviewEffectivenessMetricsMock).toHaveBeenLastCalledWith({
      fromMs: 2_100_000_000 - 7 * 24 * 60 * 60 * 1000,
      toMs: 2_100_000_000,
      tenantId: "local",
      provider: "bitbucket",
      workspace: "example-workspace",
      repo: "frontend-app",
    });
  });

  it("clears an in-flight state when disabled", async () => {
    let resolveRequest: (value: typeof mockReviewEffectivenessReport) => void = () => {};
    getReviewEffectivenessMetricsMock.mockReturnValue(
      new Promise((resolve) => {
        resolveRequest = resolve;
      }),
    );

    const { result, rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) =>
        useReviewEffectiveness({
          enabled,
          provider: "bitbucket",
          repository: null,
          days: 30,
        }),
      { initialProps: { enabled: true } },
    );

    await waitFor(() => expect(result.current.loading).toBe(true));
    rerender({ enabled: false });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.report).toBeNull();
    expect(result.current.error).toBeNull();

    resolveRequest(mockReviewEffectivenessReport);
  });
});
