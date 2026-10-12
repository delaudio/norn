import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeProviderTransport } from "../src/transport/fakeProviderTransport";

test("native F2 opens grouped PRs, filters, pages, detail and full-scope saved workspace", async () => {
  const transport = new FakeProviderTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const ui = mountApp(renderer.renderer, store, client);
  const settle = async () => {
    for (let i = 0; i < 8; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  const dismiss = async () => {
    renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await settle();
  };
  try {
    await settle();
    expect(renderer.captureCharFrame()).toContain("F2 PRs");
    renderer.mockInput.pressKey(KeyCodes.F2);
    await settle();
    expect(store.getSnapshot().providerOpen).toBe(true);
    expect(renderer.captureCharFrame()).toContain("PULL REQUESTS");
    expect(renderer.captureCharFrame()).toContain("github change 7");
    expect(renderer.captureCharFrame()).toContain("bitbucket change 7");
    renderer.mockInput.pressEnter();
    await settle();
    expect(renderer.captureCharFrame()).toContain("Actual synthetic description");
    expect(renderer.captureCharFrame()).toContain("Reviewer · REVIEWER · approved");
    expect(renderer.captureCharFrame()).toContain("Head: bbbbbbbb");
    renderer.resize(50, 15);
    await settle();
    expect(renderer.captureCharFrame()).toContain("github synthetic/review PR #7");
    await dismiss();
    expect(renderer.captureCharFrame()).toContain("PULL REQUESTS");
    renderer.mockInput.pressKey("n");
    await settle();
    expect(
      transport.calls.filter((call) => call.method === "provider.prs").at(-1)?.params.page,
    ).toBe(2);
    renderer.mockInput.pressTab();
    renderer.mockInput.pressTab();
    renderer.mockInput.pressArrow("right");
    await settle();
    expect(renderer.captureCharFrame()).toContain("State: DRAFT");
    expect(renderer.captureCharFrame()).not.toContain("change 7");
    expect(renderer.captureCharFrame()).toContain("DRAFT github change 8");
    renderer.mockInput.pressKey("/");
    await settle();
    await renderer.mockInput.pasteBracketedText("世界");
    await settle();
    renderer.mockInput.pressEnter();
    await settle();
    expect(
      transport.calls.filter((call) => call.method === "provider.prs").at(-1)?.params.query,
    ).toBe("世界");
    renderer.mockInput.pressKey(" ");
    await settle();
    expect(store.getSnapshot().providerOpen).toBe(false);
    expect(store.getSnapshot().error).toBeNull();
    expect(store.getSnapshot().files.map((file) => file.path)).toEqual(["readme.ts"]);
    expect(store.getSnapshot().providerPreview).toBe(true);
    expect(store.getSnapshot().diff?.text).toContain("github PR 8 世界");
    expect(transport.calls.filter((call) => call.method === "diff.file")).toHaveLength(0);
    expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]).toMatchObject({
      provider: "github",
      workspace: "synthetic",
      repo: "review",
      prId: 8,
      runId: null,
    });
    expect(
      transport.calls.some(
        (call) => call.method === "review.start" || call.method === "publication.apply",
      ),
    ).toBe(false);
  } finally {
    ui.unmount();
    store.cancelSyntax();
    client.close();
    renderer.renderer.destroy();
  }
});

test("Escape, settings recovery and Ctrl-C remain usable during pending provider reads", async () => {
  const transport = new FakeProviderTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  let release!: () => void;
  transport.gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const renderer = await createTestRenderer({ width: 50, height: 15 });
  let quit = 0;
  const ui = mountApp(renderer.renderer, store, client, () => {
    quit++;
  });
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    await settle();
    renderer.mockInput.pressKey(KeyCodes.F2);
    await settle();
    expect(renderer.captureCharFrame()).toContain("loading");
    renderer.resize(120, 30);
    await settle();
    renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await settle();
    expect(store.getSnapshot().providerOpen).toBe(false);
    renderer.mockInput.pressKey(KeyCodes.F2);
    await settle();
    renderer.mockInput.pressKey("m");
    await settle();
    expect(store.getSnapshot().settingsOpen).toBe(true);
    expect(store.getSnapshot().providerOpen).toBe(false);
    store.toggleSettings();
    await settle();
    renderer.mockInput.pressKey(KeyCodes.F2);
    await settle();
    renderer.mockInput.pressCtrlC();
    await settle();
    expect(quit).toBe(1);
    transport.gate = null;
    release();
    await settle();
    expect(transport.maxReads).toBeLessThanOrEqual(2);
  } finally {
    transport.gate = null;
    release();
    ui.unmount();
    store.cancelSyntax();
    client.close();
    renderer.renderer.destroy();
  }
});

for (const width of [120, 50])
  test(`explicit provider page key uses current identity and pasted keys do not launch at ${width} columns`, async () => {
    const transport = new FakeProviderTransport();
    const client = new BackendClient(transport);
    const store = new ShellStore();
    await store.connect(client);
    const renderer = await createTestRenderer({ width, height: width === 50 ? 15 : 30 });
    const ui = mountApp(renderer.renderer, store, client);
    const settle = async () => {
      for (let i = 0; i < 8; i++) {
        await Bun.sleep(0);
        await renderer.renderOnce();
      }
    };
    try {
      renderer.mockInput.pressKey(KeyCodes.F2);
      await settle();
      await renderer.mockInput.pasteBracketedText("b\nb\n");
      await settle();
      expect(transport.calls.filter((call) => call.method === "provider.browser")).toHaveLength(0);
      renderer.mockInput.pressKey("b");
      await settle();
      expect(transport.calls.filter((call) => call.method === "provider.browser")).toHaveLength(1);
      expect(transport.calls.find((call) => call.method === "provider.browser")?.params).toEqual({
        repository: { provider: "github", workspace: "synthetic", repo: "review" },
        prId: 7,
      });
      expect(renderer.captureCharFrame()).toContain("launching is disabled");
      renderer.mockInput.pressEnter();
      await settle();
      renderer.mockInput.pressKey("b");
      await settle();
      expect(transport.calls.filter((call) => call.method === "provider.browser")).toHaveLength(2);
      expect(transport.calls.filter((call) => call.method === "browser.open")).toHaveLength(0);
    } finally {
      ui.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });
