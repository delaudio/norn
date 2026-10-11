import { expect, test } from "bun:test";
import { CliRenderEvents } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("native references restore imported content, fence pasted shortcuts and require save/removal approval", async () => {
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
        "references.github:owner/repo/7": {
          kind: "references",
          value: [
            {
              id: "note-1",
              type: "note",
              source: "manual",
              title: "Imported note",
              body: "Saved Unicode 世界",
              createdAt: 1,
              updatedAt: 2,
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
    renderer.mockInput.pressKey("w");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Saved Unicode 世界");
    const frameListeners = renderer.renderer.listenerCount(CliRenderEvents.FRAME);
    renderer.mockInput.pressKey("e");
    await settle();
    expect(renderer.renderer.listenerCount(CliRenderEvents.FRAME)).toBe(frameListeners + 1);
    for (let i = 0; i < 8; i++) renderer.mockInput.pressTab();
    renderer.mockInput.pressEnter();
    await settle();
    renderer.mockInput.pressKey("u", { ctrl: true });
    await renderer.mockInput.pasteBracketedText("q\nn\nreview.start\n世界");
    await settle();
    expect(calls.filter((call) => call.method === "workspace.mutate")).toHaveLength(0);
    expect(store.getSnapshot().referencesOpen).toBe(true);
    renderer.mockInput.pressEnter();
    await settle();
    const referenceReads = calls.filter(
      (call) =>
        call.method === "workspace.read" && call.params.key === "references.github:owner/repo/7",
    ).length;
    await store.applySettings(client, (await client.settings()).settings);
    await settle();
    expect(
      calls.filter(
        (call) =>
          call.method === "workspace.read" && call.params.key === "references.github:owner/repo/7",
      ),
    ).toHaveLength(referenceReads);
    renderer.mockInput.pressKey("s", { ctrl: true });
    await settle();
    const saved = calls.find((call) => call.method === "workspace.mutate");
    expect(saved?.params).toMatchObject({
      key: "references.github:owner/repo/7",
      mutation: {
        kind: "putReference",
        reference: {
          id: "note-1",
          body: "q\nn\nreview.start\n世界",
          createdAt: 1,
          source: "manual",
        },
      },
    });
    expect(renderer.captureCharFrame()).toContain("Reference saved");
    expect(renderer.renderer.listenerCount(CliRenderEvents.FRAME)).toBe(frameListeners);
    renderer.mockInput.pressKey("d");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Remove Imported note");
    renderer.mockInput.pressKey("n");
    await settle();
    expect(calls.filter((call) => call.method === "workspace.mutate")).toHaveLength(1);
    renderer.mockInput.pressKey("d");
    await settle();
    renderer.mockInput.pressKey("y");
    await settle();
    expect(calls.filter((call) => call.method === "workspace.mutate")).toHaveLength(2);
    expect(renderer.captureCharFrame()).toContain("0 saved references");
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

test("narrow reference editor follows the selected wrapped field after native layout", async () => {
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
        "references.github:owner/repo/7": {
          kind: "references",
          value: [
            {
              id: "long-url",
              type: "note",
              source: "manual",
              title: "Synthetic",
              url: `https://example.invalid/${"x".repeat(2000)}`,
              body: "OFFSCREEN_CONTEXT_MARKER",
              createdAt: 1,
              updatedAt: 2,
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
    renderer.mockInput.pressKey("w");
    await settle();
    renderer.mockInput.pressKey("e");
    await settle();
    for (let i = 0; i < 8; i++) renderer.mockInput.pressTab();
    await settle();
    expect(renderer.captureCharFrame()).toContain("OFFSCREEN_CONTEXT_MARKER");
    const field = renderer.renderer.root.findDescendantById("reference-field-8");
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
