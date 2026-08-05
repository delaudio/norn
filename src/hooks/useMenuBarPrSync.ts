import { useEffect, useMemo } from "react";
import type { PrGroup } from "@/hooks/usePullRequests";
import {
  buildMenuBarPrSnapshot,
  readMenuBarPrSnapshot,
  writeMenuBarPrSnapshot,
} from "@/lib/menuBarPrSnapshotStorage";
import { isTauri } from "@/lib/tauri";
import type { PullRequestSummary } from "@/types";

const TRAY_ID = "norn-main";
const MAX_MENU_PRS = 8;
const MAX_NOTIFICATIONS_PER_SYNC = 3;
const MENU_TITLE_LIMIT = 46;
const MENU_BRANCH_LIMIT = 38;

interface UseMenuBarPrSyncArgs {
  groups: PrGroup[];
  loading: boolean;
  menuBarSyncEnabled: boolean;
  notificationsEnabled: boolean;
  onSync: () => Promise<void>;
  onOpenPr: (pr: PullRequestSummary) => void;
  onReviewPr?: (pr: PullRequestSummary) => Promise<void>;
  reviewingPrKey?: string | null;
}

let trayReady: Promise<void> | null = null;
let trayRef: import("@tauri-apps/api/tray").TrayIcon | null = null;

function prKey(pr: PullRequestSummary): string {
  return `${pr.workspace}/${pr.repo}#${pr.id}`;
}

function formatPrMenuLabel(pr: PullRequestSummary): string {
  return `#${pr.id} ${truncateMenuText(pr.title, MENU_TITLE_LIMIT)} (${pr.repo})`;
}

