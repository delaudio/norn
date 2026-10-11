import { expect, test } from "bun:test";
import { createTestRenderer } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("native theme preview is modal and user-data round trip retains settings drafts", async () => {
  const calls: string[] = [];
  class Transport extends FakeBackendTransport {
    async request(method: string, params: Record<string, unknown>) {
      calls.push(method);
      return super.request(method, params);
    }
  }
  const client = new BackendClient(
    new Transport({ workspaceRecords: { theme: { kind: "theme", value: "light" } } }),
  );
  const shell = new ShellStore();
  await shell.connect(client);
  const renderer = await createTestRenderer({ width: 120, height: 40 });
  const ui = mountApp(renderer.renderer, shell, client);
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await Bun.sleep(0);
      await renderer.renderOnce();
    }
  };
  try {
    renderer.mockInput.pressKey("m");
    await settle();
    renderer.mockInput.pressKey("t", { ctrl: true });
    await settle();
    expect(renderer.captureCharFrame()).toContain("Theme draft: dark → imported light");
    renderer.mockInput.pressKey("s", { ctrl: true });
    await settle();
    expect(calls.filter((method) => method === "settings.save")).toHaveLength(0);
    renderer.mockInput.pressKey("y", { ctrl: true });
    await settle();
    expect(renderer.captureCharFrame()).toContain("Imported theme staged");
    const before = calls.filter((method) => method === "settings.get").length;
    renderer.mockInput.pressKey("d", { ctrl: true });
    await settle();
    expect(renderer.captureCharFrame()).toContain("User data");
    renderer.mockInput.pressEscape();
    await Bun.sleep(30);
    await settle();
    expect(renderer.captureCharFrame()).toContain("Imported theme staged");
    expect(calls.filter((method) => method === "settings.get")).toHaveLength(before);
    renderer.mockInput.pressKey("s", { ctrl: true });
    await settle();
    expect((await client.settings()).settings.theme).toBe("light");
    expect(
      calls.some(
        (method) =>
          method === "workspace.mutate" || method === "auth.set" || method === "review.start",
      ),
    ).toBe(false);
  } finally {
    ui.unmount();
    client.close();
    renderer.renderer.destroy();
  }
});
