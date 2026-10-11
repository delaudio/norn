import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";
import { importedScopeRecords } from "../src/transport/scopeFixtures";

test("native unconfigured scope picker reaches imported drafts across providers and resize", async () => {
  const calls: string[] = [];
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      calls.push(method);
      return super.request(method, params);
    }
  }
  const client = new BackendClient(
    new Transport({ repositories: [], workspaceRecords: importedScopeRecords }),
  );
  const store = new ShellStore();
  await store.connect(client);
  const renderer = await createTestRenderer({ width: 120, height: 30 });
  const ui = mountApp(renderer.renderer, store, client);
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    await settle();
    expect(renderer.captureCharFrame()).toContain("Saved PR #7");
    expect(renderer.captureCharFrame()).not.toContain("Welcome to Norn");
    renderer.mockInput.pressKey("d");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Imported GitHub comment 世界");
    renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await settle();
    expect(store.getSnapshot().draftsOpen).toBe(false);
    renderer.mockInput.pressKey("j");
    await settle();
    expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]?.provider).toBe(
      "bitbucket",
    );
    renderer.mockInput.pressKey("d");
    await settle();
    expect(renderer.captureCharFrame()).toContain("Imported Bitbucket comment ✓");
    expect(renderer.captureCharFrame()).not.toContain("Imported GitHub comment");
    renderer.resize(50, 15);
    await settle();
    expect(renderer.captureCharFrame()).toContain("Imported Bitbucket");
    expect(calls).not.toContain("review.start");
    expect(calls).not.toContain("publication.apply");
    expect(calls).not.toContain("workspace.mutate");
  } finally {
    ui.unmount();
    client.close();
    renderer.renderer.destroy();
  }
});
