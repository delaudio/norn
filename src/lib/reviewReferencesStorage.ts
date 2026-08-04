import type { ReviewReference } from "@/types";
import { readMigratedStorageValue } from "./storageMigration";

function storageKey(workspace: string, repo: string, prId: number): string {
  return `norn.reviewReferences.v1.${workspace}/${repo}/${prId}`;
}

function legacyStorageKey(workspace: string, repo: string, prId: number): string {
  return `lachesi.reviewReferences.${workspace}/${repo}/${prId}`;
}

function isReviewReference(item: unknown): item is ReviewReference {
  return (
    item != null &&
    typeof item === "object" &&
    typeof (item as ReviewReference).id === "string" &&
    typeof (item as ReviewReference).type === "string" &&
    typeof (item as ReviewReference).source === "string"
  );
}

function normalizeLegacyReferences(raw: string): string | null {
  const parsed: unknown = JSON.parse(raw);
  return Array.isArray(parsed) && parsed.every(isReviewReference) ? JSON.stringify(parsed) : null;
}

function parseReferences(raw: string | null): ReviewReference[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(isReviewReference);
  } catch {
    return [];
  }
}

export function loadReviewReferences(
  workspace: string | null,
  repo: string | null,
  prId: number | null,
): ReviewReference[] {
  if (!workspace || !repo || prId == null) return [];
  if (typeof localStorage === "undefined") return [];
  return parseReferences(
    readMigratedStorageValue(
      localStorage,
      storageKey(workspace, repo, prId),
      legacyStorageKey(workspace, repo, prId),
      normalizeLegacyReferences,
    ),
  );
}

export function saveReviewReferences(
  workspace: string,
  repo: string,
  prId: number,
  references: ReviewReference[],
): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(storageKey(workspace, repo, prId), JSON.stringify(references));
  } catch {
    // Review references are optional; storage failures must not interrupt review work.
  }
}
