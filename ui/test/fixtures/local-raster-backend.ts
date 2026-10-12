import type { LocalSnapshotInfo } from "../../../src/protocol/generated/protocol";
import { LocalTransport } from "./local-backend";
import { newPng, oldPng } from "./raster-bytes";

const modifiedPatch =
  "diff --git a/picture.png b/picture.png\nBinary files a/picture.png and b/picture.png differ\n";
const addedPatch =
  "diff --git a/picture.png b/picture.png\nnew file mode 100644\nBinary files /dev/null and b/picture.png differ\n";
export class LocalRasterTransport extends LocalTransport {
  constructor(readonly kind: "comparison" | "added" | "legacy" | "unsupported" = "comparison") {
    super();
  }
  override async start() {
    const ready = await super.start();
    return { ...ready, capabilities: [...ready.capabilities, "local-captured-images"] };
  }
  override async request(method: string, params: Record<string, unknown>) {
    const patch = this.kind === "added" ? addedPatch : modifiedPatch;
    if (method === "local.snapshot.diff")
      return {
        request: params,
        text: patch,
        totalBytes: Buffer.byteLength(patch),
        contentSha256: new Bun.CryptoHasher("sha256").update(patch).digest("hex"),
        nextOffset: null,
      };
    if (method === "local.snapshot.image") {
      const reason =
        this.kind === "legacy"
          ? "legacySnapshot"
          : this.kind === "unsupported"
            ? "unsupportedMode"
            : this.kind === "added" && params.side === "old"
              ? "absent"
              : null;
      if (reason)
        return {
          request: params,
          available: false,
          mimeType: "",
          size: 0,
          dataBase64: "",
          nextOffset: null,
          contentSha256: null,
          width: null,
          height: null,
          reason,
        };
      const encoded = (params.layer === "staged") === (params.side === "old") ? oldPng : newPng;
      const bytes = Buffer.from(encoded, "base64");
      return {
        request: params,
        available: true,
        mimeType: "image/png",
        size: bytes.length,
        dataBase64: encoded,
        nextOffset: null,
        contentSha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
        width: 2,
        height: 2,
        reason: null,
      };
    }
    const result = await super.request(method, params);
    if (method === "local.snapshot") {
      const info = structuredClone(result.info) as LocalSnapshotInfo;
      info.layers = info.comparison.layers.map((kind) => ({
        kind,
        fileCount: kind === "committed" ? 0 : 1,
        patchBytes: kind === "committed" ? 0 : Buffer.byteLength(patch),
      })) as LocalSnapshotInfo["layers"];
      info.fileCount = info.layers.reduce((n, layer) => n + layer.fileCount, 0);
      info.patchBytes = info.layers.reduce((n, layer) => n + layer.patchBytes, 0);
      return { ...result, info };
    }
    if (method === "local.snapshot.files")
      return {
        ...result,
        total: 2,
        files: ["staged", "unstaged"].map((layer) => ({
          layer,
          path: "picture.png",
          oldPath: null,
          status: this.kind === "added" ? "added" : "modified",
          additions: 0,
          deletions: 0,
          binary: true,
        })),
      };
    return result;
  }
}
