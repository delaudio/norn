import capture from "../../../protocol/fixtures/valid/response-local-snapshot.json";
import type { LocalSnapshotInfo, LocalSnapshotRef } from "../../../src/protocol/generated/protocol";
import { LocalTransport } from "./local-backend";
export class SavedTransport extends LocalTransport {
  savedReads = 0;
  savedInfo = Array.from({ length: 35 }, (_, index): LocalSnapshotInfo => {
    const info = structuredClone(capture.result.info) as LocalSnapshotInfo;
    info.snapshot.rootId = "9".repeat(64);
    info.snapshot.snapshotId = new Bun.CryptoHasher("sha256")
      .update(`owned-saved-${index}`)
      .digest("hex");
    info.localReviewId =
      (Number.parseInt(info.snapshot.snapshotId.slice(0, 8), 16) | 0x80000000) >>> 0;
    return info;
  });
  override async start() {
    const ready = await super.start();
    return { ...ready, capabilities: [...ready.capabilities, "local-saved-snapshots"] };
  }
  override async request(method: string, params: Record<string, unknown>) {
    if (method === "local.snapshot.list") {
      this.savedReads++;
      const offset = Number(params.offset);
      return {
        request: params,
        catalogVersion: "e".repeat(64),
        total: 35,
        nextOffset: offset === 0 ? 32 : null,
        snapshots: this.savedInfo.slice(offset, offset + 32).map((info) => ({
          snapshot: info.snapshot,
          createdAt: "1791794152000",
          mode: info.comparison.mode,
          branch: info.branch,
          baseSha: info.baseSha,
          layers: info.comparison.layers,
          fileCount: info.fileCount,
        })),
      };
    }
    if (method === "local.snapshot.info") {
      const snapshot = params.snapshot as LocalSnapshotRef;
      const info = this.savedInfo.find((item) => item.snapshot.snapshotId === snapshot.snapshotId);
      if (!info) throw new Error("Unknown saved fixture");
      return { request: params, info };
    }
    return super.request(method, params);
  }
}
