import files from "../../../protocol/fixtures/valid/response-local-files.json";
import options from "../../../protocol/fixtures/valid/response-local-options.json";
import findings from "../../../protocol/fixtures/valid/response-local-review-findings.json";
import history from "../../../protocol/fixtures/valid/response-local-review-history.json";
import profiles from "../../../protocol/fixtures/valid/response-local-review-profiles.json";
import threads from "../../../protocol/fixtures/valid/response-local-review-threads.json";
import capture from "../../../protocol/fixtures/valid/response-local-snapshot.json";
import type {
  LocalCaptureParams,
  LocalOptionsParams,
} from "../../../src/protocol/generated/protocol";
import { FakeBackendTransport } from "../../src/transport/fakeTransport";
export class LocalTransport extends FakeBackendTransport {
  calls: string[] = [];
  override async start() {
    const ready = await super.start();
    return {
      ...ready,
      capabilities: [
        ...ready.capabilities,
        "local-snapshots",
        "local-evidence-reviews",
        "local-profile-catalog",
      ],
    };
  }
  override async request(
    method: string,
    params: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    this.calls.push(method);
    if (method === "local.options") {
      const source = (params as unknown as LocalOptionsParams).source;
      return structuredClone({
        ...options.result,
        request: params,
        repository: source.kind === "configured" ? source.repository : options.result.repository,
        rootId:
          source.kind === "configured"
            ? (source.repository.provider === "github" ? "1" : "2").repeat(64)
            : options.result.rootId,
      });
    }
    if (method === "local.snapshot") {
      const comparison = (params as unknown as LocalCaptureParams).comparison;
      const source = comparison.source;
      const info = { ...structuredClone(capture.result.info), comparison };
      info.snapshot.source = source;
      info.snapshot.repository =
        source.kind === "configured" ? source.repository : info.snapshot.repository;
      info.snapshot.rootId =
        source.kind === "configured"
          ? (source.repository.provider === "github" ? "1" : "2").repeat(64)
          : info.snapshot.rootId;
      info.snapshot.snapshotId =
        source.kind === "configured"
          ? (source.repository.provider === "github" ? "3" : "4").repeat(64)
          : info.snapshot.snapshotId;
      info.localReviewId =
        (Number.parseInt(info.snapshot.snapshotId.slice(0, 8), 16) | 0x80000000) >>> 0;
      info.layers = info.layers.filter((layer) =>
        comparison.layers.some((selected) => selected === layer.kind),
      );
      return { ...capture.result, request: params, info };
    }
    if (method === "local.review.profiles")
      return {
        ...profiles.result,
        request: params,
        catalog: {
          ...profiles.result.catalog,
          target: {
            kind: "local",
            localSnapshotSha256: (params.snapshot as { snapshotId: string }).snapshotId,
          },
        },
      };
    if (method === "local.snapshot.files") return { ...files.result, request: params };
    if (method === "local.review.history")
      return { ...history.result, request: params, runs: [], total: 0, nextOffset: null };
    if (method === "local.review.findings")
      return {
        ...findings.result,
        request: params,
        runId: null,
        findings: [],
        total: 0,
        nextOffset: null,
      };
    if (method === "local.review.threads")
      return { ...threads.result, request: params, runId: null, threads: [], truncated: false };
    return super.request(method, params);
  }
}
