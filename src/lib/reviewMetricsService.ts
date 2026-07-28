import { tauriCall } from "@/lib/tauri";
import type { ReviewEffectivenessFilter, ReviewEffectivenessReport } from "@/types";

export function getReviewEffectivenessMetrics(
  filter: ReviewEffectivenessFilter,
): Promise<ReviewEffectivenessReport> {
  return tauriCall<ReviewEffectivenessReport>("get_review_effectiveness_metrics", { filter });
}
