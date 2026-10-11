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
    expect(app.renderer.captureCharFrame()).toContain("review succeeded");

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

test("reply composer accepts bracketed paste without executing pasted shortcuts", async () => {
  const renderer = await createTestRenderer({ width: 100, height: 30 });
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 7,
          title: "Review",
          latestRunId: "run-1",
          status: "completed",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  let quits = 0;
  const ui = mountApp(renderer.renderer, store, client, () => {
    quits += 1;
  });
  try {
    store.move(2);
    store.beginReply();
    await renderer.mockInput.pasteBracketedText("qjk\nPlease explain 👩‍💻");
    expect(store.getSnapshot().composerText).toBe("qjk\nPlease explain 👩‍💻");
    expect(quits).toBe(0);
    store.backspaceReply();
    expect(store.getSnapshot().composerText).toBe("qjk\nPlease explain ");
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});

test("large diffs render collapsed with an expand hint", async () => {
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const body = Array.from({ length: 2500 }, (_, i) => (i % 2 === 0 ? `+l${i}` : `-l${i}`)).join(
    "\n",
  );
  const diffText = `@@ -1,2500 +1,2500 @@\n${body}\n`;
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      files: [{ path: "src/a.ts", status: "modified", additions: 1, deletions: 1, oldPath: null }],
      findings: [],
      diffText,
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  const ui = mountApp(renderer.renderer, store, client, () => {});
  await renderer.renderOnce();
  try {
    const frame = renderer.captureCharFrame();
    expect(frame).toContain("collapsed");
    expect(frame).toContain("e expand");
    expect(store.getSnapshot().diffLineCount).toBeGreaterThan(2000);
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});

test("streams review events into a bounded operation panel", async () => {
  const app = await mount(100, 30);
  try {
    app.renderer.mockInput.pressEnter();
    await new Promise((resolve) => setTimeout(resolve, 10));
    await app.renderer.renderOnce();
    const frame = app.renderer.captureCharFrame();
    expect(frame).toContain("review succeeded");
    expect(frame).toContain("collected file");
  } finally {
    app.cleanup();
    app.ui.unmount();
    app.renderer.renderer.destroy();
  }
});

test("fix confirmation intercepts shortcuts and requires a fresh explicit approval", async () => {
  const renderer = await createTestRenderer({ width: 100, height: 30 });
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 9,
          runId: "run-fix",
          title: "Fix",
          status: "succeeded",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  let quits = 0;
  const ui = mountApp(renderer.renderer, store, client, () => {
    quits += 1;
  });
  try {
    renderer.mockInput.pressKey("a");
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("Approve filesystem changes?");
    renderer.mockInput.pressKey("q");
    renderer.mockInput.pressKey("r");
    expect(quits).toBe(0);
    expect(store.getSnapshot().composing).toBe(false);
    expect(store.getSnapshot().operation).toBeNull();
    renderer.mockInput.pressKey("n");
    await renderer.renderOnce();
    expect(store.getSnapshot().fixPreview).toBeNull();
    renderer.mockInput.pressKey("y");
    expect(store.getSnapshot().operation).toBeNull();
    renderer.mockInput.pressKey("a");
    await Bun.sleep(5);
    renderer.mockInput.pressKey("y");
    await Bun.sleep(5);
    expect(store.getSnapshot().operation?.id).toContain("fake-fix");
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});

test("commit composer opens a preview and requires y before executing", async () => {
  const renderer = await createTestRenderer({ width: 100, height: 30 });
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 9,
          runId: "run-fix",
          title: "Fix",
          status: "succeeded",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  const ui = mountApp(renderer.renderer, store, client, () => {});
  try {
    renderer.mockInput.pressKey("c");
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("Commit › fix: guard null");
    renderer.mockInput.pressKey("r");
    expect(store.getSnapshot().composerText).toBe("fix: guard nullr");
    renderer.mockInput.pressEnter();
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("Approve commit?");
    expect(store.getSnapshot().operation).toBeNull();
    renderer.mockInput.pressKey("p");
    expect(store.getSnapshot().fixPreview?.action).toBe("commit");
    renderer.mockInput.pressKey("y");
    await Bun.sleep(5);
    expect(store.getSnapshot().operation?.id).toContain("fake-commit");
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});

test("branch shortcut previews cannot execute or switch action without approval", async () => {
  const renderer = await createTestRenderer({ width: 100, height: 30 });
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories,
      targets: [
        {
          provider: "github",
          workspace: "acme",
          repo: "payments",
          prId: 9,
          runId: "run-fix",
          title: "Fix",
          status: "succeeded",
        },
      ],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await store.selectPicker(2);
  const ui = mountApp(renderer.renderer, store, client, () => {});
  try {
    renderer.mockInput.pressKey("u");
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("Approve branch sync?");
    renderer.mockInput.pressKey("o");
    expect(store.getSnapshot().fixPreview?.action).toBe("sync");
    expect(store.getSnapshot().operation).toBeNull();
    renderer.mockInput.pressKey("n");
    renderer.mockInput.pressKey("o");
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("Approve conflict resolution?");
    renderer.mockInput.pressKey("y");
    await Bun.sleep(5);
    expect(store.getSnapshot().operation?.id).toContain("fake-conflicts");
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});

test("mounted operation polling stops after unmount", async () => {
  class PollTransport extends FakeBackendTransport {
    statusReads = 0;
    async request(method: string, params: Record<string, unknown>) {
      if (method === "review.start")
        return { operationId: "poll-owned", state: "accepted", sequence: 0 };
      if (method === "operation.status") {
        this.statusReads++;
        return { operationId: "poll-owned", state: "running", sequence: 0, error: null };
      }
      return super.request(method, params);
    }
  }
  const renderer = await createTestRenderer({ width: 100, height: 30 });
  const transport = new PollTransport({ repositories });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  const ui = mountApp(renderer.renderer, store, client, () => {});
  try {
    await store.startReview(client);
    await Bun.sleep(1100);
    expect(transport.statusReads).toBeGreaterThanOrEqual(2);
    ui.unmount();
    const reads = transport.statusReads;
    await Bun.sleep(1100);
    expect(transport.statusReads).toBe(reads);
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});

test("settings mask pasted credentials and close without running review shortcuts", async () => {
  const renderer = await createTestRenderer({ width: 120, height: 40 });
  const client = new BackendClient(new FakeBackendTransport({ repositories }));
  const store = new ShellStore();
  await store.connect(client);
  let quits = 0;
  const ui = mountApp(renderer.renderer, store, client, () => {
    quits++;
  });
  try {
    renderer.mockInput.pressKey("m");
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("Settings");
    for (let i = 0; i < 22; i++) renderer.mockInput.pressTab();
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).toContain("› New credential");
    renderer.mockInput.pressEnter();
    await Bun.sleep(5);
    await renderer.mockInput.pasteBracketedText("qapcrym-fixture-sensitive-value");
    await renderer.renderOnce();
    expect(renderer.captureCharFrame()).not.toContain("fixture-sensitive-value");
    expect(renderer.captureCharFrame()).toContain("••");
    expect(store.getSnapshot().operation).toBeNull();
    expect(quits).toBe(0);
    renderer.mockInput.pressEscape();
    // A raw ESC needs the terminal parser's ambiguity window before q.
    await Bun.sleep(50);
    renderer.mockInput.pressKey("q");
    await Bun.sleep(5);
    await renderer.renderOnce();
    expect(store.getSnapshot().settingsOpen).toBe(false);
    expect(quits).toBe(0);
    expect(await client.credentialStatus()).not.toContainEqual(
      expect.objectContaining({ available: true }),
    );
  } finally {
    ui.unmount();
    renderer.renderer.destroy();
  }
});
