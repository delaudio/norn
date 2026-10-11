import { expect, test } from "bun:test";
import { DataStore } from "../src/app/dataStore";
import { BackendClient, type DataImportPreview } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const preview: DataImportPreview = {
  inputVersion: "a".repeat(64),
  destinationVersion: "b".repeat(64),
  additionsCount: 1,
  unchangedCount: 0,
  conflictsCount: 1,
  rejectedCount: 1,
  detailsTruncated: false,
  additions: ["prompt.github:owner/repo"],
  unchanged: [],
  conflicts: ["drafts.github:owner/repo/7"],
  rejected: [{ entry: 1, reason: "Invalid user-data field." }],
};
class DataTransport extends FakeBackendTransport {
  failImport = false;
  requests: Array<{ method: string; params: Record<string, unknown> }> = [];
  async request(method: string, params: Record<string, unknown>) {
    if (method.startsWith("data.")) this.requests.push({ method, params });
    if (method === "data.preview") return { ...preview };
    if (method === "data.import") {
      if (this.failImport) throw new Error("Import source or destination changed; preview again.");
      return {
        imported: 1,
        unchanged: 0,
        conflicts: 1,
        rejectedCount: 1,
        detailsTruncated: false,
        rejected: preview.rejected,
        backupPath: "/synthetic/backup.json",
        destinationVersion: "c".repeat(64),
      };
    }
    if (method === "data.export") return { exported: 2 };
    return super.request(method, params);
  }
}
function enterPath(store: DataStore, path: string) {
  store.beginEdit();
  store.append(path);
  store.finishEdit();
}
test("data import requires a preview then explicit confirmation and reports kept conflicts", async () => {
  const transport = new DataTransport();
  const client = new BackendClient(transport);
  const store = new DataStore();
  enterPath(store, "/synthetic/export.json");
  await store.confirm(client);
  expect(transport.requests).toHaveLength(0);
  await store.preview(client);
  expect(transport.requests[0]?.method).toBe("data.preview");
  expect(store.getSnapshot().preview?.conflictsCount).toBe(1);
  await store.confirm(client);
  expect(transport.requests[1]?.params).toMatchObject({
    path: "/synthetic/export.json",
    confirm: true,
    inputVersion: preview.inputVersion,
    destinationVersion: preview.destinationVersion,
  });
  expect(store.getSnapshot().notice).toContain("1 conflicts kept");
  expect(store.getSnapshot().notice).toContain("backup.json");
  expect(store.getSnapshot().preview).toBeNull();
});
test("editing source or provider invalidates approval and cancellation leaves no import request", async () => {
  const transport = new DataTransport();
  const client = new BackendClient(transport);
  const store = new DataStore();
  enterPath(store, "/synthetic/export.json");
  await store.preview(client);
  store.move(1);
  store.beginEdit();
  expect(store.getSnapshot().provider).toBe("github");
  expect(store.getSnapshot().preview).toBeNull();
  await store.confirm(client);
  expect(transport.requests).toHaveLength(1);
  store.move(-1);
  store.beginEdit();
  store.append("discarded");
  store.cancelEdit();
  expect(store.getSnapshot().path).toBe("/synthetic/export.json");
  store.dispose();
  expect(store.getSnapshot().path).toBe("");
  expect(store.getSnapshot().preview).toBeNull();
});
test("failed import invalidates approval and retains source for a fresh preview", async () => {
  const transport = new DataTransport();
  transport.failImport = true;
  const client = new BackendClient(transport);
  const store = new DataStore();
  enterPath(store, "/synthetic/export.json");
  await store.preview(client);
  await store.confirm(client);
  expect(store.getSnapshot().error).toContain("changed");
  expect(store.getSnapshot().path).toBe("/synthetic/export.json");
  expect(store.getSnapshot().preview).toBeNull();
  await store.confirm(client);
  expect(transport.requests).toHaveLength(2);
});
test("path paste rejects overflow and control input without applying partial text", () => {
  const store = new DataStore();
  store.beginEdit();
  store.append("/synthetic/");
  store.append("x".repeat(4097));
  expect(store.getSnapshot().editText).toBe("/synthetic/");
  store.append("\nrun-review");
  expect(store.getSnapshot().editText).toBe("/synthetic/");
});
