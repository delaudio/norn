import { expect, test } from "bun:test";
import type { WorkspaceScope } from "../../src/protocol/generated/protocol";
import { buildPicker, ShellStore } from "../src/app/store";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const scope = (provider: WorkspaceScope["provider"], prId = 7): WorkspaceScope => ({
  provider,
  workspace: "imported",
  repo: "repo",
  prId,
  draftCount: 0,
  referenceCount: 0,
  viewedCount: 1,
  localOnly: prId >= 0x80000000,
});

test("unconfigured canonical scopes are reachable without invented runs and providers remain distinct", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      workspaceRecords: {
        "viewed.github:imported/repo/7": { kind: "viewed", value: ["github.rs"] },
        "viewed.bitbucket:imported/repo/7": { kind: "viewed", value: ["bitbucket.rs"] },
        "drafts.github:empty/repo/8": { kind: "drafts", value: [] },
        "prompt.github:other/repo": { kind: "prompt", value: "private prompt" },
      },
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  expect(store.getSnapshot().picker).toHaveLength(2);
  expect(store.getSnapshot().picker.map((entry) => entry.runId)).toEqual([null, null]);
  await store.refreshViewed(client, store.getSnapshot().generation);
  expect(store.getSnapshot().viewedFiles).toEqual(["github.rs"]);
  await store.selectPicker(1);
  await store.refreshViewed(client, store.getSnapshot().generation);
  expect(store.getSnapshot().viewedFiles).toEqual(["bitbucket.rs"]);
  store.beginReply();
  expect(store.getSnapshot().composing).toBe(false);
  expect(store.getSnapshot().notice).toContain("saved review");
  await store.applySettings(client, (await client.settings()).settings);
  expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]?.provider).toBe(
    "bitbucket",
  );
  await store.refreshViewed(client, store.getSnapshot().generation);
  expect(store.getSnapshot().viewedFiles).toEqual(["bitbucket.rs"]);
  client.close();
});

test("inventory deduplicates a saved-run PR without replacing the actual run", () => {
  const picker = buildPicker(
    [],
    [
      {
        provider: "github",
        workspace: "imported",
        repo: "repo",
        prId: 7,
        runId: "actual-run",
        title: "Review",
        status: "succeeded",
      },
    ],
    [scope("github"), scope("bitbucket")],
  );
  expect(picker).toHaveLength(2);
  expect(picker[0]?.runId).toBe("actual-run");
  expect(picker[1]?.provider).toBe("bitbucket");
  expect(picker[1]?.runId).toBeNull();
});

test("inventory follows immutable pages within the bounded total", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      scopes: Array.from({ length: 201 }, (_, index) => scope("github", index + 1)),
    }),
  );
  const scopes = await client.workspaceScopes();
  expect(scopes).toHaveLength(201);
  expect(scopes[200]?.prId).toBe(201);
});

test("duplicate scopes, inconsistent pages and malformed metadata are rejected", async () => {
  for (const failure of ["duplicate", "version", "cursor", "local", "unknown"] as const) {
    class Broken extends FakeBackendTransport {
      async request(method: string, params: Record<string, unknown>) {
        const result = await super.request(method, params);
        if (method !== "workspace.scopes") return result;
        if (failure === "duplicate") result.scopes = [scope("github"), scope("github")];
        if (failure === "version" && params.offset) result.inventoryVersion = "f".repeat(64);
        if (failure === "cursor") result.nextOffset = 0;
        if (failure === "local") result.scopes = [{ ...scope("github"), localOnly: true }];
        if (failure === "unknown") result.privateContent = "must reject";
        return result;
      }
    }
    const client = new BackendClient(
      new Broken({ scopes: Array.from({ length: 101 }, (_, index) => scope("github", index + 1)) }),
    );
    await expect(client.workspaceScopes()).rejects.toThrow();
  }
});

test("late inventory pages stop when their selection generation changes", async () => {
  let current = true;
  let calls = 0;
  class Late extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      const result = await super.request(method, params);
      if (method === "workspace.scopes") {
        calls++;
        current = false;
      }
      return result;
    }
  }
  const client = new BackendClient(
    new Late({ scopes: Array.from({ length: 101 }, (_, index) => scope("github", index + 1)) }),
  );
  await expect(client.workspaceScopes(() => current)).rejects.toThrow("selection changed");
  expect(calls).toBe(1);
});

test("legacy local scopes remain accessible but cannot start provider reviews", async () => {
  const store = new ShellStore();
  const client = new BackendClient(
    new FakeBackendTransport({ scopes: [scope("github", 0x80000001)] }),
  );
  await store.connect(client);
  await store.startReview(client);
  expect(store.getSnapshot().operation).toBeNull();
  expect(store.getSnapshot().notice).toContain("cannot start a provider review");
  client.close();
});
