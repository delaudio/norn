import { beforeEach, describe, expect, it, vi } from "vitest";
import { tauriCall } from "@/lib/tauri";
import { getReviewEffectivenessMetrics } from "./reviewMetricsService";

vi.mock("@/lib/tauri", () => ({
  tauriCall: vi.fn(),
}));

describe("reviewMetricsService", () => {
  beforeEach(() => {
    vi.mocked(tauriCall).mockReset();
  });

  it("owns the typed metrics command and payload", async () => {
    const filter = {
      tenantId: "local",
      provider: "bitbucket" as const,
      workspace: "example-workspace",
      repo: "frontend-app",
      fromMs: 1_000,
      toMs: 2_000,
    };
    vi.mocked(tauriCall).mockResolvedValue({ schemaVersion: "norn.review-effectiveness.v1" });

    await getReviewEffectivenessMetrics(filter);

    expect(tauriCall).toHaveBeenCalledWith("get_review_effectiveness_metrics", { filter });
  });
});
