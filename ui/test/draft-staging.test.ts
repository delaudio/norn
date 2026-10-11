import { expect, test } from "bun:test";
import { DraftStagingStore } from "../src/app/draftStagingStore";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import { validDraft } from "../src/transport/workspaceValidation";

const target = {
  kind: "provider" as const,
  provider: "github" as const,
  workspace: "owner",
  repo: "repo",
  prId: 7,
  runId: "run-1",
};
const finding = {
  id: "f1",
  title: "Validate input",
  severity: "high",
  summary: "Saved Unicode 世界\r\nSecond line.",
  anchor: { path: "src/test.ts", startLine: 12, side: "old" },
};
class Transport extends FakeBackendTransport {
  calls: string[] = [];
  conflict = false;
  malformed = false;
  hold: Promise<void> | null = null;
  async request(method: string, params: Record<string, unknown>) {
    this.calls.push(method);
    if (method === "draft.prepare" && this.hold) await this.hold;
    if (method === "draft.stage" && this.conflict)
      throw new Error("Drafts changed; refresh the preview.");
    const result = await super.request(method, params);
    if (method === "draft.prepare" && this.malformed)
      return { ...result, target: { ...target, provider: "bitbucket" } };
    return result;
  }
}
function fixture(anchored = true) {
  const transport = new Transport({
    findings: [{ ...finding, anchor: anchored ? finding.anchor : null }],
  });
  const client = new BackendClient(transport);
  const store = new DraftStagingStore();
  store.open(target, finding.id);
  return { transport, client, store };
}

test("finding staging needs an explicit preview and stage; persisted source, side and edits survive duplicate staging", async () => {
  const { transport, client, store } = fixture();
  await store.stage(client);
  expect(transport.calls).toEqual([]);
  await store.preview(client);
  const prepared = store.getSnapshot().prepared!;
  expect(prepared.draft).toMatchObject({
    from: 12,
    to: null,
    source: "aiFinding",
    findingRef: { reviewRunId: "run-1", findingId: "f1" },
  });
  expect(prepared.draft.raw).toContain("世界\r\nSecond line.");
  expect(validDraft(prepared.draft)).toBe(true);
  expect(transport.calls).toEqual(["draft.prepare"]);
  const first = store.stage(client);
  await store.stage(client);
  await first;
  expect(store.getSnapshot().done).toBe(true);
  expect(transport.calls.filter((method) => method === "draft.stage")).toHaveLength(1);
  const saved = await client.readWorkspace(prepared.key);
  await client.mutateWorkspace(prepared.key, saved.recordVersion, {
    kind: "putDraft",
    draft: { ...prepared.draft, raw: "Reviewer edit", publicationMode: "general" },
  });
  const reopened = new DraftStagingStore();
  reopened.open(target, finding.id);
  await reopened.preview(client);
  expect(reopened.getSnapshot().prepared).toMatchObject({
    alreadyStaged: true,
    draft: { raw: "Reviewer edit", publicationMode: "general" },
  });
  await reopened.stage(client);
  const after = await client.readWorkspace(prepared.key);
  expect(after.payload.value).toHaveLength(1);
  expect(after.payload.value).toEqual([
    { ...prepared.draft, raw: "Reviewer edit", publicationMode: "general" },
  ]);
  expect(
    transport.calls.every(
      (method) => method.startsWith("draft.") || method.startsWith("workspace."),
    ),
  ).toBe(true);
  client.close();
});

