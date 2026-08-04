import type { PullRequestSummary } from "@/types";
import { readMigratedStorageValue } from "./storageMigration";

const STORAGE_KEY = "norn.menuBar.prSnapshot.v1";
const LEGACY_STORAGE_KEY = "lachesi.menuBar.prSnapshot.v1";

export interface MenuBarPrSnapshotEntry {
  title: string;
  updatedOn: string;
  commentCount: number;
  state: string;
}

export type MenuBarPrSnapshot = Record<string, MenuBarPrSnapshotEntry>;

function snapshotKey(pr: PullRequestSummary): string {
  return `${pr.workspace}/${pr.repo}#${pr.id}`;
}

function normalizeLegacySnapshot(raw: string): string | null {
  const parsed: unknown = JSON.parse(raw);
  if (parsed == null || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const valid = Object.values(parsed).every(
    (entry) =>
      entry != null &&
      typeof entry === "object" &&
      typeof (entry as MenuBarPrSnapshotEntry).title === "string" &&
      typeof (entry as MenuBarPrSnapshotEntry).updatedOn === "string" &&
      typeof (entry as MenuBarPrSnapshotEntry).commentCount === "number" &&
      typeof (entry as MenuBarPrSnapshotEntry).state === "string",
  );
  return valid ? JSON.stringify(parsed) : null;
}

export function buildMenuBarPrSnapshot(prs: PullRequestSummary[]): MenuBarPrSnapshot {
  const snapshot: MenuBarPrSnapshot = {};
  for (const pr of prs) {
    snapshot[snapshotKey(pr)] = {
      title: pr.title,
      updatedOn: pr.updatedOn,
      commentCount: pr.commentCount,
      state: pr.state,
    };
  }
  return snapshot;
}

export function readMenuBarPrSnapshot(): MenuBarPrSnapshot | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = readMigratedStorageValue(
      localStorage,
      STORAGE_KEY,
      LEGACY_STORAGE_KEY,
      normalizeLegacySnapshot,
    );
    return raw ? (JSON.parse(raw) as MenuBarPrSnapshot) : null;
  } catch {
    return null;
  }
}

export function writeMenuBarPrSnapshot(snapshot: MenuBarPrSnapshot): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
  } catch {
    // Best effort only: notification dedupe should never break review flows.
  }
}
