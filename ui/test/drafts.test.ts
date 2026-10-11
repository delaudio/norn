import { expect, test } from "bun:test";
import type { WorkspaceDraft } from "../../src/protocol/generated/protocol";
import { DraftStore, draftFields, validDraft } from "../src/app/draftStore";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const entry = {
  provider: "github" as const,
  workspace: "owner",
  repo: "repo",
  prId: 7,
  runId: null,
  title: "Review",
  status: "completed",
  kind: "review" as const,
};
const imported: WorkspaceDraft = {
  localId: "imported",
  prId: 7,
  path: "src/test.ts",
  to: 12,
  from: null,
  raw: "Original guidance 世界",
  parentId: "19",
  createdAt: 1,
  source: "aiFinding",
  publicationMode: "inline",
  findingRef: { reviewRunId: "run-1", findingId: "finding-1", findingFingerprint: "fingerprint-1" },
  reviewBaseSha: "a".repeat(40),
  reviewHeadSha: "b".repeat(40),
};
function field(store: DraftStore, name: (typeof draftFields)[number], value: string) {
  while (draftFields[store.getSnapshot().field] !== name) store.move(1);
  store.beginField();
  store.clearField();
  store.append(value);
  store.finishField();
}
class Transport extends FakeBackendTransport {
  requests: string[] = [];
  conflict = false;
  malformed = false;
  hold: Promise<void> | null = null;
  async request(method: string, params: Record<string, unknown>) {
    this.requests.push(method);
    if (this.hold && method === "workspace.read") await this.hold;
    if (this.conflict && method === "workspace.mutate")
      throw new Error("User data changed; refresh before saving.");
    const result = await super.request(method, params);
    if (this.malformed && method === "workspace.read")
      return { ...result, payload: { kind: "drafts", value: [{ ...imported, raw: 123 }] } };
    return result;
  }
}
async function fixture() {
  const transport = new Transport({
    workspaceRecords: {
      "drafts.github:owner/repo/7": { kind: "drafts", value: [imported] },
      "drafts.bitbucket:owner/repo/7": {
        kind: "drafts",
        value: [{ ...imported, localId: "other-provider", raw: "Other provider content" }],
      },
    },
  });
  const client = new BackendClient(transport);
  const store = new DraftStore(
    () => 123,
    () => "new-draft",
  );
  await store.load(client, entry);
  return { transport, client, store };
}

test("imported draft edits preserve identity/source/timestamps/metadata and isolate provider scope", async () => {
  const { transport, client, store } = await fixture();
  expect(store.getSnapshot().items[0]).toEqual(imported);
  store.edit();
  field(store, "raw", "Updated guidance 世界\n````");
  await store.save(client);
  expect(store.getSnapshot().items[0]).toEqual({
    ...imported,
    raw: "Updated guidance 世界\n````",
  });
  const same = await client.readWorkspace("drafts.github:owner/repo/7");
  expect(same.payload.value).toEqual(store.getSnapshot().items);
  const other = await client.readWorkspace("drafts.bitbucket:owner/repo/7");
  expect(other.payload.value).toEqual([
    { ...imported, localId: "other-provider", raw: "Other provider content" },
  ]);
  expect(
    transport.requests.every((method) => ["workspace.read", "workspace.mutate"].includes(method)),
  ).toBe(true);
});

test("new general comments, cancellation, explicit removal and invalid line/paste input never publish", async () => {
  const { transport, client, store } = await fixture();
  store.add();
  field(store, "path", "");
  field(store, "raw", "Do not run q\nCtrl+S is text");
  expect(transport.requests.filter((method) => method === "workspace.mutate")).toHaveLength(0);
  await store.save(client);
  expect(store.getSnapshot().items).toHaveLength(2);
  expect(store.getSnapshot().items[1]).toMatchObject({
    localId: "new-draft",
    publicationMode: "general",
    source: "manual",
    createdAt: 123,
  });
  store.edit();
  while (draftFields[store.getSnapshot().field] !== "to") store.move(1);
  store.beginField();
  store.append("0");
  store.finishField();
  expect(store.getSnapshot().editing).toBe(true);
  expect(store.getSnapshot().error).toContain("positive");
  store.clearField();
  store.append("1\u001b[31m");
  expect(store.getSnapshot().text).toBe("");
  store.cancel();
  store.cancel();
  await store.remove(client);
  expect(store.getSnapshot().items).toHaveLength(2);
  store.requestRemoval();
  store.cancel();
  await store.remove(client);
  expect(store.getSnapshot().items).toHaveLength(2);
  store.requestRemoval();
  await store.remove(client);
  expect(store.getSnapshot().items).toEqual([imported]);
});