test("non-anchored findings require an explicit general/local-only choice and mode changes discard the old preview", async () => {
  const { transport, client, store } = fixture(false);
  await store.preview(client);
  expect(store.getSnapshot().error).toContain("no supported file position");
  expect(store.getSnapshot().prepared).toBeNull();
  store.moveMode(1);
  await store.preview(client);
  expect(store.getSnapshot().error).toContain("no supported file position");
  store.moveMode(1);
  await store.preview(client);
  expect(store.getSnapshot().prepared?.draft).toMatchObject({
    publicationMode: "general",
    path: "",
    to: null,
    from: null,
  });
  store.moveMode(1);
  expect(store.getSnapshot().prepared).toBeNull();
  await store.stage(client);
  expect(transport.calls.filter((method) => method === "draft.stage")).toHaveLength(0);
  await store.preview(client);
  expect(store.getSnapshot().prepared?.draft.publicationMode).toBe("localOnly");
  store.dismissPreview();
  await store.stage(client);
  expect(transport.calls.filter((method) => method === "draft.stage")).toHaveLength(0);
  client.close();
});

test("stale CAS retains preview, malformed target never stages and late reads cannot populate another scope", async () => {
  const { transport, client, store } = fixture();
  await store.preview(client);
  transport.conflict = true;
  await store.stage(client);
  expect(store.getSnapshot().done).toBe(false);
  expect(store.getSnapshot().error).toContain("changed");
  expect(store.getSnapshot().prepared?.draft.raw).toContain("世界");
  transport.malformed = true;
  await store.preview(client);
  expect(store.getSnapshot().error).toContain("Invalid finding draft preview");
  expect(store.getSnapshot().prepared).toBeNull();
  transport.malformed = false;
  let release!: () => void;
  transport.hold = new Promise((resolve) => {
    release = resolve;
  });
  const pending = store.preview(client);
  store.open({ ...target, provider: "bitbucket" }, finding.id);
  release();
  await pending;
  expect(store.getSnapshot().prepared).toBeNull();
  expect(store.getSnapshot().target?.provider).toBe("bitbucket");
  expect(store.getSnapshot().busy).toBe(false);
  client.close();
});

test("late staging acknowledgments never confirm a different scope, while the authorized save remains durable", async () => {
  let release!: () => void;
  let hold: Promise<void> | null = null;
  class Delayed extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      const result = await super.request(method, params);
      if (method === "draft.stage" && hold) await hold;
      return result;
    }
  }
  const client = new BackendClient(new Delayed({ findings: [finding] }));
  const store = new DraftStagingStore();
  store.open(target, finding.id);
  await store.preview(client);
  const key = store.getSnapshot().prepared!.key;
  hold = new Promise((resolve) => {
    release = resolve;
  });
  const pending = store.stage(client);
  await Bun.sleep(0);
  store.open({ ...target, prId: 8 }, finding.id);
  release();
  await pending;
  expect(store.getSnapshot().target?.prId).toBe(8);
  expect(store.getSnapshot().done).toBe(false);
  expect(store.getSnapshot().prepared).toBeNull();
  expect((await client.readWorkspace(key)).payload.value).toHaveLength(1);
  expect((await client.readWorkspace("drafts.github:owner/repo/8")).payload.value).toEqual([]);
  client.close();
});

test("runtime previews and acknowledgments reject malformed metadata, extra fields and contradictory duplicate status", async () => {
  let fault: "preview" | "ack" | "none" = "preview";
  class Faulty extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      const result = await super.request(method, params);
      if (method === "draft.prepare" && fault === "preview") return { ...result, extra: "unknown" };
      if (method === "draft.stage" && fault === "ack")
        return { ...result, changed: true, alreadyStaged: true };
      return result;
    }
  }
  const client = new BackendClient(new Faulty({ findings: [finding] }));
  const store = new DraftStagingStore();
  store.open(target, finding.id);
  await store.preview(client);
  expect(store.getSnapshot().error).toContain("Invalid finding draft preview");
  fault = "none";
  await store.preview(client);
  fault = "ack";
  await store.stage(client);
  expect(store.getSnapshot().done).toBe(false);
  expect(store.getSnapshot().error).toContain("acknowledgement is invalid");
  expect((await client.readWorkspace("drafts.github:owner/repo/7")).payload.value).toHaveLength(1);
  client.close();
});
