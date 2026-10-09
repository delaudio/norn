import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const repositories = [
  { provider: "github" as const, workspace: "acme", repo: "payments", localPath: null },
  { provider: "github" as const, workspace: "acme", repo: "billing", localPath: "/tmp/billing" },
];

async function mount(width: number, height: number) {
  const renderer = await createTestRenderer({ width, height });
  const client = new BackendClient(new FakeBackendTransport({ repositories }));
  const store = new ShellStore();
  await store.connect(client);
  let quits = 0;
  const ui = mountApp(renderer.renderer, store, client, () => {
    quits += 1;
  });
  const unsubscribe = client.onOperationEvent((event) => store.applyEvent(event));
  await renderer.renderOnce();
  return {
    renderer,
    store,
    ui,
    quits: () => quits,
    cleanup: () => unsubscribe(),
  };
}

for (const [width, height] of [
  [50, 15],
  [100, 30],
] as const) {
  test(`renders the workspace at ${width}x${height}`, async () => {
    const app = await mount(width, height);
    try {
      const frame = app.renderer.captureCharFrame();
      expect(frame).toContain("Norn / OpenTUI workspace");
      expect(frame).toContain("acme/payments");
      if (width >= 80) {
        expect(frame).toContain("TARGETS");
      }
      app.renderer.mockInput.pressKey("j");
      await app.renderer.renderOnce();
      expect(app.store.current?.repo).toBe("billing");
      app.renderer.mockInput.pressKey("k");
      await app.renderer.renderOnce();
      expect(app.store.current?.repo).toBe("payments");
    } finally {
      app.cleanup();
      app.ui.unmount();
      app.renderer.renderer.destroy();
    }
  });
}

test("Enter starts a review, logs stream, q quits and unmount clears", async () => {
  const app = await mount(100, 30);
  try {
    app.renderer.mockInput.pressEnter();
    await new Promise((resolve) => setTimeout(resolve, 10));
    await app.renderer.renderOnce();
    expect(app.store.getSnapshot().operation?.state).toBe("succeeded");
    expect(app.renderer.captureCharFrame()).toContain("review fake-op-");

    app.renderer.mockInput.pressKey("q");
    expect(app.quits()).toBe(1);

    app.ui.unmount();
    await app.renderer.renderOnce();
    expect(app.renderer.captureCharFrame()).not.toContain("TARGETS");
  } finally {
    app.cleanup();
    app.renderer.renderer.destroy();
  }
});
