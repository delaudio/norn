import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import {
  FakeProviderTransport,
  providerFixtureRepositories,
} from "../src/transport/fakeProviderTransport";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

const settle = async () => {
  for (let i = 0; i < 12; i++) await Bun.sleep(0);
};

test("saved review stays pinned until explicit preview; refresh, images, clipboard and return retain identity", async () => {
  const transport = new FakeProviderTransport();
  transport.images = true;
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    await store.selectProviderPr(providerFixtureRepositories[0]!, 7, "Saved", "OPEN");
    expect(store.getSnapshot().providerPreview).toBe(false);
    expect(transport.calls.some((call) => call.method === "provider.snapshot")).toBe(false);
    await store.toggleProviderPreview();
    const first = store.getSnapshot().liveSnapshot!;
    expect(first.snapshot.repository.provider).toBe("github");
    expect(store.getSnapshot().findings).toEqual([]);
    expect(store.getSnapshot().threads).toEqual([]);
    expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]?.runId).toBe(
      "real-saved-run",
    );
    let copied = "";
    await store.copyReviewOutput(client, (text) => {
      copied = text;
      return true;
    });
    expect(copied).toContain("github PR 7 世界");
    expect(transport.calls.some((call) => call.method === "review.output")).toBe(false);
    await store.startReview(client);
    store.beginReply();
    await store.previewFix(client);
    expect(
      transport.calls.some((call) =>
        ["review.start", "review.reply", "review.fix.preview"].includes(call.method),
      ),
    ).toBe(false);
    const settings = await client.settings();
    await store.applySettings(client, settings.settings);
    expect(store.getSnapshot().liveSnapshot?.snapshot.snapshotId).toBe(first.snapshot.snapshotId);
    expect(transport.calls.filter((call) => call.method === "provider.snapshot")).toHaveLength(1);
    store.cycleFocus();
    store.move(1);
    await settle();
    expect(store.getSnapshot().preview?.before?.available).toBe(true);
    expect(store.getSnapshot().preview?.after?.contentSha256).not.toBe(
      store.getSnapshot().preview?.before?.contentSha256,
    );
    await store.openBrowser(client);
    expect(transport.calls.at(-1)?.method).toBe("provider.snapshot.browser");
    expect(transport.calls.at(-1)?.params.snapshot).toEqual(first.snapshot);
    await store.toggleProviderPreview(true);
    expect(store.getSnapshot().liveSnapshot?.snapshot.snapshotId).not.toBe(
      first.snapshot.snapshotId,
    );
    transport.fault = "snapshot";
    await store.toggleProviderPreview(true);
    expect(store.getSnapshot().liveSnapshot).toBeNull();
    expect(store.getSnapshot().files).toEqual([]);
    expect(store.getSnapshot().diff).toBeNull();
    expect(store.getSnapshot().error).toContain("unavailable");
    transport.fault = null;
    await store.toggleProviderPreview();
    expect(store.getSnapshot().providerPreview).toBe(false);
    expect(
      transport.calls.filter((call) => call.method === "review.files").at(-1)?.params.target,
    ).toMatchObject({ runId: "real-saved-run" });
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("queued provider changes discard an old snapshot before reading its inventory", async () => {
  const transport = new FakeProviderTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    let release!: () => void;
    transport.gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const first = store.selectProviderPr(providerFixtureRepositories[0]!, 8, "First", "OPEN");
    await settle();
    await store.selectProviderPr(providerFixtureRepositories[1]!, 8, "Second", "OPEN");
    transport.gate = null;
    release();
    await first;
    await settle();
    expect(store.getSnapshot().liveSnapshot?.snapshot.repository.provider).toBe("bitbucket");
    expect(store.getSnapshot().diff?.text).toContain("bitbucket PR 8 世界");
    expect(
      transport.calls.filter((call) => call.method === "provider.snapshot.files"),
    ).toHaveLength(1);
    expect(
      transport.calls.some((call) =>
        ["diff.file", "review.start", "file.preview"].includes(call.method),
      ),
    ).toBe(false);
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("closing during snapshot capture discards evidence before any inventory or patch read", async () => {
  const transport = new FakeProviderTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  let release!: () => void;
  transport.gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const pending = store.selectProviderPr(providerFixtureRepositories[1]!, 8, "Pending", "OPEN");
  await settle();
  store.markClosed();
  transport.gate = null;
  release();
  await pending;
  expect(store.getSnapshot().status).toBe("closed");
  expect(store.getSnapshot().liveSnapshot).toBeNull();
  expect(transport.calls.some((call) => call.method.startsWith("provider.snapshot."))).toBe(false);
  store.cancelSyntax();
  client.close();
});

for (const width of [120, 50])
  test(`native F3/F5 preserve evidence and responsive input at ${width} columns`, async () => {
    const transport = new FakeProviderTransport();
    const client = new BackendClient(transport);
    const store = new ShellStore();
    await store.connect(client);
    await store.selectProviderPr(providerFixtureRepositories[0]!, 7, "Saved", "OPEN");
    const renderer = await createTestRenderer({ width, height: width === 50 ? 15 : 30 });
    const ui = mountApp(renderer.renderer, store, client);
    const render = async () => {
      await settle();
      for (
        let i = 0;
        i < 100 && (store.getSnapshot().snapshotLoading || store.getSnapshot().diffLoading);
        i++
      )
        await Bun.sleep(2);
      await renderer.renderOnce();
    };
    try {
      await render();
      expect(renderer.captureCharFrame()).toContain("Saved review");
      renderer.mockInput.pressKey(KeyCodes.F3);
      await render();
      expect(renderer.captureCharFrame()).toContain("PR preview");
      expect(store.getSnapshot().diff?.text).toContain("github PR 7");
      const old = store.getSnapshot().liveSnapshot?.snapshot.snapshotId;
      renderer.mockInput.pressKey(KeyCodes.F5);
      await render();
      expect(store.getSnapshot().liveSnapshot?.snapshot.snapshotId).not.toBe(old);
      renderer.mockInput.pressKey("s");
      await render();
      expect(store.getSnapshot().diffMode).toBe("split");
      renderer.mockInput.pressKey(KeyCodes.F3);
      await render();
      expect(store.getSnapshot().providerPreview).toBe(false);
      expect(renderer.captureCharFrame()).toContain("Saved review");
    } finally {
      ui.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });

test("canonical local-only saved scopes never become provider previews or issue provider reads", async () => {
  class NativeScopeTransport extends FakeBackendTransport {
    calls: string[] = [];
    async start() {
      const ready = await super.start();
      return { ...ready, capabilities: [...ready.capabilities, "provider-snapshot-preview"] };
    }
    async request(method: string, params: Record<string, unknown>) {
      this.calls.push(method);
      if (method.startsWith("provider."))
        throw new Error("Local-only saved data attempted a provider request.");
      return super.request(method, params);
    }
  }
  const transport = new NativeScopeTransport({
    workspaceRecords: {
      "viewed.github:owned/repo/2147483649": { kind: "viewed", value: ["saved.rs"] },
    },
  });
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    await store.selectPicker(0);
    await store.toggleProviderPreview(true);
    expect(store.getSnapshot().picker[0]?.prId).toBe(2147483649);
    expect(store.getSnapshot().providerPreview).toBe(false);
    expect(transport.calls.some((method) => method.startsWith("provider."))).toBe(false);
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

test("preview review start carries exact capture plus explicit profile when supported", async () => {
  class EvidenceTransport extends FakeProviderTransport {
    async start() {
      const ready = await super.start();
      return { ...ready, capabilities: [...ready.capabilities, "provider-evidence-reviews"] };
    }
    async request(method: string, params: Record<string, unknown>) {
      if (method === "review.start") {
        this.calls.push({ method, params: structuredClone(params) });
        return { operationId: "exact-capture-operation", state: "accepted", sequence: 0 };
      }
      return super.request(method, params);
    }
  }
  const transport = new EvidenceTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  try {
    await store.connect(client);
    await store.selectProviderPr(providerFixtureRepositories[1]!, 8, "Owned", "OPEN");
    const captured = store.getSnapshot().liveSnapshot!.snapshot;
    store.beginReviewProfile();
    store.appendReplyText("security");
    store.submitReviewProfile();
    expect(store.getSnapshot().selectedReviewProfile).toBe("security");
    await store.startReview(client);
    expect(transport.calls.find((call) => call.method === "review.start")?.params).toEqual({
      target: { kind: "provider", ...captured.repository, prId: captured.prId },
      profile: "security",
      snapshot: captured,
    });
    store.beginReply();
    expect(store.getSnapshot().composing).toBe(false);
  } finally {
    store.cancelSyntax();
    client.close();
  }
});

for (const width of [120, 50])
  test(`F6 profile input preserves cancel/default and fences pasted shortcuts at ${width} columns`, async () => {
    const client = new BackendClient(new FakeProviderTransport());
    const store = new ShellStore();
    await store.connect(client);
    const renderer = await createTestRenderer({ width, height: width === 50 ? 15 : 30 });
    const ui = mountApp(renderer.renderer, store, client);
    const render = async () => {
      await settle();
      await renderer.renderOnce();
    };
    try {
      renderer.mockInput.pressKey(KeyCodes.F6);
      await render();
      expect(renderer.captureCharFrame()).toContain("Profile (empty = default)");
      await renderer.mockInput.pasteBracketedText("security");
      renderer.mockInput.pressEnter();
      await render();
      expect(store.getSnapshot().selectedReviewProfile).toBe("security");
      renderer.mockInput.pressKey(KeyCodes.F6);
      await render();
      await renderer.mockInput.pasteBracketedText("q\nx\n");
      renderer.mockInput.pressEnter();
      await render();
      expect(store.getSnapshot().composing).toBe(true);
      expect(store.getSnapshot().status).not.toBe("closed");
      expect(store.getSnapshot().selectedReviewProfile).toBe("security");
      renderer.mockInput.pressEscape();
      await Bun.sleep(60);
      await render();
      expect(store.getSnapshot().composing).toBe(false);
      expect(store.getSnapshot().selectedReviewProfile).toBe("security");
      store.beginReviewProfile();
      for (let i = 0; i < 8; i++) store.backspaceReply();
      store.submitReviewProfile();
      expect(store.getSnapshot().selectedReviewProfile).toBeNull();
    } finally {
      ui.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });
