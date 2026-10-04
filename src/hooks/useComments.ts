import { useCallback, useEffect, useRef, useState } from "react";
import { tauriCall } from "@/lib/tauri";
import type { PrComment, ReviewProvider } from "@/types";

interface UseCommentsResult {
  comments: PrComment[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/** Loads all comments for a PR via IPC. */
export function useComments(
  provider: ReviewProvider | null,
  workspace: string | null,
  repo: string | null,
  prId: number | null,
): UseCommentsResult {
  const [comments, setComments] = useState<PrComment[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);

  const refresh = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;

    if (prId == null || !workspace || !repo) {
      setComments([]);
      setLoading(false);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const next = await tauriCall<PrComment[]>("list_comments", {
        provider,
        workspace,
        repo,
        id: prId,
      });
      if (requestId !== requestIdRef.current) {
        return;
      }
      setComments(next);
    } catch (e) {
      if (requestId !== requestIdRef.current) {
        return;
      }
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, [provider, workspace, repo, prId]);

  useEffect(() => {
    // Drop comments from the previous target immediately so a stale response
    // cannot keep showing them while the new request is in flight.
    setComments([]);
    setLoading(false);
    setError(null);
    void refresh();
    return () => {
      // Invalidate any in-flight request when the target changes or on unmount.
      requestIdRef.current += 1;
    };
  }, [refresh]);

  return { comments, loading, error, refresh };
}
