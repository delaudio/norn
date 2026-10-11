import { expect, test } from "bun:test";
import { DiffScrollPositions } from "../src/app/diffScrollPositions";
import { ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const diff =
  "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,2 +1,2 @@\n-old needle\n+new NEEDLE\n context";
async function fixture() {
  const store = new ShellStore();
  await store.connect(
    new BackendClient(
      new FakeBackendTransport({
        repositories: [{ provider: "github", workspace: "acme", repo: "repo", localPath: null }],
        diffText: diff,
        files: [{ path: "a.ts", status: "modified", additions: 1, deletions: 1, oldPath: null }],
      }),
    ),
  );
  for (let attempt = 0; attempt < 100 && store.getSnapshot().diffLoading; attempt++)
    await Bun.sleep(1);
  expect(store.getSnapshot().diff?.text).toBe(diff);
  return store;
}
test("search applies explicitly, wraps source rows and cancelling keeps the existing search", async () => {
  const store = await fixture();
  store.beginSearch();
  store.appendSearch("needle");
  expect(store.getSnapshot().searchMatches).toEqual([]);
  store.applySearch();
  expect(store.getSnapshot().searchMatches).toEqual([4, 5]);
  expect(store.getSnapshot().searchRow).toBe(4);
  store.nextSearch(-1);
  expect(store.getSnapshot().searchRow).toBe(5);
  store.nextSearch(1);
  expect(store.getSnapshot().searchRow).toBe(4);
  store.beginSearch();
  store.appendSearch(" edit");
  store.cancelSearch();
  expect(store.getSnapshot().searchQuery).toBe("needle");
  expect(store.getSnapshot().searchRow).toBe(4);
  store.beginSearch();
  store.clearSearchText();
  store.appendSearch("absent");
  store.applySearch();
  expect(store.getSnapshot().searchMatches).toEqual([]);
  expect(store.getSnapshot().notice).toContain("No matching");
  store.beginSearch();
  store.clearSearchText();
  store.applySearch();
  expect(store.getSnapshot().searchQuery).toBe("");
});
test("search paste bounds are atomic and changing target clears all match identities", async () => {
  const store = await fixture();
  store.beginSearch();
  store.appendSearch("界".repeat(200));
  expect(store.getSnapshot().searchText).toBe("");
  store.appendSearch("needle\nq");
  expect(store.getSnapshot().searchText).toBe("");
  store.appendSearch("needle");
  store.applySearch();
  await store.selectPicker(0);
  expect(store.getSnapshot().searchRow).toBeNull();
  expect(store.getSnapshot().searchMatches).toEqual([]);
});
test("scroll positions are bounded, isolated by identity and reject invalid coordinates", () => {
  const positions = new DiffScrollPositions();
  positions.set("file:unified:hash", { top: 123, left: 12 });
  positions.set("file:split:hash", { top: 456, left: 0 });
  expect(positions.get("file:unified:hash")).toEqual({ top: 123, left: 12 });
  expect(positions.get("other-file:unified:hash")).toBeUndefined();
  expect(positions.get("file:unified:changed-hash")).toBeUndefined();
  positions.set("bad", { top: NaN, left: 0 });
  expect(positions.get("bad")).toBeUndefined();
  for (let index = 0; index < 32; index++) positions.set(`file-${index}`, { top: index, left: 0 });
  expect(positions.get("file:unified:hash")).toBeUndefined();
  positions.clear();
  expect(positions.get("file-31")).toBeUndefined();
});
