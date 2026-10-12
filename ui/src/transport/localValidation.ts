import type {
  LocalComparison,
  LocalSnapshotInfo,
  LocalSnapshotRef,
} from "../../../src/protocol/generated/protocol";
import { validPublicationDTO } from "./publicationValidation";

export function sameLocalValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a) || Array.isArray(b))
    return (
      Array.isArray(a) &&
      Array.isArray(b) &&
      a.length === b.length &&
      a.every((value, index) => sameLocalValue(value, b[index]))
    );
  if (!a || !b || typeof a !== "object" || typeof b !== "object") return false;
  const aa = a as Record<string, unknown>,
    bb = b as Record<string, unknown>;
  return (
    Object.keys(aa).length === Object.keys(bb).length &&
    Object.keys(aa).every((key) => Object.hasOwn(bb, key) && sameLocalValue(aa[key], bb[key]))
  );
}
export function localPath(path: string): boolean {
  return (
    !!path &&
    Buffer.byteLength(path) <= 4096 &&
    !path.startsWith("/") &&
    !path.includes("\\") &&
    !/^[a-z]:/i.test(path) &&
    !/\p{Cc}/u.test(path) &&
    !path.split("/").some((part) => !part || part === "." || part === "..")
  );
}
export function validLocalComparison(comparison: LocalComparison): boolean {
  return (
    validPublicationDTO("LocalComparison", comparison) &&
    comparison.layers.every(
      (layer, index) =>
        index === 0 ||
        ["committed", "staged", "unstaged"].indexOf(layer) >
          ["committed", "staged", "unstaged"].indexOf(comparison.layers[index - 1]!),
    ) &&
    (comparison.baseRef === null || Buffer.byteLength(comparison.baseRef) <= 256)
  );
}
export function validLocalRef(snapshot: LocalSnapshotRef): boolean {
  return (
    validPublicationDTO("LocalSnapshotRef", snapshot) &&
    (snapshot.source.kind === "current" ||
      sameLocalValue(snapshot.source.repository, snapshot.repository))
  );
}
export function validLocalInfo(info: LocalSnapshotInfo): boolean {
  return (
    validPublicationDTO("LocalSnapshotInfo", info) &&
    validLocalRef(info.snapshot) &&
    validLocalComparison(info.comparison) &&
    sameLocalValue(info.comparison.source, info.snapshot.source) &&
    info.layers.length === info.comparison.layers.length &&
    info.layers.every((layer, index) => layer.kind === info.comparison.layers[index]) &&
    info.layers.reduce((n, layer) => n + layer.fileCount, 0) === info.fileCount &&
    info.layers.reduce((n, layer) => n + layer.patchBytes, 0) === info.patchBytes &&
    info.localReviewId ===
      (Number.parseInt(info.snapshot.snapshotId.slice(0, 8), 16) | 0x80000000) >>> 0 &&
    Buffer.byteLength(info.branch) <= 256 &&
    !/\p{Cc}/u.test(info.branch) &&
    info.warnings.every((warning) => Buffer.byteLength(warning) <= 512 && !/\p{Cc}/u.test(warning))
  );
}
