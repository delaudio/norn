import { readMigratedStorageValue } from "./storageMigration";

function storageKey(workspace: string, repo: string, prId: number): string {
  return `norn.viewedFiles.v1.${workspace}/${repo}#${prId}`;
}

function legacyStorageKey(key: string): string {
  return key.replace(/^norn\.viewedFiles\.v1\./, "lachesi.viewedFiles.");
}

function normalizeLegacyViewedFiles(raw: string): string | null {
  const parsed: unknown = JSON.parse(raw);
  return Array.isArray(parsed) && parsed.every((item) => typeof item === "string")
    ? JSON.stringify(parsed)
    : null;
}

export function viewedFilesStorageKey(
  workspace: string | null,
  repo: string | null,
  prId: number | null,
): string | null {
  if (!workspace || !repo || prId == null) return null;
  return storageKey(workspace, repo, prId);
}

export function loadViewedFiles(key: string | null): Set<string> {
  if (!key || typeof localStorage === "undefined") return new Set();
  try {
    const value = JSON.parse(
      readMigratedStorageValue(
        localStorage,
        key,
        legacyStorageKey(key),
        normalizeLegacyViewedFiles,
      ) ?? "[]",
    );
    if (!Array.isArray(value)) return new Set();
    return new Set(value.filter((item) => typeof item === "string"));
  } catch {
    return new Set();
  }
}

export function saveViewedFiles(key: string | null, viewed: Set<string>): void {
  if (!key || typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(key, JSON.stringify([...viewed]));
  } catch {
    // Viewed state is a convenience; ignore storage failures.
  }
}
