import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("native first-run input opens masked settings and refresh is status-only", async () => {
  const calls: string[] = [];
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      calls.push(method);
      return super.request(method, params);
    }
  }
  const client = new BackendClient(
    new Transport({ repositories: [], availableTools: ["git"], credentialStore: "unsupported" }),
  );
  const store = new ShellStore();
  await store.connect(client);
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const ui = mountApp(renderer.renderer, store, client);
  const settle = async () => {
    for (let index = 0; index < 5; index++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    await settle();
    expect(renderer.captureCharFrame()).toContain("Welcome to Norn");
    expect(renderer.captureCharFrame()).toContain("no native credential store");
    renderer.mockInput.pressKey("m");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Native credential store unavailable");
    expect(renderer.captureCharFrame()).toContain("Tools: git found");
    const before = calls.filter((method) => method === "workspace.readiness").length;
    renderer.mockInput.pressKey("r", { ctrl: true });
    await settle();
    expect(calls.filter((method) => method === "workspace.readiness").length).toBe(before + 1);
    renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await settle();
    expect(store.getSnapshot().settingsOpen).toBe(false);
    expect(
      calls.some((method) => ["review.start", "settings.save", "auth.set"].includes(method)),
    ).toBe(false);
  } finally {
    ui.unmount();
    client.close();
    renderer.renderer.destroy();
  }
});