test("stale save retains edited draft and duplicate save input coalesces", async () => {
  const { transport, client, store } = await fixture();
  store.edit();
  field(store, "raw", "Unsaved content");
  transport.conflict = true;
  await store.save(client);
  expect(store.getSnapshot().draft?.raw).toBe("Unsaved content");
  expect(store.getSnapshot().error).toContain("changed");
  expect(store.getSnapshot().items).toEqual([imported]);
  transport.conflict = false;
  const first = store.save(client);
  await store.save(client);
  await first;
  expect(transport.requests.filter((method) => method === "workspace.mutate")).toHaveLength(2);
});

test("working-tree scope, malformed responses, late reads and invalid timestamps fail safely", async () => {
  const { transport, client, store } = await fixture();
  await store.load(client, { ...entry, prId: null });
  expect(store.getSnapshot().key).toBeNull();
  expect(store.getSnapshot().error).toContain("Select a pull request");
  transport.malformed = true;
  await store.load(client, entry);
  expect(store.getSnapshot().items).toEqual([]);
  expect(store.getSnapshot().error).toContain("Invalid saved");
  transport.malformed = false;
  let release!: () => void;
  transport.hold = new Promise((resolve) => {
    release = resolve;
  });
  const pending = store.load(client, entry);
  store.dispose();
  release();
  await pending;
  expect(store.getSnapshot().items).toEqual([]);
  expect(store.getSnapshot().error).toBeNull();
  expect(validDraft({ ...imported, createdAt: NaN })).toBe(false);
  expect(validDraft({ ...imported, localId: "", unknown: true })).toBe(false);
});

test("draft validation fences wrong PRs, controls, unknown fields and large Unicode bodies", async () => {
  const { client, store } = await fixture();
  expect(validDraft({ ...imported, to: 0 })).toBe(false);
  expect(validDraft({ ...imported, from: 4294967296 })).toBe(false);
  expect(validDraft({ ...imported, raw: "界".repeat(21846) })).toBe(false);
  expect(validDraft({ ...imported, path: "src/\u001b[31m" })).toBe(false);
  expect(validDraft({ ...imported, findingRef: { ...imported.findingRef, extra: true } })).toBe(
    false,
  );
  expect(validDraft({ ...imported, findingRef: {} })).toBe(false);
  expect(validDraft({ ...imported, publicationMode: "arbitrary" })).toBe(false);
  const wrong = new BackendClient(
    new FakeBackendTransport({
      workspaceRecords: {
        "drafts.github:owner/repo/7": { kind: "drafts", value: [{ ...imported, prId: 8 }] },
      },
    }),
  );
  await store.load(wrong, entry);
  expect(store.getSnapshot().error).toContain("Invalid saved");
  store.add();
  expect(store.getSnapshot().draft).toBeNull();
  client.close();
  wrong.close();
});

test("late durable saves never update another PR's panel and empty fields remain strings", async () => {
  let release!: () => void;
  let hold: Promise<void> | null = null;
  class Delayed extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      const result = await super.request(method, params);
      if (method === "workspace.mutate" && hold) await hold;
      return result;
    }
  }
  const client = new BackendClient(
    new Delayed({
      workspaceRecords: {
        "drafts.github:owner/repo/7": { kind: "drafts", value: [imported] },
      },
    }),
  );
  const store = new DraftStore();
  await store.load(client, entry);
  store.edit();
  field(store, "raw", "");
  field(store, "path", "");
  hold = new Promise((resolve) => {
    release = resolve;
  });
  const save = store.save(client);
  await Bun.sleep(0);
  await store.load(client, { ...entry, prId: 8 });
  release();
  await save;
  expect(store.getSnapshot().key).toBe("drafts.github:owner/repo/8");
  expect(store.getSnapshot().items).toEqual([]);
  expect(store.getSnapshot().notice).toBeNull();
  const previous = await client.readWorkspace("drafts.github:owner/repo/7");
  expect(previous.payload.value).toEqual([{ ...imported, raw: "", path: "" }]);
  client.close();
});