function truncateMenuText(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, maxLength - 3)}...`;
}

function stateSymbol(pr: PullRequestSummary): string {
  if (pr.draft) return "◌";
  if (pr.state === "MERGED") return "✓";
  if (pr.state === "DECLINED" || pr.state === "SUPERSEDED") return "×";
  return "●";
}

function formatStatusLabel(args: UseMenuBarPrSyncArgs, latestPrs: PullRequestSummary[]) {
  if (!args.menuBarSyncEnabled) return "○ Pull request sync disabled";
  if (args.loading) return "↻ Syncing pull requests...";
  if (args.reviewingPrKey) return "▶ Background review running";
  return `● Idle · ${latestPrs.length} latest PR${latestPrs.length === 1 ? "" : "s"}`;
}

function formatPrStateLabel(pr: PullRequestSummary): string {
  const state = pr.draft ? "Draft" : pr.state.toLowerCase();
  const comments = `${pr.commentCount} comment${pr.commentCount === 1 ? "" : "s"}`;
  return `${state} · ${comments}`;
}

function findReviewingPr(args: UseMenuBarPrSyncArgs, latestPrs: PullRequestSummary[]) {
  if (!args.reviewingPrKey) return null;
  return latestPrs.find((pr) => prKey(pr) === args.reviewingPrKey) ?? null;
}

function prMenuId(pr: PullRequestSummary): string {
  return `pr-${encodeURIComponent(pr.workspace)}:${encodeURIComponent(pr.repo)}:${pr.id}`;
}

function parsePrMenuPayload(payload: string) {
  const [workspace, repo, id] = payload.split(":");
  if (!workspace || !repo || !id) return null;
  const prId = Number(id);
  if (!Number.isFinite(prId)) return null;
  return {
    workspace: decodeURIComponent(workspace),
    repo: decodeURIComponent(repo),
    id: prId,
  };
}

async function focusMainWindow() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const window = getCurrentWindow();
  await window.unminimize();
  await window.show();
  await window.setFocus();
}

async function ensureTray() {
  if (trayReady) return trayReady;

  trayReady = (async () => {
    const [{ defaultWindowIcon }, { TrayIcon }] = await Promise.all([
      import("@tauri-apps/api/app"),
      import("@tauri-apps/api/tray"),
    ]);
    const existing = await TrayIcon.getById(TRAY_ID);
    if (existing) {
      trayRef = existing;
      await trayRef.setTitle("");
      await trayRef.setTooltip("Norn");
      return;
    }
    const icon = await defaultWindowIcon();
    trayRef = await TrayIcon.new({
      id: TRAY_ID,
      icon: icon ?? undefined,
      iconAsTemplate: false,
      tooltip: "Norn",
      menuOnLeftClick: true,
      action: (event) => {
        if (event.type === "DoubleClick") {
          void focusMainWindow();
        }
      },
    });
  })();

  return trayReady;
}

async function updateTrayMenu(args: UseMenuBarPrSyncArgs, latestPrs: PullRequestSummary[]) {
  if (!isTauri()) return;
  await ensureTray();
  if (!trayRef) return;

  const { Menu } = await import("@tauri-apps/api/menu");
  const newestPr = latestPrs[0] ?? null;
  const reviewingPr = findReviewingPr(args, latestPrs);
  const canRunQuickReview = Boolean(
    args.menuBarSyncEnabled && args.onReviewPr && newestPr && !args.reviewingPrKey,
  );
  const items = [
    {
      id: "status",
      text: formatStatusLabel(args, latestPrs),
      enabled: false,
    },
    {
      item: "Separator" as const,
    },
    {
      id: "open",
      text: "Open Norn",
      action: () => {
        void focusMainWindow();
      },
    },
    {
      id: "sync",
      text: args.loading ? "↻ Syncing pull requests..." : "↻ Sync pull requests",
      enabled: args.menuBarSyncEnabled && !args.loading,
      action: () => {
        if (!args.menuBarSyncEnabled) return;
        void args.onSync();
      },
    },
    {
      id: "open-latest",
      text: newestPr ? `Open latest: #${newestPr.id}` : "Open latest PR",
      enabled: Boolean(newestPr),
      action: () => {
        if (!newestPr) return;
        void focusMainWindow().finally(() => args.onOpenPr(newestPr));
      },
    },
    {
      id: "review-latest",
      text: reviewingPr
        ? `▶ Reviewing #${reviewingPr.id}...`
        : newestPr
          ? `▶ Review latest: #${newestPr.id}`
          : "▶ Review latest PR",
      enabled: canRunQuickReview,
      action: () => {
        if (!canRunQuickReview || !newestPr || !args.onReviewPr) return;
        void args.onReviewPr(newestPr);
      },
    },
    {
      item: "Separator" as const,
    },
    {
      id: "pull-requests",
      text: latestPrs.length === 0 ? "Pull requests" : `Pull requests (${latestPrs.length})`,
      enabled: latestPrs.length > 0,
      items:
        latestPrs.length === 0
          ? [
              {
                id: "no-pull-requests",
                text: "No pull requests loaded",
                enabled: false,
              },
            ]
          : latestPrs.map((pr) => {
              const key = prKey(pr);
              const reviewing = args.reviewingPrKey === key;
              const baseId = prMenuId(pr);
              return {
                id: `actions-${baseId}`,
                text: `${reviewing ? "▶" : stateSymbol(pr)} ${formatPrMenuLabel(pr)}`,
                items: [
                  {
                    id: baseId,
                    text: "Open PR",
                    action: () => {
                      void focusMainWindow().finally(() => args.onOpenPr(pr));
                    },
                  },
                  {
                    id: `review-${baseId}`,
                    text: reviewing ? "Review running..." : "Review in background",
                    enabled: Boolean(args.onReviewPr) && !reviewing && !args.reviewingPrKey,
                    action: () => {
                      if (reviewing || args.reviewingPrKey || !args.onReviewPr) return;
                      void args.onReviewPr(pr);
                    },
                  },
                  {
                    item: "Separator" as const,
                  },
                  {
                    id: `meta-state-${baseId}`,
                    text: formatPrStateLabel(pr),
                    enabled: false,
                  },
                  {
                    id: `meta-author-${baseId}`,
                    text: `Author: ${pr.authorDisplayName}`,
                    enabled: false,
                  },
                  {
                    id: `meta-branch-${baseId}`,
                    text: `Branch: ${truncateMenuText(pr.sourceBranch, MENU_BRANCH_LIMIT)}`,
                    enabled: false,
                  },
                ],
              };
            }),
    },
  ];

  const menu = await Menu.new({ items });
  await trayRef.setTooltip(
    args.menuBarSyncEnabled ? "Norn pull requests" : "Norn pull request sync disabled",
  );
  await trayRef.setMenu(menu);
}

