import { expect, test } from "bun:test";
import diff from "../../protocol/fixtures/valid/response-local-diff.json";
import saved from "../../protocol/fixtures/valid/response-local-list.json";
import options from "../../protocol/fixtures/valid/response-local-options.json";
import profiles from "../../protocol/fixtures/valid/response-local-review-profiles.json";
import type { LocalSnapshotRef, LocalSource } from "../../src/protocol/generated/protocol";
import { localComparisonForm, selectedLocalLayers } from "../src/app/localComparison";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import { loadLocalOptions, loadLocalPatch, localFileKey } from "../src/transport/localEvidence";

class Transport extends FakeBackendTransport {
  handler: (method: string, params: Record<string, unknown>) => Record<string, unknown> =
    () => ({});
  override async request(
    method: string,
    params: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return structuredClone(this.handler(method, params));
  }
}
test("local options reject changed roots across pages and late selections", async () => {
  const transport = new Transport();
  const client = new BackendClient(transport);
  const refs = Array.from({ length: 33 }, (_, index) => `ref-${String(index).padStart(2, "0")}`);
  transport.handler = (_method, params) => ({
    ...options.result,
    request: params,
    baseRefs: refs.slice(params.offset as number, (params.offset as number) + 32),
    total: 33,
    nextOffset: params.offset === 0 ? 32 : null,
  });
  const source = options.result.request.source as LocalSource;
  expect((await loadLocalOptions(client, source, () => true)).baseRefs).toEqual(refs);
  const good = transport.handler;
  transport.handler = (method, params) => ({
    ...good(method, params),
    rootId: params.offset === 0 ? options.result.rootId : "a".repeat(64),
  });
  await expect(loadLocalOptions(client, source, () => true)).rejects.toThrow("changed");
  let current = true;
  transport.handler = (method, params) => {
    current = false;
    return good(method, params);
  };
  await expect(loadLocalOptions(client, source, () => current)).rejects.toThrow(
    "selection changed",
  );
});
test("local patches verify the complete UTF8 hash across pages", async () => {
  const transport = new Transport();
  const client = new BackendClient(transport);
  const text = "界".repeat(22000);
  const sha = new Bun.CryptoHasher("sha256").update(text).digest("hex");
  transport.handler = (_method, params) => {
    const chunk = params.offset === 0 ? text.slice(0, 21000) : text.slice(21000);
    return {
      ...diff.result,
      request: params,
      text: chunk,
      totalBytes: Buffer.byteLength(text),
      contentSha256: sha,
      nextOffset: params.offset === 0 ? Buffer.byteLength(chunk) : null,
    };
  };
  const ref = diff.result.request.snapshot as LocalSnapshotRef;
  expect(await loadLocalPatch(client, ref, "staged", "same.txt", () => true)).toBe(text);
  const good = transport.handler;
  transport.handler = (method, params) => ({
    ...good(method, params),
    contentSha256: "a".repeat(64),
  });
  await expect(loadLocalPatch(client, ref, "staged", "same.txt", () => true)).rejects.toThrow();
  expect(localFileKey({ path: "same.txt", localLayer: "staged" })).not.toBe(
    localFileKey({ path: "same.txt", localLayer: "unstaged" }),
  );
});
test("local profile catalogs reject foreign snapshot and duplicate names", async () => {
  const transport = new Transport();
  const client = new BackendClient(transport);
  transport.handler = () => profiles.result;
  const snapshot = profiles.result.request.snapshot as LocalSnapshotRef;
  expect((await client.localReviewProfiles({ snapshot })).catalog.defaultProfile).toBe("security");
  for (const catalog of [
    { ...profiles.result.catalog, profiles: ["security", "security"] },
    { ...profiles.result.catalog, target: { kind: "local", localSnapshotSha256: "a".repeat(64) } },
  ]) {
    transport.handler = () => ({ ...profiles.result, catalog });
    await expect(client.localReviewProfiles({ snapshot })).rejects.toThrow();
  }
});
test("local source forms distinguish providers and require nonempty layers", () => {
  const form = localComparisonForm([
    { provider: "github", workspace: "owned", repo: "same", localPath: "/owned/a" },
    { provider: "bitbucket", workspace: "owned", repo: "same", localPath: "/owned/b" },
    { provider: "github", workspace: "owned", repo: "missing", localPath: null },
  ]);
  expect(form.sources).toHaveLength(3);
  expect(form.sources[0]?.source.kind).toBe("current");
  expect(selectedLocalLayers(["staged", "unstaged"])).toEqual(["staged", "unstaged"]);
  expect(() => selectedLocalLayers([])).toThrow();
});

test("local browser launches retain the selected snapshot and one explicit layer", async () => {
  const transport = new Transport();
  const client = new BackendClient(transport);
  const snapshot = diff.result.request.snapshot as LocalSnapshotRef;
  const url = `http://127.0.0.1:54321/session/${"e".repeat(64)}/`;
  const good = (method: string, params: Record<string, unknown>) => {
    expect(method).toBe("local.snapshot.browser");
    return { request: params, url, launchRequested: false, launchError: null };
  };
  transport.handler = good;
  expect((await client.localSnapshotBrowser(snapshot, "staged")).request).toEqual({
    snapshot,
    layer: "staged",
  });
  for (const invalid of [
    { url: "https://example.com/session/" },
    { url: `http://127.0.0.1:65536/session/${"e".repeat(64)}/` },
    { launchError: "unexpected launch" },
    { request: { snapshot, layer: "unstaged" } },
    { request: { snapshot: { ...snapshot, rootId: "f".repeat(64) }, layer: "staged" } },
  ]) {
    transport.handler = (method, params) => ({ ...good(method, params), ...invalid });
    await expect(client.localSnapshotBrowser(snapshot, "staged")).rejects.toThrow();
  }
  let current = true;
  transport.handler = (method, params) => {
    current = false;
    return good(method, params);
  };
  await expect(client.localSnapshotBrowser(snapshot, "unstaged", () => current)).rejects.toThrow(
    "selection changed",
  );
});

test("saved local pages reject catalog drift, invalid cursors and guessed clone provenance", async () => {
  const transport = new Transport();
  const client = new BackendClient(transport);
  const params = { offset: 0, limit: 32, catalogVersion: null };
  transport.handler = () => saved.result;
  expect((await client.localSnapshots(params)).snapshots).toHaveLength(1);
  const item = saved.result.snapshots[0]!;
  for (const invalid of [
    { nextOffset: 0 },
    { snapshots: [item, item], total: 2 },
    {
      snapshots: [
        {
          ...item,
          snapshot: {
            ...item.snapshot,
            source: {
              kind: "configured",
              repository: { ...item.snapshot.repository, provider: "bitbucket" },
            },
          },
        },
      ],
    },
    { snapshots: [{ ...item, branch: "unsafe\nbranch" }] },
    { snapshots: [{ ...item, mode: "branch" }] },
    { total: 2, nextOffset: null },
  ]) {
    transport.handler = () => ({ ...saved.result, ...invalid });
    await expect(client.localSnapshots(params)).rejects.toThrow();
  }
  transport.handler = (_method, request) => ({ ...saved.result, request, snapshots: [], total: 1 });
  await expect(
    client.localSnapshots({ offset: 1, limit: 32, catalogVersion: "d".repeat(64) }),
  ).rejects.toThrow();
  await expect(
    client.localSnapshots({ offset: 1, limit: 32, catalogVersion: null }),
  ).rejects.toThrow("catalog version");
});
