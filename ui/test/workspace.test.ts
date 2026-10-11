import { expect, test } from "bun:test";
import type { PickerEntry } from "../src/app/store";
import { WorkspaceStore } from "../src/app/workspaceStore";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const entry: PickerEntry = {
  kind: "review",
  provider: "github",
  workspace: "owner",
  repo: "repo",
  prId: 7,
  runId: "run",
  title: "PR",
  status: "completed",
};
const key = "prompt.github:owner/repo";
function fixture() {
  const transport = new FakeBackendTransport({
    workspaceRecords: {
      [key]: { kind: "prompt", value: "Imported prompt ✓" },
      "viewed.github:owner/repo/7": { kind: "viewed", value: ["old.ts"] },
    },
  });
  return { client: new BackendClient(transport), store: new WorkspaceStore() };
}
test("imported prompt/viewed restored; explicit save, discard and viewed mutation persist", async () => {
  const { client, store } = fixture();
  await store.load(client, entry);
  expect(store.getSnapshot().text).toBe("Imported prompt ✓");
  expect(store.getSnapshot().viewed).toEqual(["old.ts"]);
  store.append("\nEdited");
  expect((await client.readWorkspace(key)).payload.value).toBe("Imported prompt ✓");
  store.discard();
  store.clear();
  store.append("New prompt");
  await store.save(client);
  expect((await client.readWorkspace(key)).payload.value).toBe("New prompt");
  await store.toggleViewed(client, "new.ts");
  await store.load(client, entry);
  expect(store.getSnapshot().viewed).toEqual(["old.ts", "new.ts"]);
  await store.toggleViewed(client, "old.ts");
  expect(store.getSnapshot().viewed).toEqual(["new.ts"]);
});
test("stale writes retain prompt edits and never retry or overwrite the other writer", async () => {
  const { client, store } = fixture();
  await store.load(client, entry);
  const page = await client.readWorkspace(key);
  await client.mutateWorkspace(key, page.recordVersion, {
    kind: "setPrompt",
    value: "Other writer",
  });
  store.clear();
  store.append("My unsaved edit");
  await store.save(client);
  expect(store.getSnapshot().error).toContain("changed");
  expect(store.getSnapshot().text).toBe("My unsaved edit");
  expect((await client.readWorkspace(key)).payload.value).toBe("Other writer");
});
test("UTF-8 paste is bounded and control input is rejected atomically", async () => {
  const { client, store } = fixture();
  await store.load(client, entry);
  store.clear();
  store.append("界".repeat(22000));
  expect(store.getSnapshot().text).toBe("");
  store.append("safe\u0000bad");
  expect(store.getSnapshot().text).toBe("");
  store.append("line\n\t✓");
  expect(store.getSnapshot().text).toBe("line\n\t✓");
});
test("late reads after dispose are fenced and same-slug other providers stay isolated", async () => {
  const transport = new FakeBackendTransport({
    delayMs: 15,
    workspaceRecords: { [key]: { kind: "prompt", value: "Github only" } },
  });
  const client = new BackendClient(transport);
  const store = new WorkspaceStore();
  const pending = store.load(client, entry);
  store.dispose();
  await pending;
  expect(store.getSnapshot().scope).toBeNull();
  expect(store.getSnapshot().text).toBe("");
  await store.load(client, { ...entry, provider: "bitbucket", prId: null });
  expect(store.getSnapshot().text).toBe("");
  expect(store.getSnapshot().viewedKey).toBeNull();
});
test("paged reads assemble large viewed scopes; inconsistent versions and no-progress pages fail", async () => {
  const paths = Array.from({ length: 301 }, (_, index) => `path-${index}.ts`);
  const client = new BackendClient(
    new FakeBackendTransport({
      workspaceRecords: { "viewed.github:owner/repo/7": { kind: "viewed", value: paths } },
    }),
  );
  expect((await client.readWorkspace("viewed.github:owner/repo/7")).payload.value).toEqual(paths);
  class Broken extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      const page = await super.request(method, params);
      if (method === "workspace.read") return { ...page, nextOffset: 0 };
      return page;
    }
  }
  await expect(new BackendClient(new Broken()).readWorkspace(key)).rejects.toThrow("scalar");
});

test("late save acknowledgements cannot populate a newly selected repository", async () => {
  const client = new BackendClient(new FakeBackendTransport({ delayMs: 10 }));
  const store = new WorkspaceStore();
  await store.load(client, entry);
  store.append("Old repository edit");
  const save = store.save(client);
  await store.load(client, { ...entry, repo: "other" });
  await save;
  expect(store.getSnapshot().scope).toBe("github:owner/other");
  expect(store.getSnapshot().text).toBe("");
  expect(store.getSnapshot().notice).toBeNull();
  expect((await client.readWorkspace(key)).payload.value).toBe("Old repository edit");
});
