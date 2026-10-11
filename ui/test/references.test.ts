import { expect, test } from "bun:test";
import type { WorkspaceReference } from "../../src/protocol/generated/protocol";
import { ReferenceStore, referenceFields, validReference } from "../src/app/referenceStore";
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
const imported: WorkspaceReference = {
  id: "imported",
  type: "repository",
  source: "detected",
  title: "Shared contract",
  workspace: "other",
  repo: "lib",
  localPath: "/synthetic/read-only",
  url: "https://example.invalid/reference",
  body: "Original guidance 世界",
  createdAt: 1,
  updatedAt: 2,
};
function field(store: ReferenceStore, name: (typeof referenceFields)[number], value: string) {
  while (referenceFields[store.getSnapshot().field] !== name) store.move(1);
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
      return { ...result, payload: { kind: "references", value: [{ ...imported, body: 123 }] } };
    return result;
  }
}
async function fixture() {
  const transport = new Transport({
    workspaceRecords: {
      "references.github:owner/repo/7": { kind: "references", value: [imported] },
      "references.bitbucket:owner/repo/7": {
        kind: "references",
        value: [{ ...imported, id: "other-provider", body: "Other provider content" }],
      },
    },
  });
  const client = new BackendClient(transport);
  const store = new ReferenceStore(
    () => 123,
    () => "new-ref",
  );
  await store.load(client, entry);
  return { transport, client, store };
}

test("imported reference edits preserve identity/source/timestamps/metadata and isolate provider scope", async () => {
  const { transport, client, store } = await fixture();
  expect(store.getSnapshot().items[0]).toEqual(imported);
  store.edit();
  field(store, "body", "Updated guidance 世界\n````");
  await store.save(client);
  expect(store.getSnapshot().items[0]).toEqual({
    ...imported,
    body: "Updated guidance 世界\n````",
    updatedAt: 123,
  });
  const same = await client.readWorkspace("references.github:owner/repo/7");
  expect(same.payload.value).toEqual(store.getSnapshot().items);
  const other = await client.readWorkspace("references.bitbucket:owner/repo/7");
  expect(other.payload.value).toEqual([
    { ...imported, id: "other-provider", body: "Other provider content" },
  ]);
  expect(
    transport.requests.every((method) => ["workspace.read", "workspace.mutate"].includes(method)),
  ).toBe(true);
});

test("new notes, cancellation, explicit removal and invalid PR/paste input never publish", async () => {
  const { transport, client, store } = await fixture();
  store.add();
  field(store, "title", "New note");
  field(store, "body", "Do not run q\nCtrl+S is text");
  expect(transport.requests.filter((method) => method === "workspace.mutate")).toHaveLength(0);
  await store.save(client);
  expect(store.getSnapshot().items).toHaveLength(2);
  expect(store.getSnapshot().items[1]).toMatchObject({
    id: "new-ref",
    type: "note",
    source: "manual",
    createdAt: 123,
    updatedAt: 123,
  });
  store.edit();
  while (referenceFields[store.getSnapshot().field] !== "prId") store.move(1);
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

test("stale save retains edited reference and duplicate save input coalesces", async () => {
  const { transport, client, store } = await fixture();
  store.edit();
  field(store, "body", "Unsaved content");
  transport.conflict = true;
  await store.save(client);
  expect(store.getSnapshot().draft?.body).toBe("Unsaved content");
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
  expect(validReference({ ...imported, updatedAt: NaN })).toBe(false);
  expect(validReference({ ...imported, id: "", unknown: true })).toBe(false);
});
