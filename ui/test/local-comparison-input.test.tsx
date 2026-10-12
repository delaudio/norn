import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { LocalTransport } from "./fixtures/local-backend";

for (const width of [120, 50])
  test(`native local comparison controls ignore pasted actions at ${width}`, async () => {
    const transport = new LocalTransport();
    const client = new BackendClient(transport);
    const store = new ShellStore();
    await store.connect(client);
    expect(store.getSnapshot().localInfo).not.toBeNull();
    expect(transport.calls).not.toContain("review.files");
    const renderer = await createTestRenderer({ width, height: width === 50 ? 18 : 30 });
    const app = mountApp(renderer.renderer, store, client);
    const render = async () => {
      for (let i = 0; i < 12; i++) await Bun.sleep(0);
      await renderer.renderOnce();
    };
    try {
      renderer.mockInput.pressKey(KeyCodes.F4);
      await render();
      expect(renderer.captureCharFrame()).toContain("LOCAL COMPARISON");
      expect(renderer.captureCharFrame()).toContain("Current repository");
      await renderer.mockInput.pasteBracketedText("q\nB\nx\n\r");
      await render();
      expect(store.getSnapshot().localForm?.baseIndex).toBe(0);
      expect(store.getSnapshot().localForm?.layers).toEqual(["committed", "staged", "unstaged"]);
      renderer.mockInput.pressEscape();
      await Bun.sleep(30);
      await render();
      expect(store.getSnapshot().localForm).toBeNull();
      expect(transport.calls).not.toContain("local.review.start");
    } finally {
      app.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });

test("cancelled and foreign-root captures cannot replace the selected local snapshot", async () => {
  const transport = new LocalTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  const original = store.getSnapshot().localInfo;
  const request = transport.request.bind(transport);
  try {
    await store.openLocalComparison(client);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    transport.request = async (method, params) => {
      if (method === "local.snapshot") await gate;
      return request(method, params);
    };
    const pending = store.applyLocalComparison(client);
    store.dismissLocalComparison();
    release();
    await pending;
    expect(store.getSnapshot().localInfo).toEqual(original);
    expect(store.getSnapshot().localForm).toBeNull();
    expect(transport.calls).toContain("local.snapshot.cancel");
    transport.request = async (method, params) => {
      const result = await request(method, params);
      if (method !== "local.snapshot") return result;
      const info = structuredClone(result.info) as Record<string, unknown>;
      info.snapshot = { ...(info.snapshot as Record<string, unknown>), rootId: "a".repeat(64) };
      return { ...result, info };
    };
    await store.openLocalComparison(client);
    await store.applyLocalComparison(client);
    expect(store.getSnapshot().localInfo).toEqual(original);
    expect(store.getSnapshot().localForm?.error).toContain("source changed");
    expect(transport.calls).not.toContain("local.review.start");
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("same-path captured layers require explicit finding choice and never call legacy diff", async () => {
  const transport = new LocalTransport();
  let runId = "owned-run";
  let newCapture = false;
  const originalRequest = transport.request.bind(transport);
  const patches = {
    staged: "--- a/same.txt\n+++ b/same.txt\n@@ -1 +1 @@\n-old\n+staged\n",
    unstaged: "--- a/same.txt\n+++ b/same.txt\n@@ -1 +1 @@\n-staged\n+unstaged\n",
  };
  transport.request = async (method, params) => {
    const result = method === "local.snapshot.diff" ? {} : await originalRequest(method, params);
    if (method === "local.snapshot") {
      const info = structuredClone(result.info) as Record<string, unknown>;
      info.layers = [
        { kind: "committed", fileCount: 0, patchBytes: 0 },
        ...(["staged", "unstaged"] as const).map((layer) => ({
          kind: layer,
          fileCount: 1,
          patchBytes: Buffer.byteLength(patches[layer]),
        })),
      ];
      info.fileCount = 2;
      info.patchBytes = Buffer.byteLength(patches.staged) + Buffer.byteLength(patches.unstaged);
      if (newCapture) {
        info.snapshot = {
          ...(info.snapshot as Record<string, unknown>),
          snapshotId: "d".repeat(64),
        };
        info.localReviewId = 0xdddddddd;
      }
      return { ...result, info };
    }
    if (method === "local.snapshot.files")
      return {
        ...result,
        total: 2,
        files: ["staged", "unstaged"].map((layer) => ({
          layer,
          path: "same.txt",
          oldPath: null,
          status: "modified",
          additions: 1,
          deletions: 1,
          binary: false,
        })),
      };
    if (method === "local.snapshot.diff") {
      const text = patches[params.layer as keyof typeof patches];
      return {
        request: params,
        text,
        totalBytes: Buffer.byteLength(text),
        contentSha256: new Bun.CryptoHasher("sha256").update(text).digest("hex"),
        nextOffset: null,
      };
    }
    if (method === "local.review.history")
      return {
        ...result,
        runs: [
          { runId, createdAt: "1", status: "succeeded", turnKind: "initial", findingsCount: 1 },
        ],
        total: 1,
        nextOffset: null,
      };
    if (method === "local.review.threads") return { ...result, runId };
    if (method === "local.review.findings")
      return {
        ...result,
        runId,
        total: 1,
        findings: [
          {
            id: "owned-finding",
            title: "Ambiguous layer",
            severity: "high",
            summary: "Choose the actual layer.",
            anchor: { path: "same.txt", startLine: 1, endLine: null, side: "new" },
            candidateLayers: ["staged", "unstaged"],
          },
        ],
      };
    return result;
  };
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    expect(store.getSnapshot().error).toBeNull();
    expect(store.getSnapshot().files.map((file) => file.localLayer)).toEqual([
      "staged",
      "unstaged",
    ]);
    expect(store.getSnapshot().diff?.text).toContain("+staged");
    store.cycleFocus();
    store.cycleFocus();
    expect(store.getSnapshot().findingLayers).toEqual(["staged", "unstaged"]);
    store.moveFindingLayer(1);
    store.chooseFindingLayer();
    for (let i = 0; i < 12; i++) await Bun.sleep(0);
    expect(store.getSnapshot().files[store.getSnapshot().selectedFile]?.localLayer).toBe(
      "unstaged",
    );
    expect(store.getSnapshot().diff?.text).toContain("+unstaged");
    // A status resync must not reopen a chooser that intercepts cancellation.
    await store.refreshOperation();
    expect(store.getSnapshot().findingLayers).toEqual([]);
    expect(store.getSnapshot().files[store.getSnapshot().selectedFile]?.localLayer).toBe(
      "unstaged",
    );
    runId = "new-owned-run";
    await store.openLocalComparison(client);
    await store.applyLocalComparison(client);
    expect(store.getSnapshot().localRunId).toBe(runId);
    expect(store.getSnapshot().findingLayers).toEqual(["staged", "unstaged"]);
    store.chooseFindingLayer();
    newCapture = true;
    await store.openLocalComparison(client);
    await store.applyLocalComparison(client);
    expect(store.getSnapshot().localInfo?.snapshot.snapshotId).toBe("d".repeat(64));
    expect(store.getSnapshot().findingLayers).toEqual(["staged", "unstaged"]);
    expect(transport.calls).not.toContain("diff.file");
    expect(transport.calls).not.toContain("review.findings");
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("qualified local profile selection uses the native catalog and never launches an agent", async () => {
  const transport = new LocalTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    await store.openReviewProfiles(client);
    expect(store.getSnapshot().profileCatalog?.defaultProfile).toBe("security");
    store.moveReviewProfile(2);
    store.chooseReviewProfile();
    expect(store.getSnapshot().selectedReviewProfile).toBe("security");
    expect(transport.calls).toContain("local.review.profiles");
    expect(transport.calls).not.toContain("review.profiles");
    expect(transport.calls).not.toContain("local.review.start");
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("configured providers, base cycling and layer recapture retain exact qualified sources", async () => {
  const transport = new LocalTransport({
    repositories: [
      { provider: "github", workspace: "owned", repo: "same", localPath: "/owned/github" },
      { provider: "bitbucket", workspace: "owned", repo: "same", localPath: "/owned/bitbucket" },
    ],
  });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    const currentRoot = store.getSnapshot().localInfo?.snapshot.rootId;
    await store.openLocalComparison(client);
    await store.moveLocalComparison(2);
    await store.applyLocalComparison(client);
    expect(store.getSnapshot().localForm?.error ?? store.getSnapshot().error).toBeNull();
    expect(store.getSnapshot().localInfo?.snapshot.repository.provider).toBe("bitbucket");
    expect(store.getSnapshot().localInfo?.comparison.source.kind).toBe("configured");
    expect(store.getSnapshot().localInfo?.snapshot.rootId).not.toBe(currentRoot);
    await store.cycleLocalBase(client);
    expect(store.getSnapshot().localInfo?.comparison).toMatchObject({
      mode: "branch",
      baseRef: "comparison",
      layers: ["committed"],
    });
    await store.cycleLocalBase(client);
    expect(store.getSnapshot().localInfo?.comparison).toMatchObject({
      mode: "workingTree",
      baseRef: null,
      layers: ["committed", "staged", "unstaged"],
    });
    await store.openLocalComparison(client);
    store.cycleLocalField();
    store.cycleLocalField();
    store.toggleLocalLayer();
    await store.applyLocalComparison(client);
    expect(store.getSnapshot().localInfo?.comparison.layers).toEqual(["staged", "unstaged"]);
    expect(transport.calls).not.toContain("local.review.start");
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("local thread selection and refresh retain the chosen run in the same immutable snapshot", async () => {
  const transport = new LocalTransport();
  const request = transport.request.bind(transport);
  transport.request = async (method, params) => {
    const result = await request(method, params);
    if (method === "local.review.history")
      return {
        ...result,
        runs: ["new", "old"].map((runId, index) => ({
          runId,
          createdAt: String(2 - index),
          status: "succeeded",
          turnKind: "initial",
          findingsCount: 0,
        })),
        total: 2,
      };
    if (method === "local.review.findings") return { ...result, runId: params.runId };
    if (method === "local.review.threads")
      return {
        ...result,
        runId: params.runId,
        threads: ["new", "old"].map((runId) => ({
          id: `thread-${runId}`,
          title: runId,
          latestRunId: runId,
          selected: runId === params.runId,
          messages: [],
          truncated: false,
        })),
      };
    return result;
  };
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    const snapshot = store.getSnapshot().localInfo?.snapshot;
    expect(store.getSnapshot().localRunId).toBe("new");
    store.toggleThreads();
    store.move(1);
    await store.selectThread();
    expect(store.getSnapshot().localRunId).toBe("old");
    await store.loadSnapshot(client, store.getSnapshot().generation);
    expect(store.getSnapshot().localRunId).toBe("old");
    expect(store.getSnapshot().localInfo?.snapshot).toEqual(snapshot);
    expect(store.getSnapshot().conversationOpen).toBe(true);
    expect(transport.calls).not.toContain("review.threads");
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("qualified local start accepts events before its response and rejects foreign target events", async () => {
  const transport = new LocalTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  const request = transport.request.bind(transport);
  let operationTarget = { kind: "local" as const, localSnapshotSha256: "" };
  transport.request = async (method, params) => {
    if (method === "local.review.start") {
      operationTarget = {
        kind: "local",
        localSnapshotSha256: (params.snapshot as { snapshotId: string }).snapshotId,
      };
      store.applyEvent({
        operationId: "owned-operation",
        sequence: 1,
        target: operationTarget,
        event: { kind: "state", state: "running" },
      });
      return {
        request: params,
        target: operationTarget,
        operationId: "owned-operation",
        state: "accepted",
      };
    }
    if (method === "operation.status")
      return { operationId: "owned-operation", state: "running", sequence: 1, error: null };
    return request(method, params);
  };
  try {
    await store.connect(client);
    await store.startReview(client);
    expect(store.getSnapshot().operation?.state).toBe("running");
    expect(store.getSnapshot().operation?.sequence).toBe(1);
    store.applyEvent({
      operationId: "owned-operation",
      sequence: 2,
      target: { kind: "local", localSnapshotSha256: "a".repeat(64) },
      event: { kind: "state", state: "succeeded" },
    });
    expect(store.getSnapshot().operation?.state).toBe("running");
    store.applyEvent({
      operationId: "owned-operation",
      sequence: 2,
      target: operationTarget,
      event: { kind: "progress", message: "Owned progress" },
    });
    expect(store.getSnapshot().operation?.sequence).toBe(2);
    expect(transport.calls).not.toContain("review.start");
  } finally {
    store.cancelSyntax();
    client.close();
  }
});
