import { useCallback, useEffect, useRef, useState } from "react";
import { getReviewEffectivenessMetrics } from "@/lib/reviewMetricsService";
import type {
  RepoRef,
  ReviewEffectivenessFilter,
  ReviewEffectivenessReport,
  ReviewProvider,
} from "@/types";

const DAY_MS = 24 * 60 * 60 * 1000;

interface UseReviewEffectivenessOptions {
  enabled: boolean;
  provider: ReviewProvider;
  repository: RepoRef | null;
  days: number | null;
}

export interface UseReviewEffectivenessResult {
  report: ReviewEffectivenessReport | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useReviewEffectiveness({
  enabled,
  provider,
  repository,
  days,
}: UseReviewEffectivenessOptions): UseReviewEffectivenessResult {
  const [report, setReport] = useState<ReviewEffectivenessReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestId = useRef(0);

  const buildFilter = useCallback((): ReviewEffectivenessFilter => {
    const toMs = Date.now();
    return {
      tenantId: "local",
      provider,
      workspace: repository?.workspace,
      repo: repository?.repo,
      fromMs: days == null ? undefined : toMs - days * DAY_MS,
      toMs,
    };
  }, [days, provider, repository?.repo, repository?.workspace]);

  const refresh = useCallback(async () => {
    if (!enabled) return;
    const currentRequest = requestId.current + 1;
    requestId.current = currentRequest;
    setLoading(true);
    setError(null);
    setReport(null);
    try {
      const next = await getReviewEffectivenessMetrics(buildFilter());
      if (requestId.current === currentRequest) setReport(next);
    } catch (nextError) {
      if (requestId.current !== currentRequest) return;
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      if (requestId.current === currentRequest) setLoading(false);
    }
  }, [buildFilter, enabled]);

  useEffect(() => {
    if (!enabled) {
      requestId.current += 1;
      setLoading(false);
      setError(null);
      setReport(null);
      return;
    }
    void refresh();
    return () => {
      requestId.current += 1;
    };
  }, [enabled, refresh]);

  return { report, loading, error, refresh };
}
