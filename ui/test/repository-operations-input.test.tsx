import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { RepositoryOperationsFixture } from "./fixtures/repository-operations-backend";

for (const [width, height] of [
  [120, 30],
  [50, 20],
])
  test(`native branch approvals preserve review selection and ignore pasted commands at ${width} columns`, async () => {
    const fixture = new RepositoryOperationsFixture(),
      client = new BackendClient(fixture),
      store = new ShellStore();
    if (width === 50) fixture.summary = `${"x".repeat(1200)} TARGET-END`;
    await store.connect(client);
    const before = store.getSnapshot();
    const renderer = await createTestRenderer({ width, height });
    const ui = mountApp(renderer.renderer, store, client);
    const settle = async () => {
      for (let i = 0; i < 20; i++) {
        await Bun.sleep(1);
        await renderer.renderOnce();
      }
    };
    // Poll for an expected frame instead of assuming a fixed number of yields
    // is enough for the async preview/apply to resolve and render.
    const waitForFrame = async (text: string) => {
      for (let i = 0; i < 150; i++) {
        await Bun.sleep(2);
        await renderer.renderOnce();
        if (renderer.captureCharFrame().includes(text)) {
          return true;
        }
      }
      return false;
    };
    try {
      renderer.mockInput.pressKey(KeyCodes.F8);
      await settle();
      renderer.mockInput.pressKey("b");
      await settle();
      expect(renderer.captureCharFrame()).toContain("BRANCHES");
      renderer.mockInput.pressEnter();
      expect(await waitForFrame("CONFIRM GIT OPERATION")).toBe(true);
      await renderer.mockInput.pasteBracketedText("y\nf\np\ns\n");
      await settle();
      expect(fixture.operationCalls.some((call) => call.method.endsWith(".apply"))).toBe(false);
      if (width === 50) {
        expect(renderer.captureCharFrame()).not.toContain("TARGET-END");
        for (let page = 0; page < 4; page++) {
          renderer.mockInput.pressKey("d", { ctrl: true });
          await settle();
        }
        expect(await waitForFrame("TARGET-END")).toBe(true);
        expect(fixture.operationCalls.some((call) => call.method.endsWith(".apply"))).toBe(false);
      }
      renderer.mockInput.pressKey("y");
      await settle();
      expect(fixture.operationCalls.filter((call) => call.method.endsWith(".apply"))).toHaveLength(
        1,
      );
      renderer.mockInput.pressEscape();
      await Bun.sleep(30);
      await settle();
      expect(await waitForFrame("FILES")).toBe(true);
      renderer.mockInput.pressEscape();
      await Bun.sleep(30);
      await settle();
      expect(store.getSnapshot().selectedPicker).toBe(before.selectedPicker);
      expect(store.getSnapshot().selectedFile).toBe(before.selectedFile);
    } finally {
      ui.unmount();
      store.cancelSyntax();
      renderer.renderer.destroy();
      await client.close();
    }
  });
