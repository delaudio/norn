import {
  type ChangeData,
  type FileData,
  getChangeKey,
  type HunkData,
  parseDiff,
} from "react-diff-view";

export type { ChangeData, FileData, HunkData };

/** Parse a raw unified diff (the body of Bitbucket's `/diff` endpoint). */
export function parseUnifiedDiff(raw: string): FileData[] {
  if (!raw.trim()) return [];
  return parseDiff(raw, { nearbySequences: "zip" });
}

/** The path to show for a file (new path, except for deletions). */
export function fileDisplayPath(file: FileData): string {
  if (file.type === "delete") return file.oldPath;
  return file.newPath || file.oldPath;
}

/** Stable identity for a file across renders (used as React key / anchor id). */
export function fileKey(file: FileData): string {
  return `${file.oldRevision ?? ""}:${file.oldPath ?? ""}->${file.newRevision ?? ""}:${file.newPath ?? ""}`;
}

/** A DOM-safe anchor id for scroll-to-file navigation. */
export function fileAnchorId(file: FileData): string {
  return `file-${fileKey(file).replace(/[^a-zA-Z0-9]/g, "-")}`;
}

export function countChanges(file: FileData): { additions: number; deletions: number } {
  let additions = 0;
  let deletions = 0;
  for (const hunk of file.hunks) {
    for (const change of hunk.changes) {
      if (change.type === "insert") additions += 1;
      else if (change.type === "delete") deletions += 1;
    }
  }
  return { additions, deletions };
}

/** New-file line number for a change, if it has one (insert / normal). */
export function changeNewLine(change: ChangeData): number | undefined {
  if (change.type === "insert") return change.lineNumber;
  if (change.type === "normal") return change.newLineNumber;
  return undefined;
}

/** Old-file line number for a change, if it has one (delete / normal). */
export function changeOldLine(change: ChangeData): number | undefined {
  if (change.type === "delete") return change.lineNumber;
  if (change.type === "normal") return change.oldLineNumber;
  return undefined;
}

function diffPrefixForChange(change: ChangeData): string {
  if (change.type === "insert") return "+";
  if (change.type === "delete") return "-";
  return " ";
}

export function buildTargetedHunkDiff(file: FileData, change: ChangeData): string {
  const changeKey = getChangeKey(change);
  const hunk = file.hunks.find((candidate) =>
    candidate.changes.some((item) => getChangeKey(item) === changeKey),
  );
  const oldPath = file.oldPath || "/dev/null";
  const newPath = file.newPath || "/dev/null";
  const lines = [
    `diff --git a/${file.oldPath || newPath} b/${file.newPath || oldPath}`,
    oldPath === "/dev/null" ? "--- /dev/null" : `--- a/${oldPath}`,
    newPath === "/dev/null" ? "+++ /dev/null" : `+++ b/${newPath}`,
  ];
  if (hunk) {
    lines.push(hunk.content);
    for (const hunkChange of hunk.changes) {
      lines.push(`${diffPrefixForChange(hunkChange)}${hunkChange.content}`);
    }
  } else {
    lines.push(`${diffPrefixForChange(change)}${change.content}`);
  }
  return lines.join("\n");
}

/**
 * Resolve a Bitbucket inline anchor (`to` = new side, `from` = old side) to a
 * react-diff-view change key, so existing comments / drafts can be rendered as
 * widgets on the right line.
 */
export function changeKeyForAnchor(
  file: FileData,
  to: number | null,
  from: number | null,
): string | null {
  for (const hunk of file.hunks) {
    for (const change of hunk.changes) {
      if (to != null && changeNewLine(change) === to) return getChangeKey(change);
      if (from != null && changeOldLine(change) === from) return getChangeKey(change);
    }
  }
  return null;
}

export { getChangeKey };
