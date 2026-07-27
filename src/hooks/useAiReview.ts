import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { tauriCall } from "@/lib/tauri";
import type {
  AiProvider,
  AiReviewRunState,
  AiReviewStore,
  AiReviewThread,
  ClaudeReviewEffort,
  ClaudeReviewModel,
  CodexReviewEffort,
  ReviewRun,
} from "@/types";

interface StartReviewArgs {
  payload: string;
  displayMessage?: string | null;
  title: string;
  sourceBranch: string;
  destinationBranch: string;
  reviewKind?: "lineQuestion" | null;
  threadTitle?: string | null;
  aiProvider: AiProvider;
  claudeModel: ClaudeReviewModel | null;
  claudeEffort: ClaudeReviewEffort | null;
  codexModel: string | null;
  codexEffort: CodexReviewEffort | null;
  reviewProfile?: string | null;
}

interface ReplyReviewArgs {
  title: string;
  sourceBranch: string;
  destinationBranch: string;
  threadId: string;
  userMessage: string;
  basePayload: string;
  aiProvider: AiProvider;
  claudeModel: ClaudeReviewModel | null;
  claudeEffort: ClaudeReviewEffort | null;
  codexModel: string | null;
  codexEffort: CodexReviewEffort | null;
}

interface UseAiReviewResult {
  store: AiReviewStore | null;
  activeThread: AiReviewThread | null;
  activeRun: ReviewRun | null;
  state: AiReviewRunState | null;
  loading: boolean;
  error: string | null;
  refreshStore: () => Promise<AiReviewStore | null>;
  createThread: (title: string, initialMessage?: string | null) => Promise<AiReviewThread | null>;
  run: (args: StartReviewArgs) => Promise<void>;
  reply: (args: ReplyReviewArgs) => Promise<void>;
  cancel: () => Promise<void>;
  setActiveThread: (threadId: string) => Promise<void>;
  clearThread: (threadId: string) => Promise<void>;
}

function activeThreadFromStore(store: AiReviewStore | null): AiReviewThread | null {
  if (!store?.activeThreadId) return null;
  return store.threads.find((thread) => thread.id === store.activeThreadId) ?? null;
}

function activeRunFromStore(store: AiReviewStore | null): ReviewRun | null {
  if (!store?.activeThreadId) return null;
  const runs = store.reviewRuns ?? [];
  for (let index = runs.length - 1; index >= 0; index -= 1) {
    const run = runs[index];
    if (run?.threadId === store.activeThreadId) {
      return run;
    }
  }
  return null;
}

/**
 * Manages persisted AI review threads for a single PR plus the in-flight Claude run state.
 *
 * Review history lives on disk; the backend only tracks the currently running turn so the UI can
 * render progress, elapsed time, and cancellation safely across reloads.
 */
