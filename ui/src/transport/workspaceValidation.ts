import type { WorkspaceDraft } from "../../../src/protocol/generated/protocol";
export const draftModes = ["general", "inline", "file", "localOnly"] as const;
const keys = new Set([
  "localId",
  "prId",
  "path",
  "to",
  "from",
  "raw",
  "parentId",
  "createdAt",
  "source",
  "findingRef",
  "publicationMode",
  "reviewBaseSha",
  "reviewHeadSha",
]);
const text = (value: unknown, max: number, multiline = false): value is string =>
  typeof value === "string" &&
  Buffer.byteLength(value) <= max &&
  !(multiline ? /[^\P{Cc}\n\r\t]/u : /\p{Cc}/u).test(value);
const positive = (value: unknown) =>
  Number.isInteger(value) && Number(value) > 0 && Number(value) <= 4294967295;
export function validDraft(value: unknown): value is WorkspaceDraft {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const item = value as WorkspaceDraft;
  if (
    Object.keys(value).some((key) => !keys.has(key)) ||
    !text(item.localId, 256) ||
    !item.localId ||
    !positive(item.prId) ||
    !text(item.path, 4096) ||
    !text(item.raw, 65536, true) ||
    !Number.isSafeInteger(item.createdAt) ||
    item.createdAt < 0 ||
    (item.source != null && !["manual", "aiFinding"].includes(item.source)) ||
    (item.publicationMode != null && !draftModes.includes(item.publicationMode))
  )
    return false;
  if ([item.to, item.from].some((value) => value != null && !positive(value))) return false;
  if (
    [item.parentId, item.reviewBaseSha, item.reviewHeadSha].some(
      (value) => value != null && !text(value, 256),
    )
  )
    return false;
  const ref = item.findingRef;
  return (
    ref == null ||
    (typeof ref === "object" &&
      !Array.isArray(ref) &&
      Object.keys(ref).every((key) =>
        ["reviewRunId", "findingId", "findingFingerprint"].includes(key),
      ) &&
      [ref.reviewRunId, ref.findingId, ref.findingFingerprint].every((value) => text(value, 256)))
  );
}
