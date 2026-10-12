import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { EditorExplorerFixture } from "./fixtures/editor-backend";

for (const [width, height] of [
  [120, 30],
  [50, 20],
]) {
  test(`explicit editor actions preserve native navigation at ${width} columns`, async () => {
    const fixture = new EditorExplorerFixture(),
      client = new BackendClient(fixture),
      store = new ShellStore();
    await store.connect(client);
    const before = store.getSnapshot();
    const native = await createTestRenderer({ width, height });
    const ui = mountApp(native.renderer, store, client);
    const settle = async () => {
      for (let pass = 0; pass < 10; pass++) {
        await Bun.sleep(0);
        await native.renderOnce();
      }
    };
    try {
      await settle();
      native.mockInput.pressKey(KeyCodes.F8);
      await settle();
      native.mockInput.pressEnter();
      await settle();
      await native.mockInput.pasteBracketedText("o\nO\nq");
      await settle();
      expect(fixture.editorCalls).toHaveLength(0);
      native.mockInput.pressKey("o");
      await settle();
      expect(fixture.editorCalls.map((call) => call.method)).toEqual([
        "repository.editor.prepare",
        "repository.editor.open",
      ]);
      expect(fixture.prepared?.path).toBe("README.md");
      expect(fixture.prepared?.line).toBe(1);
      expect(fixture.prepared?.mode).toBe("graphical");
      expect(native.captureCharFrame()).toContain("zed · launched");
      native.mockInput.pressKey("2");
      await settle();
      native.mockInput.pressKey("o");
      await settle();
      expect(fixture.prepared?.line).toBeNull();
      fixture.state = "failed";
      native.mockInput.pressKey("o");
      await settle();
      expect(native.captureCharFrame()).toContain("Owned launcher failed.");
      native.mockInput.pressEscape();
      await Bun.sleep(30);
      await settle();
      expect(store.getSnapshot().explorerOpen).toBe(false);
      expect(store.getSnapshot().selectedPicker).toBe(before.selectedPicker);
      expect(store.getSnapshot().selectedFile).toBe(before.selectedFile);
      expect(store.getSnapshot().selectedFinding).toBe(before.selectedFinding);
    } finally {
      ui.unmount();
      store.cancelSyntax();
      await client.close();
      native.renderer.destroy();
    }
  });
}
