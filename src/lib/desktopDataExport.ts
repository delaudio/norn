/** Explicit allowlisted export of user-authored webview content; no migration writes. */
export interface DesktopDataExport {
  schemaVersion: 1;
  source: "desktopLocalStorage";
  entries: Array<{ key: string; value: string }>;
}
const allowed =
  /^(?:norn\.(?:drafts|reviewReferences|viewedFiles|reviewPrompt)\.v1\..+|lachesi\.(?:drafts|reviewReferences|viewedFiles|reviewPrompt)\..+|norn\.theme\.v1|lachesi\.theme)$/;
const maximumBytes = 16 * 1024 * 1024;
export function collectDesktopData(
  storage: Pick<Storage, "length" | "key" | "getItem">,
): DesktopDataExport {
  const entries: DesktopDataExport["entries"] = [];
  if (storage.length > 100_000) throw new Error("Storage is too large to inspect safely.");
  for (let index = 0; index < storage.length; index++) {
    const key = storage.key(index);
    if (!key || !allowed.test(key)) continue;
    const value = storage.getItem(key);
    if (value == null)
      throw new Error("Storage changed during export. Retry without editing drafts.");
    if (new TextEncoder().encode(value).length > 256 * 1024 || key.length > 1024)
      throw new Error(
        "A user-data entry exceeds the export limit. Preserve the profile and split the export.",
      );
    entries.push({ key, value });
    if (entries.length > 10_000) throw new Error("Too many user-data entries to export at once.");
  }
  entries.sort((left, right) => left.key.localeCompare(right.key));
  const bundle: DesktopDataExport = { schemaVersion: 1, source: "desktopLocalStorage", entries };
  if (new TextEncoder().encode(JSON.stringify(bundle)).length > maximumBytes)
    throw new Error("User data exceeds 16 MiB. Preserve the profile and split the export.");
  return bundle;
}
export function downloadDesktopData(storage: Storage = localStorage): number {
  const bundle = collectDesktopData(storage);
  const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
  if (blob.size > maximumBytes)
    throw new Error("Formatted export exceeds 16 MiB. Preserve the profile and split the export.");
  const url = URL.createObjectURL(blob);
  try {
    const link = document.createElement("a");
    link.href = url;
    link.download = "norn-desktop-user-data-v1.json";
    document.body.append(link);
    link.click();
    link.remove();
  } finally {
    // Allow the webview download to consume the URL before revoking it.
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  return bundle.entries.length;
}
