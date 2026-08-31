import { useCallback, useEffect, useState } from "react";
import { tauriCall } from "@/lib/tauri";
import type { BranchSyncResult } from "@/types";

interface UseBranchSyncResult {
  loading: boolean;
  result: BranchSyncResult | null;
  error: string | null;
  sync: (sourceBranch: string, destinationBranch: string) => Promise<BranchSyncResult | null>;
  clear: () => void;
}

export function useBranchSync(
  workspace: string | null,
  repo: string | null,
  prId: number | null,
): UseBranchSyncResult {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<BranchSyncResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // A changed pull request selection invalidates results from the prior selection.
    void workspace;
    void repo;
    void prId;
    setLoading(false);
    setResult(null);
    setError(null);
  }, [workspace, repo, prId]);

  const sync = useCallback(
    async (sourceBranch: string, destinationBranch: string) => {
      if (!workspace || !repo || prId == null) return null;
      setLoading(true);
      setError(null);
      try {
        const next = await tauriCall<BranchSyncResult>("sync_pr_branch", {
          workspace,
          repo,
          id: prId,
          sourceBranch,
          destinationBranch,
        });
        setResult(next);
        return next;
      } catch (err) {
        setResult(null);
        setError(err instanceof Error ? err.message : String(err));
        return null;
      } finally {
        setLoading(false);
      }
    },
    [workspace, repo, prId],
  );

  const clear = useCallback(() => {
    setResult(null);
    setError(null);
  }, []);

  return { loading, result, error, sync, clear };
}
