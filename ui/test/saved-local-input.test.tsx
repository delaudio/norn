import { expect, test } from "bun:test";
import { createTestRenderer, KeyCodes } from "@opentui/core/testing";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { SavedTransport } from "./fixtures/saved-local-backend";

for (const width of [120, 50])
  test(`saved local native input opens immutable evidence and ignores pasted actions at ${width}`, async () => {
    const transport = new SavedTransport();
    const client = new BackendClient(transport);
    const store = new ShellStore();
    await store.connect(client);
    const captures = transport.calls.filter((method) => method === "local.snapshot").length;
    const renderer = await createTestRenderer({ width, height: width === 50 ? 18 : 30 });
    const app = mountApp(renderer.renderer, store, client);
    const render = async () => {
      for (let i = 0; i < 12; i++) await Bun.sleep(0);
      await renderer.renderOnce();
    };
    try {
      renderer.mockInput.pressKey(KeyCodes.F7);
      await render();
      expect(renderer.captureCharFrame()).toContain("SAVED LOCAL SNAPSHOTS");
      expect(renderer.captureCharFrame()).toContain("Selected clone 9999999999999999");
      await renderer.mockInput.pasteBracketedText("q\nr\n\r\nx");
      await render();
      expect(transport.savedReads).toBe(1);
      expect(store.getSnapshot().savedLocal?.cursor).toBe(0);
      await store.pageSavedLocal(client, 1);
      await render();
      expect(store.getSnapshot().savedLocal?.offset).toBe(32);
      expect(renderer.captureCharFrame()).toContain("33–35 of 35");
      const selected = store.getSnapshot().savedLocal!.items[0]!.snapshot;
      renderer.mockInput.pressEnter();
      await render();
      expect(store.getSnapshot().savedLocal).toBeNull();
      expect(store.getSnapshot().localInfo?.snapshot).toEqual(selected);
      expect(store.getSnapshot().picker[store.getSnapshot().selectedPicker]?.title).toBe(
        "Saved local snapshot",
      );
      expect(transport.calls.filter((method) => method === "local.snapshot")).toHaveLength(
        captures,
      );
      expect(transport.calls).not.toContain("review.files");
      expect(transport.calls).not.toContain("local.review.start");
    } finally {
      app.unmount();
      store.cancelSyntax();
      client.close();
      renderer.renderer.destroy();
    }
  });
test("closing the saved selector fences a late page and a late snapshot selection", async () => {
  const transport = new SavedTransport();
  const client = new BackendClient(transport);
  const store = new ShellStore();
  await store.connect(client);
  const before = store.getSnapshot().localInfo;
  const original = transport.request.bind(transport);
  try {
    for (const method of ["local.snapshot.list", "local.snapshot.info"]) {
      let release!: () => void;
      const gate = new Promise<void>((resolve) => {
        release = resolve;
      });
      if (method === "local.snapshot.info") await store.openSavedLocal(client);
      transport.request = async (requested, params) => {
        if (requested === method) await gate;
        return original(requested, params);
      };
      const pending =
        method === "local.snapshot.list"
          ? store.openSavedLocal(client)
          : store.chooseSavedLocal(client);
      store.dismissSavedLocal();
      release();
      await pending;
      expect(store.getSnapshot().savedLocal).toBeNull();
      expect(store.getSnapshot().localInfo).toEqual(before);
      transport.request = original;
    }
  } finally {
    store.cancelSyntax();
    client.close();
  }
});
