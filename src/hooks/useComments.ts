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
  const requestId = useRef(0);
  const currentSelection = useRef<string | null>(null);
  const selectionKey =
    provider != null && workspace != null && repo != null && prId != null
      ? `${provider}:${workspace}/${repo}/${prId}`
      : null;

  useEffect(() => {
    currentSelection.current = selectionKey;
  }, [selectionKey]);

  const refresh = useCallback(async () => {
    if (selectionKey == null) {
      setComments([]);
      setError(null);
      setLoading(false);
      return;
    }
    if (currentSelection.current !== selectionKey) return;

    const currentRequest = requestId.current + 1;
    requestId.current = currentRequest;
    setLoading(true);
    setError(null);
    try {
      const next = await tauriCall<PrComment[]>("list_comments", {
        provider,
        workspace,
        repo,
        id: prId,
      });
      if (requestId.current !== currentRequest || currentSelection.current !== selectionKey) return;
      setComments(next);
    } catch (e) {
      if (requestId.current !== currentRequest || currentSelection.current !== selectionKey) return;
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (requestId.current === currentRequest && currentSelection.current === selectionKey) {
        setLoading(false);
      }
    }
  }, [provider, workspace, repo, prId]);

  useEffect(() => {
    if (selectionKey == null) {
      requestId.current += 1;
      setComments([]);
      setLoading(false);
      setError(null);
      return;
    }
    setComments([]);
    setError(null);
    void refresh();
    return () => {
      requestId.current += 1;
    };
  }, [refresh]);

  return { comments, loading, error, refresh };
}
