import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppShell } from "@/components/AppShell";
import { type AppPaneId, BottomPaneBar } from "@/components/BottomPaneBar";
import { OverviewPanel } from "@/components/overview/OverviewPanel";
import { PrDetailPanel } from "@/components/pr-detail/PrDetailPanel";
import type { AuthorOption } from "@/components/pr-sidebar/AuthorFilter";
import { PrSidebar } from "@/components/pr-sidebar/PrSidebar";
import { RepositoryBranchesPanel } from "@/components/repositories/RepositoryBranchesPanel";
import { RepositoryExplorerPanel } from "@/components/repository-explorer/RepositoryExplorerPanel";
import { AiReviewPanel } from "@/components/review/AiReviewPanel";
import { ReviewHistoryPanel } from "@/components/review-history/ReviewHistoryPanel";
import { ShortcutsDialog } from "@/components/ShortcutsDialog";
import { SettingsPage, type SettingsSaveInput } from "@/components/settings/SettingsDialog";
import { ThemeToggle } from "@/components/ThemeToggle";
import { useAiReview } from "@/hooks/useAiReview";
import { useAiReviewFix } from "@/hooks/useAiReviewFix";
import { useAutomaticSyncPolling } from "@/hooks/useAutomaticSyncPolling";
import { useClosedPrAnalytics } from "@/hooks/useClosedPrAnalytics";
import { useConfig } from "@/hooks/useConfig";
import { useCredentials } from "@/hooks/useCredentials";
import { authorKey, useCurrentUser } from "@/hooks/useCurrentUser";
import {
  type PublishedDraftComment,
  type PublishFindingDraftsResult,
  useDraftComments,
} from "@/hooks/useDraftComments";
import { useMenuBarPrSync } from "@/hooks/useMenuBarPrSync";
import { type PrGroup, usePullRequests } from "@/hooks/usePullRequests";
import { useReviewReferences } from "@/hooks/useReviewReferences";
import { useReviewTerminals } from "@/hooks/useReviewTerminals";
import { useTheme } from "@/hooks/useTheme";
import {
  buildAiReviewCommentDraftPayload,
  linkAiReviewDraftCommentsToFindings,
  normalizeAiReviewDraftComments,
} from "@/lib/aiReviewDraftComments";
import { buildReviewPromptDisplayMessage } from "@/lib/aiReviewPromptDisplay";
import { buildBackgroundReviewStartArgs } from "@/lib/backgroundReviewStart";
import { buildAiFixPayload } from "@/lib/buildAiFixPayload";
import {
  buildAiReviewPayloadForPr,
  loadStablePullRequestReviewSnapshot,
  resolveLineQuestionHunkFromReviewSnapshot,
} from "@/lib/buildAiReviewPayloadForPr";
import { shouldIgnoreShortcut } from "@/lib/keyboard";
import {
  assertPullRequestMatchesReviewRun,
  buildFindingPublicationRequest,
  filterStageableAiReviewDraftComments,
  latestReviewFindingFingerprintsForRevision,
  latestTrackedFindingCommentId,
  latestTrackedFindingComments,
  selectTrackedFindingCommentsForBatch,
  summarizeActiveReviewFindings,
} from "@/lib/reviewFindingPublication";
import { reconcileReviewFindings, recordReviewFindingPublicationEvents } from "@/lib/reviewService";
import { tauriCall } from "@/lib/tauri";
import type {
  AiLineQuestionContext,
  AiReviewContext,
  AiReviewDraftCommentSuggestion,
  AiReviewJob,
  AiReviewJobStatus,
  AiReviewRunState,
  AppSelection,
  DraftComment,
  FindingReconciliationSummary,
  PrListFilter,
  PullRequestDetail,
  PullRequestSummary,
  RepoRef,
  RepoReviewConfigLoadResult,
  ReviewFindingPublicationEvent,
  ReviewProvider,
} from "@/types";
import { repoKey } from "@/types";

const ClosedPrAnalyticsPanel = lazy(() =>
  import("@/components/overview/ClosedPrAnalyticsPanel").then((module) => ({
    default: module.ClosedPrAnalyticsPanel,
  })),
);

const EMPTY_REPOS: RepoRef[] = [];

function lineQuestionLabel(context: AiLineQuestionContext): string {
  const line = context.to ?? context.from;
  return line == null ? context.path : `${context.path}:${line}`;
}

