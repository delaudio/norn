import type {
  LocalSnapshotFile,
  LocalSnapshotInfo,
  LocalSnapshotRef,
  LocalSource,
  PrRepositoryIdentity,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "./backendClient";
import { sameLocalValue, validLocalInfo } from "./localValidation";

export type LocalLayer = LocalSnapshotFile["layer"];
export interface LocalSourceOptions {
  source: LocalSource;
  repository: PrRepositoryIdentity;
  rootId: string;
  branch: string | null;
  headSha: string | null;
  baseRefs: string[];
}
function guard(current: () => boolean): void {
  if (!current()) throw new Error("The local selection changed.");
}
export async function loadLocalOptions(
  client: BackendClient,
  source: LocalSource,
  current: () => boolean,
): Promise<LocalSourceOptions> {
  let offset = 0;
  let result: LocalSourceOptions | null = null;
  let total: number | null = null;
  for (let page = 0; page < 32; page++) {
    guard(current);
    const value = await client.localOptions({ source, offset, limit: 32 });
    guard(current);
    const identity = {
      source,
      repository: value.repository,
      rootId: value.rootId,
      branch: value.branch,
      headSha: value.headSha,
    };
    if (
      result &&
      (!sameLocalValue({ ...result, baseRefs: [] }, { ...identity, baseRefs: [] }) ||
        value.total !== total)
    )
      throw new Error("The local source changed while loading base refs.");
    result ??= { ...identity, baseRefs: [] };
    total ??= value.total;
    for (const name of value.baseRefs) {
      const previous = result.baseRefs.at(-1);
      if (previous !== undefined && Buffer.compare(Buffer.from(previous), Buffer.from(name)) >= 0)
        throw new Error("Invalid local base ref ordering.");
      result.baseRefs.push(name);
    }
    if (value.nextOffset === null) return result;
    if (value.nextOffset <= offset) throw new Error("The local base ref page did not advance.");
    offset = value.nextOffset;
  }
  throw new Error("The local base ref list exceeds its page limit.");
}
export function localFileKey(file: {
  path: string;
  localLayer?: LocalLayer;
  layer?: LocalLayer;
}): string {
  const layer = file.localLayer ?? file.layer;
  return layer ? JSON.stringify([layer, file.path]) : file.path;
}
export async function loadLocalFiles(
  client: BackendClient,
  info: LocalSnapshotInfo,
  current: () => boolean,
): Promise<LocalSnapshotFile[]> {
  if (!validLocalInfo(info)) throw new Error("Invalid local snapshot metadata.");
  const files: LocalSnapshotFile[] = [];
  const keys = new Set<string>();
  const counts = new Map<LocalLayer, number>();
  let offset = 0,
    bytes = 0;
  for (let page = 0; page < 189; page++) {
    guard(current);
    const value = await client.localFiles({ snapshot: info.snapshot, offset, limit: 32 });
    guard(current);
    bytes += Buffer.byteLength(JSON.stringify(value.files));
    if (value.total !== info.fileCount || bytes > 32 * 1024 * 1024)
      throw new Error("Invalid local file inventory size.");
    for (const file of value.files) {
      const key = localFileKey(file);
      if (keys.has(key) || !info.comparison.layers.some((layer) => layer === file.layer))
        throw new Error("Invalid or duplicated local layer file.");
      keys.add(key);
      files.push(file);
      counts.set(file.layer, (counts.get(file.layer) ?? 0) + 1);
    }
    if (value.nextOffset === null) {
      if (info.layers.some((layer) => (counts.get(layer.kind) ?? 0) !== layer.fileCount))
        throw new Error("Local layer inventory does not match its captured metadata.");
      return files;
    }
    if (value.nextOffset <= offset) throw new Error("The local file page did not advance.");
    offset = value.nextOffset;
  }
  throw new Error("The local file inventory exceeds its page limit.");
}
export async function loadLocalPatch(
  client: BackendClient,
  snapshot: LocalSnapshotRef,
  layer: LocalLayer,
  path: string,
  current: () => boolean,
): Promise<string> {
  const parts: string[] = [];
  const hasher = new Bun.CryptoHasher("sha256");
  let offset = 0,
    total: number | null = null,
    sha: string | null = null;
  // A UTF-8 chunk may use up to three fewer bytes than 64KiB.
  for (let page = 0; page < 258; page++) {
    guard(current);
    const value = await client.localDiff({ snapshot, layer, path, offset });
    guard(current);
    total ??= value.totalBytes;
    sha ??= value.contentSha256;
    if (value.totalBytes !== total || value.contentSha256 !== sha || total > 16 * 1024 * 1024)
      throw new Error("The captured local patch identity changed.");
    const bytes = Buffer.byteLength(value.text);
    if (offset + bytes > total) throw new Error("The local patch exceeds its captured size.");
    parts.push(value.text);
    hasher.update(value.text);
    if (value.nextOffset === null) {
      if (offset + bytes !== total || hasher.digest("hex") !== sha)
        throw new Error("The captured local patch hash does not match.");
      return parts.join("");
    }
    if (value.nextOffset <= offset) throw new Error("The local patch page did not advance.");
    offset = value.nextOffset;
  }
  throw new Error("The captured local patch exceeds its page limit.");
}
