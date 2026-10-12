import { expect, test } from "bun:test";
import diffFixture from "../../protocol/fixtures/valid/response-local-diff.json";
import filesFixture from "../../protocol/fixtures/valid/response-local-files.json";
import infoFixture from "../../protocol/fixtures/valid/response-local-info.json";
import optionsFixture from "../../protocol/fixtures/valid/response-local-options.json";
import recheckFixture from "../../protocol/fixtures/valid/response-local-recheck.json";
import captureFixture from "../../protocol/fixtures/valid/response-local-snapshot.json";
import type {
  LocalCaptureParams,
  LocalDiffParams,
  LocalFilesParams,
  LocalLookupParams,
  LocalOptionsParams,
  LocalRecheckParams,
  LocalSnapshotRef,
} from "../../src/protocol/generated/protocol";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

class LocalTransport extends FakeBackendTransport {
  calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  replies = new Map<string, Record<string, unknown>>();
  override async request(
    method: string,
    params: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    this.calls.push({ method, params });
    return structuredClone(this.replies.get(method) ?? {});
  }
}
function fixture() {
  const transport = new LocalTransport();
  for (const [method, value] of [
    ["local.snapshot", captureFixture],
    ["local.options", optionsFixture],
    ["local.snapshot.files", filesFixture],
    ["local.snapshot.diff", diffFixture],
    ["local.snapshot.info", infoFixture],
    ["local.snapshot.recheck", recheckFixture],
  ] as const)
    transport.replies.set(method, structuredClone(value.result));
  return { transport, client: new BackendClient(transport) };
}
test("typed local facade accepts full immutable identity and distinct comparison layers", async () => {
  const { transport, client } = fixture();
  expect(
    (await client.localCapture(captureFixture.result.request as LocalCaptureParams)).info.snapshot,
  ).toEqual(captureFixture.result.info.snapshot as LocalSnapshotRef);
  expect(
    (await client.localOptions(optionsFixture.result.request as LocalOptionsParams)).baseRefs,
  ).toEqual(["comparison"]);
  expect(
    (await client.localInfo(infoFixture.result.request as LocalLookupParams)).info.fileCount,
  ).toBe(0);
  expect((await client.localFiles(filesFixture.result.request as LocalFilesParams)).files).toEqual(
    [],
  );
  expect((await client.localDiff(diffFixture.result.request as LocalDiffParams)).text).toBe("");
  expect(
    (await client.localRecheck(recheckFixture.result.request as LocalRecheckParams)).unchanged,
  ).toBe(true);
  expect(transport.calls.every((call) => call.method.startsWith("local."))).toBe(true);
});
test("local facade rejects caller paths, malformed handles and unordered layers before transport", async () => {
  const { transport, client } = fixture();
  const requests = [
    {
      ...captureFixture.result.request,
      comparison: {
        ...captureFixture.result.request.comparison,
        source: { kind: "current", path: "/foreign" },
      },
    },
    {
      ...captureFixture.result.request,
      comparison: { ...captureFixture.result.request.comparison, layers: ["staged", "staged"] },
    },
    {
      ...captureFixture.result.request,
      comparison: { ...captureFixture.result.request.comparison, layers: ["unstaged", "staged"] },
    },
    { ...captureFixture.result.request, captureId: "A".repeat(64) },
    {
      ...captureFixture.result.request,
      comparison: { ...captureFixture.result.request.comparison, baseRef: "unlisted" },
    },
  ];
  for (const request of requests)
    await expect(client.localCapture(request as LocalCaptureParams)).rejects.toThrow();
  await expect(
    client.localDiff({ ...diffFixture.result.request, path: "../foreign" } as LocalDiffParams),
  ).rejects.toThrow();
  expect(transport.calls).toHaveLength(0);
});
test("local facade rejects altered source, inventory, ID, comparison and response echo", async () => {
  const bad = [
    {
      info: {
        ...captureFixture.result.info,
        snapshot: {
          ...captureFixture.result.info.snapshot,
          source: {
            kind: "configured",
            repository: captureFixture.result.info.snapshot.repository,
          },
        },
      },
    },
    { info: { ...captureFixture.result.info, localReviewId: 0 } },
    { info: { ...captureFixture.result.info, fileCount: 1 } },
    {
      info: {
        ...captureFixture.result.info,
        comparison: {
          ...captureFixture.result.info.comparison,
          layers: ["unstaged", "staged", "committed"],
        },
      },
    },
    { info: { ...captureFixture.result.info, warnings: ["世界".repeat(300)] } },
    { info: { ...captureFixture.result.info, privatePath: "/foreign" } },
    { request: { ...captureFixture.result.request, captureId: "b".repeat(64) } },
  ];
  for (const mutation of bad) {
    const { transport, client } = fixture();
    transport.replies.set("local.snapshot", { ...captureFixture.result, ...mutation });
    await expect(
      client.localCapture(captureFixture.result.request as LocalCaptureParams),
    ).rejects.toThrow();
  }
  for (const mutation of [
    { rootId: "d".repeat(64) },
    { snapshotId: "d".repeat(64) },
    { repository: { provider: "bitbucket", workspace: "owned", repo: "same" } },
  ]) {
    const { transport, client } = fixture();
    transport.replies.set("local.snapshot.info", {
      ...infoFixture.result,
      info: {
        ...infoFixture.result.info,
        snapshot: { ...infoFixture.result.info.snapshot, ...mutation },
      },
    });
    await expect(
      client.localInfo(infoFixture.result.request as LocalLookupParams),
    ).rejects.toThrow();
  }
});
test("local facade rejects contradictory pages, traversal, UTF8 excess and mismatched hash", async () => {
  const { transport, client } = fixture();
  transport.replies.set("local.options", { ...optionsFixture.result, nextOffset: 1 });
  await expect(
    client.localOptions(optionsFixture.result.request as LocalOptionsParams),
  ).rejects.toThrow();
  for (const text of ["世界".repeat(22000), "tampered"]) {
    transport.replies.set("local.snapshot.diff", {
      ...diffFixture.result,
      text,
      totalBytes: Buffer.byteLength(text),
    });
    await expect(client.localDiff(diffFixture.result.request as LocalDiffParams)).rejects.toThrow();
  }
  transport.replies.set("local.snapshot.files", {
    ...filesFixture.result,
    files: [
      {
        layer: "staged",
        path: "../foreign",
        oldPath: null,
        status: "modified",
        additions: 1,
        deletions: 1,
        binary: false,
      },
    ],
    total: 1,
  });
  await expect(
    client.localFiles(filesFixture.result.request as LocalFilesParams),
  ).rejects.toThrow();
});