export function useAiReview(
  workspace: string | null,
  repo: string | null,
  prId: number | null,
): UseAiReviewResult {
  const [store, setStore] = useState<AiReviewStore | null>(null);
  const [state, setState] = useState<AiReviewRunState | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const keyRef = useRef<string | null>(null);
  const activeThread = useMemo(() => activeThreadFromStore(store), [store]);
  const activeRun = useMemo(() => activeRunFromStore(store), [store]);

  const loadStore = useCallback(async () => {
    if (!workspace || !repo || prId == null) {
      setStore(null);
      return null;
    }
    const key = `${workspace}/${repo}/${prId}`;
    keyRef.current = key;
    const result = await tauriCall<AiReviewStore | null>("load_ai_review_store", {
      workspace,
      repo,
      id: prId,
    });
    if (keyRef.current === key) {
      setStore(result ?? null);
    }
    return result ?? null;
  }, [workspace, repo, prId]);

  const loadState = useCallback(async () => {
    if (!workspace || !repo || prId == null) {
      setState(null);
      return null;
    }
    const key = `${workspace}/${repo}/${prId}`;
    keyRef.current = key;
    const next = await tauriCall<AiReviewRunState | null>("get_ai_review_run_state", {
      workspace,
      repo,
      id: prId,
    });
    if (keyRef.current === key) {
      setState(next);
    }
    return next;
  }, [workspace, repo, prId]);

  useEffect(() => {
    if (!workspace || !repo || prId == null) {
      setStore(null);
      setState(null);
      setError(null);
      return;
    }
    const key = `${workspace}/${repo}/${prId}`;
    keyRef.current = key;
    setError(null);
    void Promise.all([loadStore(), loadState()]);
  }, [workspace, repo, prId, loadStore, loadState]);

  useEffect(() => {
    if (state?.status !== "running") return;
    const timer = window.setInterval(() => {
      void loadState();
    }, 1000);
    return () => window.clearInterval(timer);
  }, [state, loadState]);

  useEffect(() => {
    if (!state) return;
    if (state.status === "failed") {
      setError(state.error ?? "The AI review failed.");
      return;
    }
    setError(null);
    if (state.status === "succeeded" || state.status === "cancelled") {
      void loadStore();
    }
  }, [state, loadStore]);

  const run = useCallback(
    async ({
      payload,
      displayMessage,
      title,
      sourceBranch,
      destinationBranch,
      reviewKind,
      threadTitle,
      aiProvider,
      claudeModel,
      claudeEffort,
      codexModel,
      codexEffort,
      reviewProfile,
    }: StartReviewArgs) => {
      if (!workspace || !repo || prId == null) return;
      const key = `${workspace}/${repo}/${prId}`;
      keyRef.current = key;
      setPending(true);
      setError(null);
      try {
        const result = await tauriCall<AiReviewRunState>("start_inline_review", {
          workspace,
          repo,
          id: prId,
          title,
          payload,
          displayMessage,
          sourceBranch,
          destinationBranch,
          reviewKind,
          threadTitle,
          aiProvider,
          claudeModel,
          claudeEffort,
          codexModel,
          codexEffort,
          reviewProfile: reviewProfile?.trim() || null,
          skipAnalyzers: true,
        });
        if (keyRef.current === key) {
          setState(result);
        }
        await loadStore();
      } catch (e) {
        if (keyRef.current === key) {
          setError(e instanceof Error ? e.message : String(e));
        }
      } finally {
        if (keyRef.current === key) {
          setPending(false);
        }
      }
    },
    [workspace, repo, prId, loadStore],
  );

  const reply = useCallback(
    async ({
      title,
      sourceBranch,
      destinationBranch,
      threadId,
      userMessage,
      basePayload,
      aiProvider,
      claudeModel,
      claudeEffort,
      codexModel,
      codexEffort,
    }: ReplyReviewArgs) => {
      if (!workspace || !repo || prId == null) return;
      const key = `${workspace}/${repo}/${prId}`;
      keyRef.current = key;
      setPending(true);
      setError(null);
      try {
        const result = await tauriCall<AiReviewRunState>("reply_inline_review", {
          workspace,
          repo,
          id: prId,
          title,
          sourceBranch,
          destinationBranch,
          threadId,
          userMessage,
          basePayload,
          aiProvider,
          claudeModel,
          claudeEffort,
          codexModel,
          codexEffort,
        });
        if (keyRef.current === key) {
          setState(result);
        }
        await loadStore();
      } catch (e) {
        if (keyRef.current === key) {
          setError(e instanceof Error ? e.message : String(e));
        }
      } finally {
        if (keyRef.current === key) {
          setPending(false);
        }
      }
    },
    [workspace, repo, prId, loadStore],
  );

  const cancel = useCallback(async () => {
    if (!workspace || !repo || prId == null) return;
    const key = `${workspace}/${repo}/${prId}`;
    keyRef.current = key;
    setPending(true);
    try {
      const result = await tauriCall<AiReviewRunState>("cancel_inline_review", {
        workspace,
        repo,
        id: prId,
      });
      if (keyRef.current === key) {
        setState(result);
      }
    } catch (e) {
      if (keyRef.current === key) {
        setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      if (keyRef.current === key) {
        setPending(false);
      }
    }
  }, [workspace, repo, prId]);

  const createThread = useCallback(
    async (title: string, initialMessage?: string | null) => {
      if (!workspace || !repo || prId == null) return null;
      const key = `${workspace}/${repo}/${prId}`;
      keyRef.current = key;
      setPending(true);
      setError(null);
      try {
        const next = await tauriCall<AiReviewStore>("create_ai_review_thread", {
          workspace,
          repo,
          id: prId,
          title,
          initialMessage,
        });
        if (keyRef.current === key) {
          setStore(next);
        }
        return activeThreadFromStore(next);
      } catch (e) {
        if (keyRef.current === key) {
          setError(e instanceof Error ? e.message : String(e));
        }
        return null;
      } finally {
        if (keyRef.current === key) {
          setPending(false);
        }
      }
    },
    [workspace, repo, prId],
  );

  const setActiveThread = useCallback(
    async (threadId: string) => {
      if (!workspace || !repo || prId == null) return;
      const key = `${workspace}/${repo}/${prId}`;
      keyRef.current = key;
      const next = await tauriCall<AiReviewStore>("set_active_ai_review_thread", {
        workspace,
        repo,
        id: prId,
        threadId,
      });
      if (keyRef.current === key) {
        setStore(next);
        setError(null);
      }
    },
    [workspace, repo, prId],
  );

  const clearThread = useCallback(
    async (threadId: string) => {
      if (!workspace || !repo || prId == null) return;
      const key = `${workspace}/${repo}/${prId}`;
      keyRef.current = key;
      const next = await tauriCall<AiReviewStore | null>("delete_ai_review_thread", {
        workspace,
        repo,
        id: prId,
        threadId,
      });
      if (keyRef.current === key) {
        setStore(next ?? null);
        setError(null);
      }
    },
    [workspace, repo, prId],
  );

  return {
    store,
    activeThread,
    activeRun,
    state,
    loading: pending || state?.status === "running",
    error,
    refreshStore: loadStore,
    createThread,
    run,
    reply,
    cancel,
    setActiveThread,
    clearThread,
  };
}
