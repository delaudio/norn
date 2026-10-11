import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("native staging preview/cancel/paste never saves; explicit stage persists and opens the scoped draft list", async () => {
  const calls: string[] = [];
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      calls.push(method);
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
          runId: "run-1",
          status: "completed",
        },
      ],
      findings: [
        {
          id: "f1",
          title: "Validate input",
          severity: "high",
          summary: "Windows content 世界\r\nSecond line.",
          anchor: { path: "src/test.ts", startLine: 12, endLine: null, side: "old" },
        },
      ],
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
    renderer.mockInput.pressKey("i");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Stage finding");
    await renderer.mockInput.pasteBracketedText("y\nd\nq");
    await settle();
    expect(calls.filter((method) => method === "draft.stage")).toHaveLength(0);
    renderer.mockInput.pressEnter();
    await settle();
    expect(renderer.captureCharFrame()).toContain("Windows content 世界");
    expect(renderer.captureCharFrame()).toContain("Second line.");
    expect(renderer.captureCharFrame()).toContain("old-side 12");
    expect(calls.filter((method) => method === "workspace.mutate")).toHaveLength(0);
    const reads = calls.filter((method) => method === "draft.prepare").length;
    await store.applySettings(client, (await client.settings()).settings);
    await settle();
    expect(calls.filter((method) => method === "draft.prepare")).toHaveLength(reads);
    expect(renderer.captureCharFrame()).toContain("Windows content 世界");
    renderer.mockInput.pressKey("n");
    await settle();
    renderer.mockInput.pressKey("y");
    await settle();
    expect(calls.filter((method) => method === "draft.stage")).toHaveLength(0);
    renderer.mockInput.pressEnter();
    await settle();
    renderer.mockInput.pressKey("y");
    await settle();
    expect(calls.filter((method) => method === "draft.stage")).toHaveLength(1);
    expect(renderer.captureCharFrame()).toContain("Draft staging confirmed locally");
    renderer.mockInput.pressKey("y");
    await settle();
    expect(calls.filter((method) => method === "draft.stage")).toHaveLength(1);
    renderer.mockInput.pressKey("d");
    await settle();
    expect(renderer.captureCharFrame()).toContain("1 staged comments");
    expect(renderer.captureCharFrame()).toContain("Body: ### Validate input");
    expect(renderer.captureCharFrame()).toContain("Second line.");
    const saved = await client.readWorkspace("drafts.github:owner/repo/7");
    expect(saved.payload.value).toMatchObject([
      { raw: "### Validate input\n\nWindows content 世界\r\nSecond line." },
    ]);
    expect(
      calls.some(
        (method) =>
          method.startsWith("auth.") || method.includes("publish") || method === "review.start",
      ),
    ).toBe(false);
  } finally {
    ui.unmount();
    client.close();
    renderer.renderer.destroy();
  }
});