async function notifyPrChanges(prs: PullRequestSummary[]) {
  if (!isTauri() || prs.length === 0) return;

  const previous = readMenuBarPrSnapshot();
  const next = buildMenuBarPrSnapshot(prs);
  if (!previous) {
    writeMenuBarPrSnapshot(next);
    return;
  }

  const changes: Array<{ title: string; body: string }> = [];
  for (const pr of prs) {
    const before = previous[prKey(pr)];
    if (!before) {
      changes.push({
        title: "New pull request",
        body: `${pr.repo} #${pr.id}: ${pr.title}`,
      });
      continue;
    }
    if (pr.commentCount > before.commentCount) {
      changes.push({
        title: "New PR comments",
        body: `${pr.repo} #${pr.id}: ${pr.commentCount - before.commentCount} new comment(s)`,
      });
      continue;
    }
    if (pr.updatedOn !== before.updatedOn || pr.state !== before.state) {
      changes.push({
        title: "Pull request updated",
        body: `${pr.repo} #${pr.id}: ${pr.title}`,
      });
    }
  }

  writeMenuBarPrSnapshot(next);
  if (changes.length === 0) return;

  const { isPermissionGranted, requestPermission, sendNotification } = await import(
    "@tauri-apps/plugin-notification"
  );
  let permissionGranted = await isPermissionGranted();
  if (!permissionGranted) {
    permissionGranted = (await requestPermission()) === "granted";
  }
  if (!permissionGranted) return;

  for (const change of changes.slice(0, MAX_NOTIFICATIONS_PER_SYNC)) {
    sendNotification(change);
  }
  if (changes.length > MAX_NOTIFICATIONS_PER_SYNC) {
    sendNotification({
      title: "More pull request updates",
      body: `${changes.length - MAX_NOTIFICATIONS_PER_SYNC} additional update(s) in Norn.`,
    });
  }
}

export function useMenuBarPrSync({
  groups,
  loading,
  menuBarSyncEnabled,
  notificationsEnabled,
  onSync,
  onOpenPr,
  onReviewPr,
  reviewingPrKey,
}: UseMenuBarPrSyncArgs) {
  const pullRequests = useMemo(() => groups.flatMap((group) => group.pullRequests), [groups]);
  const latestPrs = useMemo(
    () =>
      [...pullRequests]
        .sort((a, b) => Date.parse(b.updatedOn) - Date.parse(a.updatedOn))
        .slice(0, MAX_MENU_PRS),
    [pullRequests],
  );

  useEffect(() => {
    void updateTrayMenu(
      {
        groups,
        loading,
        menuBarSyncEnabled,
        notificationsEnabled,
        onSync,
        onOpenPr,
        onReviewPr,
        reviewingPrKey,
      },
      latestPrs,
    ).catch((error) => {
      console.error("Failed to update Norn menu bar item:", error);
    });
  }, [
    groups,
    loading,
    latestPrs,
    menuBarSyncEnabled,
    notificationsEnabled,
    onSync,
    onOpenPr,
    onReviewPr,
    reviewingPrKey,
  ]);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisten = Promise.all([
      import("@tauri-apps/api/event").then(({ listen }) =>
        listen("norn-menu-sync", () => {
          void onSync();
        }),
      ),
      import("@tauri-apps/api/event").then(({ listen }) =>
        listen<string>("norn-menu-open-pr", (event) => {
          const parsed = parsePrMenuPayload(event.payload);
          const pr =
            parsed &&
            pullRequests.find(
              (item) =>
                item.workspace === parsed.workspace &&
                item.repo === parsed.repo &&
                item.id === parsed.id,
            );
          if (pr) {
            void focusMainWindow().finally(() => onOpenPr(pr));
          }
        }),
      ),
    ]);
    return () => {
      disposed = true;
      void unlisten.then((callbacks) => {
        if (!disposed) return;
        for (const callback of callbacks) callback();
      });
    };
  }, [onSync, onOpenPr, pullRequests]);

  useEffect(() => {
    if (!notificationsEnabled || loading) return;
    void notifyPrChanges(pullRequests).catch((error) => {
      console.error("Failed to send Norn PR notification:", error);
    });
  }, [pullRequests, loading, notificationsEnabled]);
}
