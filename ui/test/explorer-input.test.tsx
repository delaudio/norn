import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { ExplorerFixture } from "./fixtures/explorer-backend";

for (const [width, height] of [
  [120, 30],
  [50, 20],
]) {
  test(`native F8 explorer input preserves review selection at ${width} columns`, async () => {
    const fixture = new ExplorerFixture(),
      client = new BackendClient(fixture),
      store = new ShellStore();
    await store.connect(client);
    const before = store.getSnapshot();
    const renderer = await createTestRenderer({ width, height });
    const ui = mountApp(renderer.renderer, store, client);
    const settle = async () => {
      for (let index = 0; index < 8; index++) {
        await Bun.sleep(0);
        await renderer.renderOnce();
      }
    };
    try {
      await settle();
      renderer.mockInput.pressKey(KeyCodes.F8);
      await settle();
      expect(store.getSnapshot().explorerOpen).toBe(true);
      expect(renderer.captureCharFrame()).toContain("FILES · github owned/same");
      renderer.mockInput.pressEnter();
      await settle();
      expect(renderer.captureCharFrame()).toContain("Owned explorer");
      renderer.mockInput.pressKey("v");
      await settle();
      // The bundled Markdown grammar initializes in OpenTUI's real worker.
      const deadline = Date.now() + 3000;
      while (!renderer.captureCharFrame().includes("Owned explorer") && Date.now() < deadline) {
        await Bun.sleep(10);
        await renderer.renderOnce();
      }
      expect(renderer.captureCharFrame()).toContain("rendered");
      expect(renderer.captureCharFrame()).toContain("Owned explorer");
      expect(renderer.captureCharFrame()).not.toContain("**Readable**");
      renderer.mockInput.pressKey("v");
      await settle();
      await renderer.mockInput.pasteBracketedText("q\n2\n3\nF8");
      await settle();
      expect(store.getSnapshot().explorerOpen).toBe(true);
      renderer.mockInput.pressKey("2");
      await settle();
      expect(renderer.captureCharFrame()).toContain("DIFF · README.md");
      renderer.mockInput.pressKey("3");
      await settle();
      expect(renderer.captureCharFrame()).toContain("BLAME · README.md");
      renderer.mockInput.pressTab();
      await settle();
      for (let index = 0; index < 20; index++) renderer.mockInput.pressKey("j");
      await settle();
      expect(renderer.captureCharFrame()).toContain("Retained final paragraph");
      renderer.mockInput.pressKey("1");
      await settle();
      renderer.mockInput.pressKey("f", { ctrl: true });
      await settle();
      await renderer.mockInput.pasteBracketedText("Readable");
      renderer.mockInput.pressEnter();
      await settle();
      expect(renderer.captureCharFrame()).toContain("Find Readable · 1/1");
      renderer.mockInput.pressKey("/");
      await settle();
      await renderer.mockInput.pasteBracketedText("src/");
      renderer.mockInput.pressEnter();
      await settle();
      expect(
        fixture.calls.filter((call) => call.method === "repository.explore.files").at(-1)?.params
          .query,
      ).toBe("src/");
      renderer.mockInput.pressEscape();
      await Bun.sleep(30);
      await settle();
      expect(store.getSnapshot().explorerOpen).toBe(false);
      expect(store.getSnapshot().selectedPicker).toBe(before.selectedPicker);
      expect(store.getSnapshot().selectedFile).toBe(before.selectedFile);
    } finally {
      ui.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });
}
