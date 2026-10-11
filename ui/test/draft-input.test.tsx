import { expect, test } from "bun:test";
import { CliRenderEvents } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("native drafts restore imported content, fence pasted shortcuts and require save/removal approval", async () => {
  const calls: Array<{ method: string; params: Record<string, unknown> }> = [];
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      calls.push({ method, params });
      return super.request(method, params);
    }
  }
  const client = new BackendClient(
    new Transport({
      repositories: [{ provider: "github", workspace: "owner", repo: "repo", localPath: null }],
      targets: [
        {
          provider: "github",
          workspace: "owner",
          repo: "repo",
          prId: 7,
          title: "PR",
          latestRunId: "run-1",
          status: "completed",
        },
      ],
      workspaceRecords: {
        "drafts.github:owner/repo/7": {
          kind: "drafts",
          value: [
            {
              localId: "draft-1",
              prId: 7,
              path: "src/imported.ts",
              raw: "Saved Unicode 世界",
              createdAt: 1,
              source: "manual",
              publicationMode: "file",
            },
          ],
        },
      },
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(1);
  const renderer = await createTestRenderer({ width: 120, height: 40 });
  const ui = mountApp(renderer.renderer, store, client);
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    renderer.mockInput.pressKey("d");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Saved Unicode 世界");
    const frameListeners = renderer.renderer.listenerCount(CliRenderEvents.FRAME);
    renderer.mockInput.pressKey("e");
    await settle();
    expect(renderer.renderer.listenerCount(CliRenderEvents.FRAME)).toBe(frameListeners + 1);
    for (let i = 0; i < 5; i++) renderer.mockInput.pressTab();
    renderer.mockInput.pressEnter();
    await settle();
    renderer.mockInput.pressKey("u", { ctrl: true });
    await renderer.mockInput.pasteBracketedText("q\nn\nreview.start\n世界");
    await settle();
    expect(calls.filter((call) => call.method === "workspace.mutate")).toHaveLength(0);
    expect(store.getSnapshot().draftsOpen).toBe(true);
    renderer.mockInput.pressEnter();
    await settle();
    const draftReads = calls.filter(
      (call) =>
        call.method === "workspace.read" && call.params.key === "drafts.github:owner/repo/7",
    ).length;
    await store.applySettings(client, (await client.settings()).settings);
    await settle();
    expect(
      calls.filter(
        (call) =>
          call.method === "workspace.read" && call.params.key === "drafts.github:owner/repo/7",
      ),
    ).toHaveLength(draftReads);
    renderer.mockInput.pressKey("s", { ctrl: true });
    await settle();
    const saved = calls.find((call) => call.method === "workspace.mutate");
    expect(saved?.params).toMatchObject({
      key: "drafts.github:owner/repo/7",
      mutation: {
        kind: "putDraft",
        draft: {
          localId: "draft-1",
          raw: "q\nn\nreview.start\n世界",
          createdAt: 1,
          source: "manual",
        },
      },
    });
    expect(renderer.captureCharFrame()).toContain("Draft saved");
    expect(renderer.renderer.listenerCount(CliRenderEvents.FRAME)).toBe(frameListeners);
    renderer.mockInput.pressKey("d");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Remove src/imported.ts");
    renderer.mockInput.pressKey("n");
    await settle();
    expect(calls.filter((call) => call.method === "workspace.mutate")).toHaveLength(1);
    renderer.mockInput.pressKey("d");
    await settle();
    renderer.mockInput.pressKey("y");
    await settle();
    expect(calls.filter((call) => call.method === "workspace.mutate")).toHaveLength(2);
    expect(renderer.captureCharFrame()).toContain("0 staged comments");
    expect(
      calls.some(
        (call) =>
          call.method === "review.start" ||
          call.method.startsWith("auth.") ||
          call.method === "browser.open",
      ),
    ).toBe(false);
  } finally {
    ui.unmount();
    client.close();
    renderer.renderer.destroy();
  }
});

test("narrow draft editor follows the selected wrapped field after native layout", async () => {
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories: [{ provider: "github", workspace: "owner", repo: "repo", localPath: null }],
      targets: [
        {
          provider: "github",
          workspace: "owner",
          repo: "repo",
          prId: 7,
          title: "PR",
          latestRunId: "run-1",
          status: "completed",
        },
      ],
      workspaceRecords: {
        "drafts.github:owner/repo/7": {
          kind: "drafts",
          value: [
            {
              localId: "long-path",
              prId: 7,
              source: "manual",
              path: `src/${"x".repeat(2000)}`,
              raw: "OFFSCREEN_CONTEXT_MARKER",
              createdAt: 1,
              publicationMode: "file",
            },
          ],
        },
      },
    }),
  );
  const shell = new ShellStore();
  await shell.connect(client);
  await shell.selectPicker(1);
  const renderer = await createTestRenderer({ width: 50, height: 20 });
  const ui = mountApp(renderer.renderer, shell, client);
  const settle = async () => {
    for (let i = 0; i < 6; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    renderer.mockInput.pressKey("d");
    await settle();
    renderer.mockInput.pressKey("e");
    await settle();
    for (let i = 0; i < 5; i++) renderer.mockInput.pressTab();
    await settle();
    expect(renderer.captureCharFrame()).toContain("OFFSCREEN_CONTEXT_MARKER");
    const field = renderer.renderer.root.findDescendantById("draft-field-5");
    expect(field!.y).toBeGreaterThanOrEqual(0);
    expect(field!.y).toBeLessThan(18);
    renderer.resize(80, 24);
    await settle();
    expect(renderer.captureCharFrame()).toContain("OFFSCREEN_CONTEXT_MARKER");
  } finally {
    ui.unmount();
    client.close();
    renderer.renderer.destroy();
  }
});
