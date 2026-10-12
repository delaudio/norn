import { expect, test } from "bun:test";
import { ExplorerStore, explorerLabel, explorerText, treeRows } from "../src/app/explorerStore";
import { BackendClient } from "../src/transport/backendClient";
import { ExplorerFixture } from "./fixtures/explorer-backend";

const repository = { provider: "github", workspace: "owned", repo: "same" } as const;

test("native explorer retains tree/status/filter and bounded content/diff/blame meaning", async () => {
  const fixture = new ExplorerFixture(),
    client = new BackendClient(fixture),
    store = new ExplorerStore();
  await store.load(client, repository);
  expect(store.getSnapshot().context?.rootId).toBe("a".repeat(64));
  expect(
    treeRows(store.getSnapshot())
      .filter((row) => row.kind === "folder")
      .map((row) => row.path),
  ).toEqual(["images", "src"]);
  await store.choose();
  expect(store.getSnapshot().preview?.text).toContain("# Owned explorer");
  store.toggleMarkdown();
  expect(store.getSnapshot().markdown).toBe(true);
  await store.setMode("diff");
  expect(store.getSnapshot().preview?.text).toContain("+owned change");
  await store.setMode("blame");
  expect(store.getSnapshot().blame?.lines[0]?.message).toContain("Retained final paragraph");
  store.openInput("filter");
  store.appendInput("src/");
  await store.applyInput();
  expect(store.getSnapshot().files.map((file) => file.status)).toEqual([
    "modified",
    "conflicted",
    "renamed",
  ]);
  expect(store.getSnapshot().files.at(-1)?.oldPath).toBe("src/old.ts");
  await store.choose();
  expect(treeRows(store.getSnapshot())).toHaveLength(1);
  await store.choose();
  expect(treeRows(store.getSnapshot())).toHaveLength(4);
  await store.toggleChanged();
  expect(store.getSnapshot().files.every((file) => file.status !== "unchanged")).toBe(true);
  store.dispose();
  client.close();
});
test("find jumps directly into a pinned bounded window without loading the full selected document", async () => {
  const fixture = new ExplorerFixture(),
    client = new BackendClient(fixture),
    store = new ExplorerStore();
  await store.load(client, repository);
  const rows = treeRows(store.getSnapshot());
  store.move(rows.findIndex((row) => row.path === "src/--modified ü.ts"));
  await store.choose();
  expect(store.getSnapshot().preview?.text).not.toContain("TARGET_AFTER_FIRST_WINDOW");
  store.openInput("find");
  store.appendInput("target_after_first_window");
  await store.applyInput();
  expect(store.getSnapshot().preview?.offset).toBeGreaterThan(32768);
  expect(store.getSnapshot().preview?.text).toStartWith("TARGET_AFTER_FIRST_WINDOW");
  expect(store.getSnapshot().search?.total).toBe(1);
  const reads = fixture.calls.filter((call) => call.method === "repository.explore.content");
  expect(reads).toHaveLength(2);
  expect(reads[1]?.params.contentSha256).toBe(store.getSnapshot().preview?.sha);
  expect(Buffer.byteLength(store.getSnapshot().preview!.text)).toBeLessThanOrEqual(32768);
  store.openInput("find");
  store.clearInput();
  await store.applyInput();
  expect(store.getSnapshot().search).toBeNull();
  await store.firstPreview();
  const firstOffset = store.getSnapshot().preview!.offset;
  await store.pageViewport(1, 10);
  expect(store.getSnapshot().previewRow).toBe(10);
  expect(store.getSnapshot().preview!.offset).toBe(firstOffset);
  store.moveHorizontal(12);
  expect(store.getSnapshot().previewColumn).toBe(12);
  await store.setMode("diff");
  store.openInput("find");
  store.appendInput("owned change");
  await store.applyInput();
  expect(store.getSnapshot().search?.request.source).toBe("diff");
  expect(store.getSnapshot().preview?.text).toStartWith("owned change");
  store.dispose();
  client.close();
});
test("tree pages preserve inventory identity and stale failures clear navigation metadata", async () => {
  const fixture = new ExplorerFixture(),
    client = new BackendClient(fixture),
    store = new ExplorerStore();
  fixture.files = Array.from({ length: 70 }, (_, index) => ({
    path: `file-${String(index).padStart(3, "0")}.txt`,
    oldPath: null,
    status: "unchanged",
  }));
  await store.load(client, repository);
  expect(store.getSnapshot().files).toHaveLength(32);
  const version = store.getSnapshot().version;
  await store.pageFiles(1);
  expect(store.getSnapshot().offset).toBe(32);
  expect(fixture.calls.at(-1)?.params.version).toBe(version);
  await store.pageFiles(1);
  expect(store.getSnapshot().files).toHaveLength(6);
  const calls = fixture.calls.length;
  await store.pageFiles(1);
  expect(fixture.calls).toHaveLength(calls);
  fixture.files.pop();
  await store.pageFiles(-1);
  expect(store.getSnapshot().error).toContain("Inventory changed");
  expect(store.getSnapshot().total).toBe(0);
  expect(store.getSnapshot().next).toBeNull();
  await store.refresh();
  expect(store.getSnapshot().total).toBe(69);
  store.dispose();
  client.close();
});
test("blame details have independent keyboard focus and binary/deleted states remain explicit", async () => {
  const fixture = new ExplorerFixture(),
    client = new BackendClient(fixture),
    store = new ExplorerStore();
  await store.load(client, repository);
  await store.choose();
  await store.setMode("blame");
  store.cycleFocus();
  expect(store.getSnapshot().focus).toBe("details");
  store.cycleFocus();
  expect(store.getSnapshot().focus).toBe("tree");
  store.move(1);
  await store.choose();
  expect(store.getSnapshot().preview?.state).toBe("deleted");
  store.cycleFocus();
  store.move(2);
  await store.choose();
  expect(store.getSnapshot().preview?.state).toBe("binary");
  store.openInput("find");
  expect(store.getSnapshot().input).toBeNull();
  store.dispose();
  client.close();
});
test("late source and file results never apply after provider switch or disposal", async () => {
  const fixture = new ExplorerFixture(),
    client = new BackendClient(fixture),
    store = new ExplorerStore();
  let release!: () => void;
  fixture.nextGate = {
    method: "repository.explore.info",
    wait: new Promise<void>((resolve) => {
      release = resolve;
    }),
  };
  const late = store.load(client, repository);
  await store.load(client, { ...repository, provider: "bitbucket" });
  release();
  await late;
  expect(store.getSnapshot().context?.repository.provider).toBe("bitbucket");
  expect(store.getSnapshot().context?.rootId).toBe("b".repeat(64));
  fixture.nextGate = {
    method: "repository.explore.content",
    wait: new Promise<void>((resolve) => {
      release = resolve;
    }),
  };
  const content = store.choose();
  store.dispose();
  release();
  await content;
  expect(store.getSnapshot().preview).toBeNull();
  client.close();
});
test("unavailable backend and malformed echo remain explicit; display escapes never change request paths", async () => {
  const fixture = new ExplorerFixture(),
    client = new BackendClient(fixture),
    store = new ExplorerStore();
  await store.load(client, repository, false);
  expect(store.getSnapshot().error).toContain("matching Norn");
  expect(fixture.calls).toHaveLength(0);
  fixture.alter = (result) => {
    if (result.context) (result.context as { rootId: string }).rootId = "wrong";
  };
  await store.load(client, repository);
  expect(store.getSnapshot().error).toContain("Invalid or mismatched");
  expect(store.getSnapshot().context).toBeNull();
  expect(explorerText("owned\u001b[31m\u0007")).toBe("owned\\u001b[31m\\u0007");
  expect(explorerLabel("legitimate\nfilename")).toBe("legitimate\\nfilename");
  store.dispose();
  client.close();
});