export default function App() {
  const { theme, toggle } = useTheme();
  const { config, saveConfig } = useConfig();
  const { testConnection, saveCredentials, saveGithubToken, saveJiraToken, saveNotionToken } =
    useCredentials();
  const { terminals: reviewTerminalOptions } = useReviewTerminals();
  const [filter, setFilter] = useState<PrListFilter>("OPEN");
  const [authorFilter, setAuthorFilter] = useState<string | null>(null);
  const [repositoryFilter, setRepositoryFilter] = useState<string | null>(null);
  const [selection, setSelection] = useState<AppSelection>({ kind: "pr-list" });
  const [helpOpen, setHelpOpen] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [repositoriesPanelOpen, setRepositoriesPanelOpen] = useState(false);
  const [reviewHistoryPanelOpen, setReviewHistoryPanelOpen] = useState(false);
  const [repositoryExplorerOpen, setRepositoryExplorerOpen] = useState(false);
  const [detailPaneOpen, setDetailPaneOpen] = useState(true);
  const [reviewPanelOpen, setReviewPanelOpen] = useState(false);
  const [reviewPanelExpanded, setReviewPanelExpanded] = useState(false);
  const [aiReviewContext, setAiReviewContext] = useState<AiReviewContext | null>(null);
  const [backgroundReviewPrKey, setBackgroundReviewPrKey] = useState<string | null>(null);
  const [reviewProfiles, setReviewProfiles] = useState<string[]>([]);
  const [selectedReviewProfile, setSelectedReviewProfile] = useState("");
  const pendingReviewThreadIdRef = useRef<string | null>(null);

  const reviewProvider = config?.reviewProvider ?? "bitbucket";
  const repos = config?.repos ?? EMPTY_REPOS;
  const activeRepos = useMemo(
    () => repos.filter((repo) => (repo.provider ?? "bitbucket") === reviewProvider),
    [repos, reviewProvider],
  );
  const reposKey = activeRepos.map(repoKey).join("|");
  const { groups, loading, refresh, loadMore } = usePullRequests(activeRepos, filter);
  const closedPrAnalytics = useClosedPrAnalytics(activeRepos);
  const currentUser = useCurrentUser(activeRepos.length > 0, reviewProvider);
  const activeSel = selection.kind === "pr" ? selection : null;
  const activeRepo = activeSel
    ? (repos.find(
        (repo) =>
          (repo.provider ?? "bitbucket") === reviewProvider &&
          repo.workspace === activeSel.workspace &&
          repo.repo === activeSel.repo,
      ) ?? null)
    : null;
  const aiReview = useAiReview(
    activeSel?.workspace ?? null,
    activeSel?.repo ?? null,
    activeSel?.prId ?? null,
  );
  const aiReviewFix = useAiReviewFix(
    activeSel?.workspace ?? null,
    activeSel?.repo ?? null,
    activeSel?.prId ?? null,
    aiReview.activeThread?.id ?? null,
  );
  const activeFindingPublication = useMemo(
    () => summarizeActiveReviewFindings(aiReview.store, aiReview.activeRun),
    [aiReview.store, aiReview.activeRun],
  );
  const aiReviewStore = aiReview.store;
  const setActiveAiReviewThread = aiReview.setActiveThread;

  useEffect(() => {
    const repoPath = activeRepo?.localPath?.trim();
    if (!repoPath) {
      setReviewProfiles([]);
      setSelectedReviewProfile("");
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const result = await tauriCall<RepoReviewConfigLoadResult>("validate_repo_review_config", {
          repoPath,
        });
        if (cancelled) return;
        const profiles = Object.keys(result.config?.profiles ?? {});
        setReviewProfiles(profiles);
        setSelectedReviewProfile((current) => {
          if (current && profiles.includes(current)) return current;
          return result.selectedProfile ?? "";
        });
      } catch {
        if (!cancelled) {
          setReviewProfiles([]);
          setSelectedReviewProfile("");
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [activeRepo?.localPath]);

  const selectPullRequest = useCallback((pr: { workspace: string; repo: string; id: number }) => {
    setRepositoriesPanelOpen(false);
    setReviewHistoryPanelOpen(false);
    setDetailPaneOpen(true);
    setSelection({
      kind: "pr",
      workspace: pr.workspace,
      repo: pr.repo,
      prId: pr.id,
      activeFilePath: null,
      activeFileLine: null,
    });
  }, []);

  const notifyReviewFinished = useCallback(
    async (title: string, body: string) => {
      if (!config?.notificationsEnabled) return;
      try {
        const { isPermissionGranted, requestPermission, sendNotification } = await import(
          "@tauri-apps/plugin-notification"
        );
        let permissionGranted = await isPermissionGranted();
        if (!permissionGranted) {
          permissionGranted = (await requestPermission()) === "granted";
        }
        if (permissionGranted) sendNotification({ title, body });
      } catch (error) {
        console.error("Failed to send AI review notification:", error);
      }
    },
    [config?.notificationsEnabled],
  );

  const runBackgroundMenuReview = useCallback(
    async (pr: PullRequestSummary) => {
      const key = `${pr.workspace}/${pr.repo}#${pr.id}`;
      if (backgroundReviewPrKey) return;
      setBackgroundReviewPrKey(key);
      let job: AiReviewJob | null = null;
      const updateJob = async (
        status: AiReviewJobStatus,
        threadId?: string | null,
        error?: string | null,
      ) => {
        if (!job) return;
        job = await tauriCall<AiReviewJob>("update_ai_review_job_status", {
          jobId: job.id,
          status,
          threadId: threadId ?? null,
          error: error ?? null,
        });
      };
      try {
        const repoConfig =
          repos.find((item) => item.workspace === pr.workspace && item.repo === pr.repo) ?? null;
        const { payload, pr: detail } = await buildAiReviewPayloadForPr({
          workspace: pr.workspace,
          repo: pr.repo,
          provider: repoConfig?.provider ?? reviewProvider,
          prId: pr.id,
          repoConfig,
          jiraBaseUrl: config?.jiraBaseUrl ?? null,
          jiraContextEnabled: Boolean(config?.hasJira && config?.jiraBaseUrl),
          reviewProfile: null,
        });
        job = await tauriCall<AiReviewJob>("create_ai_review_job", {
          workspace: pr.workspace,
          repo: pr.repo,
          prId: pr.id,
          prTitle: detail.title || pr.title || `PR #${pr.id}`,
          sourceBranch: detail.sourceBranch,
          destinationBranch: detail.destinationBranch,
          trigger: "menuBar",
        });
        const started = await tauriCall<AiReviewRunState>("start_inline_review", {
          ...buildBackgroundReviewStartArgs({
            workspace: pr.workspace,
            repo: pr.repo,
            prId: pr.id,
            detail,
            payload,
            aiProvider: config?.aiProvider ?? "claude",
            claudeModel: config?.claudeModel ?? null,
            claudeEffort: config?.claudeEffort ?? null,
            codexModel: config?.codexModel ?? null,
            codexEffort: config?.codexEffort ?? null,
          }),
          skipAnalyzers: true,
        });
        await updateJob("running", started.threadId);

        let finalState: AiReviewRunState | null = null;
        for (let attempt = 0; attempt < 60 * 30; attempt += 1) {
          await new Promise((resolve) => window.setTimeout(resolve, 1000));
          finalState = await tauriCall<AiReviewRunState | null>("get_ai_review_run_state", {
            workspace: pr.workspace,
            repo: pr.repo,
            id: pr.id,
          });
          if (finalState?.status !== "running") break;
        }

        if (
          activeSel?.workspace === pr.workspace &&
          activeSel.repo === pr.repo &&
          activeSel.prId === pr.id
        ) {
          await aiReview.refreshStore();
        }

        if (finalState?.status === "succeeded") {
          await updateJob("succeeded", finalState.threadId);
          await notifyReviewFinished("AI review finished", `${pr.repo} #${pr.id}: ${pr.title}`);
        } else if (finalState?.status === "failed") {
          await updateJob("failed", finalState.threadId, finalState.error);
          await notifyReviewFinished(
            "AI review failed",
            finalState.error || `${pr.repo} #${pr.id}: ${pr.title}`,
          );
        } else if (finalState?.status === "cancelled") {
          await updateJob("cancelled", finalState.threadId);
          await notifyReviewFinished("AI review cancelled", `${pr.repo} #${pr.id}: ${pr.title}`);
        } else {
          await updateJob(
            "failed",
            finalState?.threadId,
            "AI review did not finish before timeout.",
          );
        }
      } catch (error) {
        await updateJob("failed", null, error instanceof Error ? error.message : String(error));
        await notifyReviewFinished(
          "AI review failed",
          error instanceof Error ? error.message : String(error),
        );
      } finally {
        setBackgroundReviewPrKey(null);
      }
    },
    [
      activeSel,
      aiReview.refreshStore,
      backgroundReviewPrKey,
      config?.aiProvider,
      config?.claudeEffort,
      config?.claudeModel,
      config?.codexEffort,
      config?.codexModel,
      config?.hasJira,
      config?.jiraBaseUrl,
      notifyReviewFinished,
      repos,
      reviewProvider,
    ],
  );

  useMenuBarPrSync({
    groups,
    loading,
    menuBarSyncEnabled: config?.menuBarSyncEnabled ?? true,
    notificationsEnabled: config?.notificationsEnabled ?? false,
    onSync: refresh,
    onOpenPr: selectPullRequest,
    onReviewPr: runBackgroundMenuReview,
    reviewingPrKey: backgroundReviewPrKey,
  });

  useAutomaticSyncPolling({
    enabled: activeRepos.length > 0,
    intervalSeconds: config?.automaticSyncIntervalSeconds ?? null,
    contextKey: `${reposKey}:${filter}:${
      activeSel ? `${activeSel.workspace}/${activeSel.repo}#${activeSel.prId}` : "no-active-pr"
    }`,
    onSync: refresh,
  });

  const recordFindingPublicationEvents = async (
    events: ReviewFindingPublicationEvent[],
  ): Promise<void> => {
    if (!activeSel || events.length === 0) return;
    await recordReviewFindingPublicationEvents({
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      prId: activeSel.prId,
      events,
    });
    await aiReview.refreshStore();
  };

  const stageFindingDrafts = async (drafts: DraftComment[]): Promise<void> => {
    const events: ReviewFindingPublicationEvent[] = [];
    for (const draft of drafts) {
      if (!draft.findingRef) continue;
      events.push({
        kind: "stageDraft",
        reviewRunId: draft.findingRef.reviewRunId,
        findingFingerprint: draft.findingRef.findingFingerprint,
        mode: draft.publicationMode ?? "inline",
        draftId: draft.localId,
        remoteCommentId: null,
        publishedAt: null,
      });
    }
    await recordFindingPublicationEvents(events);
  };

  const removeFindingDrafts = async (drafts: DraftComment[]): Promise<void> => {
    const events: ReviewFindingPublicationEvent[] = [];
    for (const draft of drafts) {
      if (!draft.findingRef) continue;
      events.push({
        kind: "removeDraft",
        reviewRunId: draft.findingRef.reviewRunId,
        findingFingerprint: draft.findingRef.findingFingerprint,
        mode: draft.publicationMode ?? "inline",
        draftId: draft.localId,
        remoteCommentId: null,
        publishedAt: null,
      });
    }
    await recordFindingPublicationEvents(events);
  };

  const removeFindingDraft = async (draft: DraftComment): Promise<void> => {
    await removeFindingDrafts([draft]);
  };

  const publishStructuredFindingDraft = async (
    draft: DraftComment,
  ): Promise<PublishedDraftComment> => {
    if (!activeSel || !aiReviewContext || !draft.findingRef) {
      throw new Error("The structured finding publication context is unavailable.");
    }
    const reviewRun =
      aiReview.store?.reviewRuns?.find(
        (candidate) => candidate.id === draft.findingRef?.reviewRunId,
      ) ?? null;
    if (!reviewRun) {
      throw new Error("The review run linked to this draft is no longer available.");
    }
    const snapshot = await loadStablePullRequestReviewSnapshot({
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      provider: activeRepo?.provider ?? reviewProvider,
      prId: activeSel.prId,
    });
    assertPullRequestMatchesReviewRun(reviewRun, snapshot.pr);
    const request = buildFindingPublicationRequest({
      provider: activeRepo?.provider ?? reviewProvider,
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      pr: snapshot.pr,
      reviewRun,
      draft,
    });
    const trackedCommentId = latestTrackedFindingCommentId(
      aiReview.store,
      request.findingFingerprint,
    );
    const reconciliation = await reconcileReviewFindings({
      schemaVersion: "v1",
      tenantId: request.tenantId,
      provider: request.provider,
      workspace: request.workspace,
      repository: request.repository,
      pullRequestId: request.pullRequestId,
      baseSha: request.baseSha,
      headSha: request.headSha,
      trackedComments: trackedCommentId
        ? [
            {
              findingFingerprint: request.findingFingerprint,
              commentId: trackedCommentId,
            },
          ]
        : [],
      currentFindings: [request],
    });
    const action = reconciliation.actions.find(
      (candidate) => candidate.findingFingerprint === request.findingFingerprint,
    );
    if (!action || action.kind === "failed" || !action.commentId) {
      throw new Error(
        action?.error?.message ?? "The structured finding was not reconciled with the provider.",
      );
    }
    return {
      id: action.commentId,
      createdOn: new Date().toISOString(),
    };
  };

  const recordPublishedFindingDraft = async (
    draft: DraftComment,
    comment: PublishedDraftComment,
  ): Promise<void> => {
    if (!draft.findingRef) return;
    await recordFindingPublicationEvents([
      {
        kind: "publishDraft",
        reviewRunId: draft.findingRef.reviewRunId,
        findingFingerprint: draft.findingRef.findingFingerprint,
        mode: draft.publicationMode ?? "inline",
        draftId: draft.localId,
        remoteCommentId: comment.id,
        publishedAt: comment.createdOn || null,
      },
    ]);
  };

  const publishStructuredFindingDrafts = async (
    drafts: DraftComment[],
  ): Promise<PublishFindingDraftsResult> => {
    if (!activeSel || !aiReviewContext || drafts.length === 0) {
      throw new Error("The structured finding reconciliation context is unavailable.");
    }
    const snapshot = await loadStablePullRequestReviewSnapshot({
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      provider: activeRepo?.provider ?? reviewProvider,
      prId: activeSel.prId,
    });
    const comments = new Map<string, PublishedDraftComment>();
    const failures = new Map<string, string>();
    const errors: string[] = [];
    const requests: Array<{
      draft: DraftComment;
      request: ReturnType<typeof buildFindingPublicationRequest>;
    }> = [];
    for (const draft of drafts) {
      try {
        const reviewRun =
          aiReview.store?.reviewRuns?.find(
            (candidate) => candidate.id === draft.findingRef?.reviewRunId,
          ) ?? null;
        if (!reviewRun) {
          throw new Error("A review run linked to the staged finding is no longer available.");
        }
        assertPullRequestMatchesReviewRun(reviewRun, snapshot.pr);
        requests.push({
          draft,
          request: buildFindingPublicationRequest({
            provider: activeRepo?.provider ?? reviewProvider,
            workspace: activeSel.workspace,
            repo: activeSel.repo,
            pr: snapshot.pr,
            reviewRun,
            draft,
          }),
        });
      } catch (error) {
        failures.set(draft.localId, error instanceof Error ? error.message : String(error));
      }
    }

    const batches = new Map<string, typeof requests>();
    for (const entry of requests) {
      const key = `${entry.request.baseSha.toLowerCase()}:${entry.request.headSha.toLowerCase()}`;
      const batch = batches.get(key);
      if (batch) batch.push(entry);
      else batches.set(key, [entry]);
    }

    for (const batch of batches.values()) {
      const first = batch[0];
      if (!first) continue;
      const stagedFingerprints = new Set(batch.map(({ request }) => request.findingFingerprint));
      const currentFingerprints = latestReviewFindingFingerprintsForRevision(
        aiReview.store,
        first.request.baseSha,
        first.request.headSha,
      );
      for (const fingerprint of stagedFingerprints) currentFingerprints.add(fingerprint);
      const trackedComments = selectTrackedFindingCommentsForBatch(
        latestTrackedFindingComments(aiReview.store),
        currentFingerprints,
        stagedFingerprints,
      );
      let reconciliation: FindingReconciliationSummary;
      try {
        reconciliation = await reconcileReviewFindings({
          schemaVersion: "v1",
          tenantId: first.request.tenantId,
          provider: first.request.provider,
          workspace: first.request.workspace,
          repository: first.request.repository,
          pullRequestId: first.request.pullRequestId,
          baseSha: first.request.baseSha,
          headSha: first.request.headSha,
          trackedComments,
          currentFindings: batch.map(({ request }) => request),
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        for (const { draft } of batch) failures.set(draft.localId, message);
        continue;
      }

      for (const { draft, request } of batch) {
        const action = reconciliation.actions.find(
          (candidate) => candidate.findingFingerprint === request.findingFingerprint,
        );
        if (!action || action.kind === "failed" || !action.commentId) {
          failures.set(
            draft.localId,
            action?.error?.message ?? "A structured finding was not reconciled with the provider.",
          );
          continue;
        }
        comments.set(draft.localId, {
          id: action.commentId,
          createdOn: new Date().toISOString(),
        });
      }
      errors.push(
        ...reconciliation.actions
          .filter(
            (action) =>
              action.kind === "failed" && !stagedFingerprints.has(action.findingFingerprint),
          )
          .map(
            (action) =>
              action.error?.message ??
              `Finding ${action.findingFingerprint} could not be reconciled with the provider.`,
          ),
      );
    }
    return { comments, failures, errors };
  };

  const draftComments = useDraftComments(
    activeRepo?.provider ?? reviewProvider,
    activeSel?.workspace ?? null,
    activeSel?.repo ?? null,
    activeSel?.prId ?? null,
    {
      publishFindingDraft: publishStructuredFindingDraft,
      publishFindingDrafts: publishStructuredFindingDrafts,
      onFindingDraftPublished: recordPublishedFindingDraft,
      onDraftRemoved: removeFindingDraft,
      onDraftsDiscarded: async (drafts) => {
        await removeFindingDrafts(drafts);
      },
    },
  );
  const reviewReferences = useReviewReferences(
    activeSel?.workspace ?? null,
    activeSel?.repo ?? null,
    activeSel?.prId ?? null,
  );
  const meKey = currentUser ? authorKey(currentUser.accountId, currentUser.displayName) : null;

  // First run (or all repos removed): nudge the user to configure.
  useEffect(() => {
    if (config && activeRepos.length === 0) setSelection({ kind: "settings" });
  }, [activeRepos.length, config]);

  useEffect(() => {
    if (
      selection.kind === "overview" ||
      selection.kind === "closed-analytics" ||
      selection.kind === "settings"
    ) {
      setRepositoriesPanelOpen(false);
      setReviewHistoryPanelOpen(false);
      setRepositoryExplorerOpen(false);
    }
    if (!activeSel) {
      setAiReviewContext(null);
      setReviewPanelOpen(false);
      setReviewPanelExpanded(false);
      return;
    }
    setAiReviewContext(null);
    setReviewPanelExpanded(false);
  }, [activeSel, selection.kind]);

  useEffect(() => {
    const pendingReviewThreadId = pendingReviewThreadIdRef.current;
    if (!pendingReviewThreadId || !activeSel || !aiReviewStore) return;
    const exists = aiReviewStore.threads.some((thread) => thread.id === pendingReviewThreadId);
    if (!exists) {
      pendingReviewThreadIdRef.current = null;
      return;
    }
    void setActiveAiReviewThread(pendingReviewThreadId).finally(() => {
      pendingReviewThreadIdRef.current = null;
    });
  }, [activeSel, aiReviewStore, setActiveAiReviewThread]);

  // Distinct authors across the loaded PRs, with the current user pinned first.
  const authors: AuthorOption[] = (() => {
    const map = new Map<string, AuthorOption>();
    for (const group of groups) {
      for (const pr of group.pullRequests) {
        const key = authorKey(pr.authorAccountId, pr.authorDisplayName);
        if (!map.has(key)) {
          map.set(key, { key, label: pr.authorDisplayName, isMe: meKey != null && key === meKey });
        }
      }
    }
    if (meKey && currentUser && !map.has(meKey)) {
      map.set(meKey, { key: meKey, label: currentUser.displayName, isMe: true });
    }
    return [...map.values()].sort((a, b) =>
      a.isMe ? -1 : b.isMe ? 1 : a.label.localeCompare(b.label),
    );
  })();

  const closedAnalyticsAuthors: AuthorOption[] = useMemo(() => {
    const map = new Map<string, AuthorOption>();
    for (const metric of closedPrAnalytics.metrics) {
      if (repositoryFilter != null && repoKey(metric) !== repositoryFilter) continue;
      const key = authorKey(metric.authorAccountId, metric.authorDisplayName);
      if (!map.has(key)) {
        map.set(key, {
          key,
          label: metric.authorDisplayName || "Unknown",
          isMe: meKey != null && key === meKey,
        });
      }
    }
    if (meKey && currentUser && !map.has(meKey)) {
      map.set(meKey, { key: meKey, label: currentUser.displayName, isMe: true });
    }
    return [...map.values()].sort((a, b) =>
      a.isMe ? -1 : b.isMe ? 1 : a.label.localeCompare(b.label),
    );
  }, [closedPrAnalytics.metrics, currentUser, meKey, repositoryFilter]);

  const repositories = groups.map((group) => ({
    key: repoKey(group.repo),
    label: `${group.repo.workspace}/${group.repo.repo}`,
    count: group.pullRequests.length,
  }));
  const availableReviewReferencePullRequests = groups.flatMap((group) =>
    group.pullRequests.filter(
      (pr) =>
        activeSel == null ||
        pr.workspace !== activeSel.workspace ||
        pr.repo !== activeSel.repo ||
        pr.id !== activeSel.prId,
    ),
  );

  useEffect(() => {
    if (repositoryFilter == null) return;
    if (groups.some((group) => repoKey(group.repo) === repositoryFilter)) return;
    setRepositoryFilter(null);
  }, [groups, repositoryFilter]);

  const displayedGroups: PrGroup[] = groups
    .filter((group) => repositoryFilter == null || repoKey(group.repo) === repositoryFilter)
    .map((group) => ({
      ...group,
      pullRequests:
        authorFilter == null
          ? group.pullRequests
          : group.pullRequests.filter(
              (pr) => authorKey(pr.authorAccountId, pr.authorDisplayName) === authorFilter,
            ),
    }));

  // Keyboard: ? = help, o = overview, j / k = next / previous pull request across the list.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (shouldIgnoreShortcut(e)) return;
      if (e.key === "?") {
        e.preventDefault();
        setHelpOpen(true);
        return;
      }
      if (e.key === "o" && selection.kind !== "overview") {
        e.preventDefault();
        setSelection({ kind: "overview" });
        return;
      }
      if (
        e.key === "Escape" &&
        (selection.kind === "overview" || selection.kind === "closed-analytics")
      ) {
        e.preventDefault();
        setSelection({ kind: "pr-list" });
        return;
      }
      if (e.key === "r" && selection.kind === "pr") {
        e.preventDefault();
        setReviewPanelOpen((prev) => !prev);
        return;
      }
      if (e.key !== "j" && e.key !== "k") return;
      const flat = displayedGroups.flatMap((g) => g.pullRequests);
      if (flat.length === 0) return;
      const idx = flat.findIndex(
        (pr) =>
          activeSel != null &&
          pr.id === activeSel.prId &&
          pr.workspace === activeSel.workspace &&
          pr.repo === activeSel.repo,
      );
      const next =
        e.key === "j"
          ? Math.min(idx < 0 ? 0 : idx + 1, flat.length - 1)
          : Math.max(idx < 0 ? 0 : idx - 1, 0);
      const pr = flat[next];
      if (pr) {
        e.preventDefault();
        setSelection({
          kind: "pr",
          workspace: pr.workspace,
          repo: pr.repo,
          prId: pr.id,
          activeFilePath: null,
          activeFileLine: null,
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [displayedGroups, activeSel, selection]);

  const isOverview = selection.kind === "overview";
  const isClosedAnalytics = selection.kind === "closed-analytics";
  const sidebarAuthors = isClosedAnalytics ? closedAnalyticsAuthors : authors;
  const openPaneCount = [
    sidebarOpen,
    repositoriesPanelOpen,
    reviewHistoryPanelOpen,
    repositoryExplorerOpen,
    detailPaneOpen,
    reviewPanelOpen,
  ].filter(Boolean).length;
  const paneStatus = `${openPaneCount} pane${openPaneCount === 1 ? "" : "s"} open`;

  const buildActiveReviewRequest = async (): Promise<{
    payload: string;
    displayMessage: string;
    pr: PullRequestDetail;
  } | null> => {
    if (!activeSel) return null;
    const { payload, pr } = await buildAiReviewPayloadForPr({
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      provider: activeRepo?.provider ?? reviewProvider,
      prId: activeSel.prId,
      repoConfig: activeRepo,
      jiraBaseUrl: config?.jiraBaseUrl ?? null,
      jiraContextEnabled: Boolean(config?.hasJira && config?.jiraBaseUrl),
      reviewProfile: selectedReviewProfile || null,
      reviewReferences: reviewReferences.references,
    });
    return {
      payload,
      displayMessage: buildReviewPromptDisplayMessage(payload),
      pr,
    };
  };

  const buildLineQuestionRequest = async (
    lineContext: AiLineQuestionContext,
    question: string,
  ): Promise<{ payload: string; displayMessage: string; pr: PullRequestDetail } | null> => {
    if (!activeSel) return null;
    const snapshot = await loadStablePullRequestReviewSnapshot({
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      provider: activeRepo?.provider ?? reviewProvider,
      prId: activeSel.prId,
    });
    const currentHunkDiff = resolveLineQuestionHunkFromReviewSnapshot(
      snapshot.rawDiff,
      lineContext,
    );
    const label = lineQuestionLabel(lineContext);
    const displayMessage = [`Question about \`${label}\``, "", question.trim()].join("\n");
    const payload = [
      "You are answering a focused reviewer question about one changed line in a pull request.",
      "Answer directly and concisely.",
      "",
      "## Pull request",
      `${snapshot.pr.title} (#${snapshot.pr.id})`,
      `Branch: ${snapshot.pr.sourceBranch} -> ${snapshot.pr.destinationBranch}`,
      "",
      "## Selected line",
      `File: ${lineContext.path}`,
      `Side: ${lineContext.side}`,
      lineContext.to != null ? `New line: ${lineContext.to}` : null,
      lineContext.from != null ? `Old line: ${lineContext.from}` : null,
      `Selected line: ${lineContext.lineText}`,
      "",
      "## Diff hunk",
      "```diff",
      currentHunkDiff.trim(),
      "```",
      "",
      "## Reviewer question",
      question.trim(),
    ]
      .filter((line): line is string => line != null)
      .join("\n");
    return { payload, displayMessage, pr: snapshot.pr };
  };

  const hasAssistantReview =
    aiReview.activeThread?.messages.some((message) => message.role === "assistant") ?? false;
  const reviewForFix = hasAssistantReview ? aiReview.activeThread : null;
  const fixPayload =
    reviewForFix && aiReviewContext
      ? buildAiFixPayload({
          pr: aiReviewContext.pr,
          thread: reviewForFix,
          branchStatus: aiReviewContext.branchStatus,
          rawDiff: aiReviewContext.rawDiff,
          jiraKeys: aiReviewContext.jiraKeys,
          jiraBaseUrl: aiReviewContext.jiraBaseUrl,
          jiraContext: aiReviewContext.jiraContext,
        })
      : null;

  const handleRunInlineReview = (
    reviewTarget: PullRequestDetail,
    payload: string,
    displayMessage?: string | null,
    options: {
      reviewKind?: "lineQuestion";
      threadTitle?: string;
      reviewProfile?: string | null;
    } = {},
  ) => {
    if (!activeSel) return;
    const selectionForReview = activeSel;
    setReviewPanelOpen(true);
    void (async () => {
      let job: AiReviewJob | null = null;
      const updateJob = async (
        status: AiReviewJobStatus,
        threadId?: string | null,
        error?: string | null,
      ) => {
        if (!job) return;
        job = await tauriCall<AiReviewJob>("update_ai_review_job_status", {
          jobId: job.id,
          status,
          threadId: threadId ?? null,
          error: error ?? null,
        });
      };
      try {
        job = await tauriCall<AiReviewJob>("create_ai_review_job", {
          workspace: selectionForReview.workspace,
          repo: selectionForReview.repo,
          prId: selectionForReview.prId,
          prTitle: reviewTarget.title || `PR #${selectionForReview.prId}`,
          sourceBranch: reviewTarget.sourceBranch,
          destinationBranch: reviewTarget.destinationBranch,
          trigger: "manual",
        });
        await aiReview.run({
          payload,
          displayMessage,
          reviewKind: options.reviewKind ?? null,
          threadTitle: options.threadTitle ?? null,
          title: reviewTarget.title || `PR #${selectionForReview.prId}`,
          sourceBranch: reviewTarget.sourceBranch,
          destinationBranch: reviewTarget.destinationBranch,
          reviewedBaseSha: reviewTarget.destinationCommitHash ?? null,
          reviewedHeadSha: reviewTarget.sourceCommitHash ?? null,
          aiProvider: config?.aiProvider ?? "claude",
          claudeModel: config?.claudeModel ?? null,
          claudeEffort: config?.claudeEffort ?? null,
          codexModel: config?.codexModel ?? null,
          codexEffort: config?.codexEffort ?? null,
          reviewProfile: options.reviewProfile ?? null,
        });

        let finalState: AiReviewRunState | null = null;
        for (let attempt = 0; attempt < 60 * 30; attempt += 1) {
          await new Promise((resolve) => window.setTimeout(resolve, 1000));
          finalState = await tauriCall<AiReviewRunState | null>("get_ai_review_run_state", {
            workspace: selectionForReview.workspace,
            repo: selectionForReview.repo,
            id: selectionForReview.prId,
          });
          if (finalState?.status === "running") {
            await updateJob("running", finalState.threadId);
            continue;
          }
          break;
        }

        if (finalState?.status === "succeeded") {
          await updateJob("succeeded", finalState.threadId);
        } else if (finalState?.status === "failed") {
          await updateJob("failed", finalState.threadId, finalState.error);
        } else if (finalState?.status === "cancelled") {
          await updateJob("cancelled", finalState.threadId);
        } else {
          await updateJob(
            "failed",
            finalState?.threadId,
            "AI review did not finish before timeout.",
          );
        }
      } catch (error) {
        await updateJob("failed", null, error instanceof Error ? error.message : String(error));
      }
    })();
  };

  const handleRunNewReview = async () => {
    if (!activeSel || !aiReviewContext) return;
    try {
      const request = await buildActiveReviewRequest();
      if (!request) return;
      if (aiReview.activeThread?.id) {
        setReviewPanelOpen(true);
        void aiReview.reply({
          title: request.pr.title || `PR #${activeSel.prId}`,
          sourceBranch: request.pr.sourceBranch,
          destinationBranch: request.pr.destinationBranch,
          reviewedBaseSha: request.pr.destinationCommitHash ?? null,
          reviewedHeadSha: request.pr.sourceCommitHash ?? null,
          threadId: aiReview.activeThread.id,
          userMessage: request.displayMessage,
          basePayload: request.payload,
          aiProvider: config?.aiProvider ?? "claude",
          claudeModel: config?.claudeModel ?? null,
          claudeEffort: config?.claudeEffort ?? null,
          codexModel: config?.codexModel ?? null,
          codexEffort: config?.codexEffort ?? null,
        });
      } else {
        handleRunInlineReview(request.pr, request.payload, request.displayMessage, {
          reviewProfile: selectedReviewProfile || null,
        });
      }
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    }
  };

  const handleAskClaude = async (userMessage: string) => {
    try {
      const request = await buildActiveReviewRequest();
      if (!request) return;
      const payload = [
        request.payload.trim(),
        "",
        "## Initial question from the reviewer",
        userMessage.trim(),
      ].join("\n");
      handleRunInlineReview(request.pr, payload, userMessage.trim());
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    }
  };

  const handleAskAiLine = async (lineContext: AiLineQuestionContext, question: string) => {
    try {
      const request = await buildLineQuestionRequest(lineContext, question);
      if (!request) return;
      handleRunInlineReview(request.pr, request.payload, request.displayMessage, {
        reviewKind: "lineQuestion",
        threadTitle: "Line question",
      });
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    }
  };

  const handleReplyToReview = async (threadId: string, userMessage: string) => {
    if (!activeSel || !aiReviewContext) return;
    try {
      const request = await buildActiveReviewRequest();
      if (!request) return;
      setReviewPanelOpen(true);
      void aiReview.reply({
        title: request.pr.title || `PR #${activeSel.prId}`,
        sourceBranch: request.pr.sourceBranch,
        destinationBranch: request.pr.destinationBranch,
        reviewedBaseSha: request.pr.destinationCommitHash ?? null,
        reviewedHeadSha: request.pr.sourceCommitHash ?? null,
        threadId,
        userMessage,
        basePayload: request.payload,
        aiProvider: config?.aiProvider ?? "claude",
        claudeModel: config?.claudeModel ?? null,
        claudeEffort: config?.claudeEffort ?? null,
        codexModel: config?.codexModel ?? null,
        codexEffort: config?.codexEffort ?? null,
      });
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    }
  };

  const handleClearReview = (threadId: string) => {
    void aiReviewFix.reset().finally(() => aiReview.clearThread(threadId));
  };

  const handleCloseReviewPanel = () => {
    setReviewPanelOpen(false);
    setReviewPanelExpanded(false);
  };

  const handleResolveBranchConflicts = async (
    sourceBranch: string,
    destinationBranch: string,
    tips: string,
  ) => {
    setReviewPanelOpen(true);
    setReviewPanelExpanded(false);
    await aiReviewFix.startConflictResolution(sourceBranch, destinationBranch, tips);
  };

  const handleOpenRepositoryFile = (path: string, line?: number | null) => {
    if (!activeSel) return;
    setRepositoriesPanelOpen(false);
    setReviewHistoryPanelOpen(false);
    setDetailPaneOpen(false);
    setRepositoryExplorerOpen(true);
    setSelection({
      ...activeSel,
      activeFilePath: path,
      activeFileLine: line ?? null,
    });
  };

  const handleSelectRepositoryExplorerFile = (path: string, line?: number | null) => {
    if (!activeSel) return;
    setSelection({
      ...activeSel,
      activeFilePath: path,
      activeFileLine: line ?? null,
    });
  };

  const handleStageAiReviewComments = async () => {
    if (!activeSel || !aiReviewContext || !aiReview.activeThread) {
      return {
        added: 0,
        skipped: 0,
        skippedUnanchored: 0,
        skippedExistingDrafts: 0,
        skippedAlreadyStaged: 0,
        skippedAlreadyPublished: 0,
      };
    }
    const reviewRun = aiReview.activeRun;
    if (!reviewRun) {
      throw new Error("The review snapshot is unavailable; rerun the review before staging.");
    }
    const snapshot = await loadStablePullRequestReviewSnapshot({
      workspace: activeSel.workspace,
      repo: activeSel.repo,
      provider: activeRepo?.provider ?? reviewProvider,
      prId: activeSel.prId,
    });
    assertPullRequestMatchesReviewRun(reviewRun, snapshot.pr);
    const stagingContext: AiReviewContext = {
      ...aiReviewContext,
      pr: snapshot.pr,
      branchStatus: snapshot.branchStatus,
      rawDiff: snapshot.rawDiff,
    };

    const payload = buildAiReviewCommentDraftPayload({
      pr: stagingContext.pr,
      thread: aiReview.activeThread,
      reviewRun,
      branchStatus: stagingContext.branchStatus,
      rawDiff: stagingContext.rawDiff,
      jiraKeys: stagingContext.jiraKeys,
      jiraBaseUrl: stagingContext.jiraBaseUrl,
      jiraContext: stagingContext.jiraContext,
    });

    const suggestions = await tauriCall<AiReviewDraftCommentSuggestion[]>(
      "draft_ai_review_comments",
      {
        workspace: activeSel.workspace,
        repo: activeSel.repo,
        id: activeSel.prId,
        payload,
      },
    );

    const normalized = normalizeAiReviewDraftComments(stagingContext.rawDiff, suggestions);
    const linked = linkAiReviewDraftCommentsToFindings(reviewRun, normalized.comments);
    const filtered = filterStageableAiReviewDraftComments(
      linked,
      draftComments.drafts,
      activeFindingPublication,
    );
    const stageableComments = filtered.stageable;

    const stagedDrafts = draftComments.addDrafts(
      stageableComments.map((comment) => ({
        path: comment.path,
        to: comment.to,
        from: comment.from,
        raw: comment.raw,
        parentId: null,
        source: comment.findingRef ? "aiFinding" : "manual",
        findingRef: comment.findingRef,
        publicationMode: comment.publicationMode,
        reviewBaseSha: comment.findingRef ? (aiReview.activeRun?.reviewedBaseSha ?? null) : null,
        reviewHeadSha: comment.findingRef ? (aiReview.activeRun?.reviewedHeadSha ?? null) : null,
      })),
    );
    await stageFindingDrafts(stagedDrafts);
    if (stageableComments.length > 0) {
      setDetailPaneOpen(true);
    }
    const skippedUnanchored = normalized.skipped;
    const skipped = skippedUnanchored + filtered.skipped;
    return {
      added: stageableComments.length,
      skipped,
      skippedUnanchored,
      skippedExistingDrafts: filtered.skippedExistingDrafts,
      skippedAlreadyStaged: filtered.skippedAlreadyStaged,
      skippedAlreadyPublished: filtered.skippedAlreadyPublished,
    };
  };

  const handleTogglePane = (pane: AppPaneId) => {
    const next = {
      pullRequests: pane === "pullRequests" ? !sidebarOpen : sidebarOpen,
      repositories: pane === "repositories" ? !repositoriesPanelOpen : repositoriesPanelOpen,
      reviewHistory: pane === "reviewHistory" ? !reviewHistoryPanelOpen : reviewHistoryPanelOpen,
      repositoryExplorer:
        pane === "repositoryExplorer" ? !repositoryExplorerOpen : repositoryExplorerOpen,
      details: pane === "details" ? !detailPaneOpen : detailPaneOpen,
      aiReview: pane === "aiReview" ? !reviewPanelOpen : reviewPanelOpen,
    };
    if (
      !next.pullRequests &&
      !next.repositories &&
      !next.reviewHistory &&
      !next.repositoryExplorer &&
      !next.details &&
      !next.aiReview
    ) {
      return;
    }

    if (pane === "pullRequests") {
      setSidebarOpen((prev) => !prev);
      return;
    }
    if (pane === "repositories") {
      setRepositoriesPanelOpen((prev) => {
        const open = !prev;
        if (open) {
          setReviewHistoryPanelOpen(false);
          setRepositoryExplorerOpen(false);
        }
        return open;
      });
      return;
    }
    if (pane === "reviewHistory") {
      setReviewHistoryPanelOpen((prev) => {
        const open = !prev;
        if (open) {
          setRepositoriesPanelOpen(false);
          setRepositoryExplorerOpen(false);
        }
        return open;
      });
      return;
    }
    if (pane === "repositoryExplorer") {
      setRepositoryExplorerOpen((prev) => {
        const open = !prev;
        if (open) {
          setRepositoriesPanelOpen(false);
          setReviewHistoryPanelOpen(false);
          setDetailPaneOpen(false);
        }
        return open;
      });
      return;
    }
    if (pane === "details") {
      setDetailPaneOpen((prev) => {
        const open = !prev;
        if (open) setRepositoryExplorerOpen(false);
        return open;
      });
      return;
    }
    setReviewPanelOpen((prev) => {
      const open = !prev;
      if (!open) setReviewPanelExpanded(false);
      return open;
    });
  };

  const handleSelectReviewJob = (job: AiReviewJob) => {
    if (job.threadId) {
      pendingReviewThreadIdRef.current = job.threadId;
      setReviewPanelOpen(true);
    }
    selectPullRequest({ workspace: job.workspace, repo: job.repo, id: job.prId });
  };

  const handleSaveSettings = async ({
    repos: nextRepos,
    reviewProvider: nextReviewProvider,
    defaultDiffView,
    reviewTerminal,
    aiProvider,
    claudeModel,
    claudeEffort,
    codexModel,
    codexEffort,
    jiraBaseUrl,
    automaticSyncIntervalSeconds,
    menuBarSyncEnabled,
    notificationsEnabled,
    username,
    token,
    githubToken,
    jiraToken,
    notionToken,
  }: SettingsSaveInput) => {
    if (username && token) {
      await saveCredentials(username, token);
    }
    if (githubToken) {
      await saveGithubToken(githubToken);
    }
    if (jiraToken) await saveJiraToken(jiraToken);
    if (notionToken) await saveNotionToken(notionToken);
    await saveConfig({
      repos: nextRepos,
      reviewProvider: nextReviewProvider,
      defaultDiffView,
      theme,
      reviewTerminal,
      aiProvider,
      claudeModel,
      claudeEffort,
      codexModel,
      codexEffort,
      jiraBaseUrl,
      automaticSyncIntervalSeconds,
      menuBarSyncEnabled,
      notificationsEnabled,
    });
  };

  const handleReviewProviderChange = async (nextProvider: ReviewProvider) => {
    if (!config || nextProvider === reviewProvider) return;
    setAuthorFilter(null);
    setRepositoryFilter(null);
    setSelection({ kind: "pr-list" });
    await saveConfig({
      repos,
      reviewProvider: nextProvider,
      defaultDiffView: config.defaultDiffView,
      theme,
      reviewTerminal: config.reviewTerminal,
      aiProvider: config.aiProvider,
      claudeModel: config.claudeModel,
      claudeEffort: config.claudeEffort,
      codexModel: config.codexModel,
      codexEffort: config.codexEffort,
      jiraBaseUrl: config.jiraBaseUrl,
      automaticSyncIntervalSeconds: config.automaticSyncIntervalSeconds,
      menuBarSyncEnabled: config.menuBarSyncEnabled,
      notificationsEnabled: config.notificationsEnabled,
    });
  };

  return (
    <>
      <AppShell
        reviewProvider={reviewProvider}
        onReviewProviderChange={handleReviewProviderChange}
        headerRight={<ThemeToggle theme={theme} onToggle={toggle} />}
        footer={
          isOverview || isClosedAnalytics || selection.kind === "settings" ? undefined : (
            <BottomPaneBar
              panes={{
                pullRequests: sidebarOpen,
                repositories: repositoriesPanelOpen,
                reviewHistory: reviewHistoryPanelOpen,
                repositoryExplorer: repositoryExplorerOpen,
                details: detailPaneOpen,
                aiReview: reviewPanelOpen,
              }}
              disabled={{
                aiReview: activeSel == null && !reviewPanelOpen,
                repositoryExplorer: activeSel == null,
              }}
              status={paneStatus}
              onTogglePane={handleTogglePane}
            />
          )
        }
        rightPanelExpanded={reviewPanelExpanded}
        rightPanel={
          reviewPanelOpen && activeSel ? (
            <AiReviewPanel
              key={`${activeSel.workspace}/${activeSel.repo}/${activeSel.prId}`}
              store={aiReview.store}
              activeThread={aiReview.activeThread}
              activeRun={aiReview.activeRun}
              reviewState={aiReview.state}
              aiProvider={config?.aiProvider ?? "claude"}
              loading={aiReview.loading}
              error={aiReview.error}
              reviewProfiles={reviewProfiles}
              selectedReviewProfile={selectedReviewProfile}
              onReviewProfileChange={setSelectedReviewProfile}
              onRun={aiReviewContext ? handleRunNewReview : undefined}
              onAsk={aiReviewContext ? handleAskClaude : undefined}
              onReply={aiReviewContext ? handleReplyToReview : undefined}
              onCancelReview={() => aiReview.cancel()}
              onSelectThread={(threadId) => aiReview.setActiveThread(threadId)}
              onClearThread={handleClearReview}
              onClose={handleCloseReviewPanel}
              onOpenFile={handleOpenRepositoryFile}
              expanded={reviewPanelExpanded}
              onToggleExpand={() => setReviewPanelExpanded((prev) => !prev)}
              onStageComments={handleStageAiReviewComments}
              fixState={aiReviewFix.state}
              fixBusy={aiReviewFix.loading}
              onStartFix={
                reviewForFix && aiReviewContext && fixPayload
                  ? () =>
                      aiReviewFix.startFix({
                        payload: fixPayload,
                        sourceBranch: aiReviewContext.pr.sourceBranch,
                        destinationBranch: aiReviewContext.pr.destinationBranch,
                      })
                  : undefined
              }
              onCommit={(message) => aiReviewFix.startCommit(message)}
              onPush={() => aiReviewFix.startPush()}
            />
          ) : undefined
        }
        sidebar={
          isOverview || selection.kind === "settings" || !sidebarOpen ? undefined : (
            <PrSidebar
              groups={displayedGroups}
              filter={filter}
              loading={loading}
              active={
                activeSel
                  ? { workspace: activeSel.workspace, repo: activeSel.repo, prId: activeSel.prId }
                  : null
              }
              authors={sidebarAuthors}
              authorFilter={authorFilter}
              repositories={repositories}
              repositoryFilter={repositoryFilter}
              onFilterChange={setFilter}
              onAuthorFilterChange={setAuthorFilter}
              onRepositoryFilterChange={setRepositoryFilter}
              onSelect={(pr) => {
                selectPullRequest(pr);
              }}
              onLoadMore={loadMore}
              onRefresh={refresh}
              onOpenSettings={() => setSelection({ kind: "settings" })}
              onOpenOverview={() => setSelection({ kind: "overview" })}
              onOpenClosedAnalytics={() => setSelection({ kind: "closed-analytics" })}
            />
          )
        }
        main={
          isOverview ? (
            <OverviewPanel
              groups={groups}
              loading={loading}
              onRefresh={refresh}
              onBack={() => setSelection({ kind: "pr-list" })}
              onOpenClosedAnalytics={() => setSelection({ kind: "closed-analytics" })}
              onSelectPr={(pr) => selectPullRequest(pr)}
              currentUser={currentUser}
            />
          ) : isClosedAnalytics ? (
            <Suspense
              fallback={
                <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                  Loading analytics...
                </div>
              }
            >
              <ClosedPrAnalyticsPanel
                metrics={closedPrAnalytics.metrics}
                loading={closedPrAnalytics.loading}
                syncing={closedPrAnalytics.syncing}
                error={closedPrAnalytics.error}
                lastSync={closedPrAnalytics.lastSync}
                repositoryFilter={repositoryFilter}
                authorFilter={authorFilter}
                onSync={closedPrAnalytics.sync}
                onBack={() => setSelection({ kind: "pr-list" })}
                onSelectPr={(pr) => selectPullRequest(pr)}
              />
            </Suspense>
          ) : selection.kind === "settings" ? (
            <SettingsPage
              repos={repos}
              reviewProvider={reviewProvider}
              defaultDiffView={config?.defaultDiffView ?? "unified"}
              reviewTerminal={config?.reviewTerminal ?? null}
              aiProvider={config?.aiProvider ?? "claude"}
              claudeModel={config?.claudeModel ?? null}
              claudeEffort={config?.claudeEffort ?? null}
              codexModel={config?.codexModel ?? null}
              codexEffort={config?.codexEffort ?? null}
              reviewTerminalOptions={reviewTerminalOptions}
              jiraBaseUrl={config?.jiraBaseUrl ?? null}
              automaticSyncIntervalSeconds={config?.automaticSyncIntervalSeconds ?? null}
              menuBarSyncEnabled={config?.menuBarSyncEnabled ?? true}
              notificationsEnabled={config?.notificationsEnabled ?? false}
              hasCredentials={config?.hasCredentials ?? false}
              hasGithubCredentials={config?.hasGithubCredentials ?? false}
              hasJira={config?.hasJira ?? false}
              hasNotion={config?.hasNotion ?? false}
              onTestConnection={testConnection}
              onSave={handleSaveSettings}
              onBack={() => setSelection({ kind: "pr-list" })}
            />
          ) : repositoriesPanelOpen ? (
            <RepositoryBranchesPanel />
          ) : reviewHistoryPanelOpen ? (
            <ReviewHistoryPanel onSelectJob={handleSelectReviewJob} />
          ) : repositoryExplorerOpen ? (
            <RepositoryExplorerPanel
              workspace={activeSel?.workspace ?? null}
              repo={activeSel?.repo ?? null}
              initialPath={activeSel?.activeFilePath ?? null}
              initialLine={activeSel?.activeFileLine ?? null}
              onSelectFile={handleSelectRepositoryExplorerFile}
            />
          ) : detailPaneOpen ? (
            <PrDetailPanel
              provider={activeRepo?.provider ?? reviewProvider}
              workspace={activeSel?.workspace ?? null}
              repo={activeSel?.repo ?? null}
              prId={activeSel?.prId ?? null}
              currentUserAccountId={currentUser?.accountId ?? null}
              currentUserDisplayName={currentUser?.displayName ?? null}
              defaultViewMode={config?.defaultDiffView ?? "unified"}
              jiraBaseUrl={config?.jiraBaseUrl ?? null}
              jiraContextEnabled={Boolean(config?.hasJira && config?.jiraBaseUrl)}
              availablePullRequests={availableReviewReferencePullRequests}
              availableRepositories={activeRepos}
              reviewReferences={reviewReferences.references}
              addReviewReference={reviewReferences.addReference}
              updateReviewReference={reviewReferences.updateReference}
              removeReviewReference={reviewReferences.removeReference}
              onOpenAiReview={() => setReviewPanelOpen(true)}
              onResolveBranchConflicts={handleResolveBranchConflicts}
              onAiReviewContextChange={setAiReviewContext}
              onAskAiLine={handleAskAiLine}
              drafts={draftComments.drafts}
              publishing={draftComments.publishing}
              publishingDraftId={draftComments.publishingDraftId}
              addDraft={draftComments.addDraft}
              updateDraft={draftComments.updateDraft}
              removeDraft={draftComments.removeDraft}
              discardAll={draftComments.discardAll}
              publishDraft={draftComments.publishDraft}
              publishAll={draftComments.publishAll}
            />
          ) : undefined
        }
      />
      <ShortcutsDialog open={helpOpen} onOpenChange={setHelpOpen} />
    </>
  );
}
