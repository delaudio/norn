import { expect, test } from "bun:test";
import { ImageRenderable, type Renderable } from "@opentui/core";
import { createTerminalCapabilities, createTestRenderer } from "@opentui/core/testing";
import { imageProtocol } from "../src/app/imageProtocol";
import { ShellStore } from "../src/app/store";
import { mountApp } from "../src/app/ui";
import { BackendClient } from "../src/transport/backendClient";
import { FakeBackendTransport } from "../src/transport/fakeTransport";

test("iTerm automatic image selection prefers physically verified Sixel with safe fallbacks", () => {
  const caps = createTerminalCapabilities({
    kitty_graphics: true,
    sixel: true,
    terminal: { name: "iTerm2", version: "3.6.11", from_xtversion: true },
  });
  expect(imageProtocol(caps, true)).toBe("sixel");
  expect(imageProtocol(caps, false)).toBe("blocks");
  expect(imageProtocol({ ...caps, sixel: false }, true)).toBe("blocks");
  expect(imageProtocol({ ...caps, multiplexer: "tmux" }, true)).toBe("blocks");
  expect(imageProtocol({ ...caps, image_protocol: "kitty" }, true)).toBe("kitty");
  expect(imageProtocol({ ...caps, image_protocol: "blocks" }, true)).toBe("blocks");
  expect(imageProtocol(createTerminalCapabilities({ kitty_graphics: true }), true)).toBe("kitty");
  expect(imageProtocol(null, false)).toBe("blocks");
});

test("both native image panes fill available geometry across wide/narrow resize and clean up listeners", async () => {
  const scene = await createTestRenderer({ width: 120, height: 30 });
  const client = new BackendClient(
    new FakeBackendTransport({
      repositories: [
        { provider: "github", workspace: "synthetic", repo: "images", localPath: null },
      ],
      files: [{ path: "image.png", status: "modified", additions: 0, deletions: 0, oldPath: null }],
      findings: [],
    }),
  );
  const store = new ShellStore();
  await store.connect(client);
  await Bun.sleep(10);
  const baseline = scene.renderer.listenerCount("capabilities");
  const ui = mountApp(scene.renderer, store, client);
  function images(node: Renderable): ImageRenderable[] {
    return [
      ...(node instanceof ImageRenderable ? [node] : []),
      ...node.getChildren().flatMap(images),
    ];
  }
  try {
    for (const [width, height] of [
      [120, 30],
      [50, 20],
      [120, 30],
    ]) {
      scene.resize(width!, height!);
      for (let i = 0; i < 3; i++) await scene.renderOnce();
      const panes = images(scene.renderer.root);
      expect(panes.length).toBe(2);
      for (const pane of panes) {
        expect(pane.width).toBeGreaterThan(4);
        expect(pane.height).toBeGreaterThan(2);
        expect(pane.protocol).toBe("blocks");
      }
      expect(scene.captureCharFrame()).toContain("BEFORE");
      expect(scene.captureCharFrame()).toContain("AFTER");
    }
    expect(scene.renderer.listenerCount("capabilities")).toBe(baseline + 1);
  } finally {
    ui.unmount();
    client.close();
    expect(scene.renderer.listenerCount("capabilities")).toBe(baseline);
    scene.renderer.destroy();
  }
});
