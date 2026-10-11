import { expect, test } from "bun:test";
import { type Renderable, type ScrollBoxRenderable, TextRenderable } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { createRoot, flushSync } from "@opentui/react";
import { useState } from "react";
import { VirtualDiff } from "../src/app/components/VirtualDiff";
import { measureDiffRows, visibleDiffRows } from "../src/app/diffGeometry";

async function fixture(texts: string[][], anchor = -1, width = 80) {
  const testRenderer = await createTestRenderer({ width, height: 20 });
  const root = createRoot(testRenderer.renderer);
  const scroll: { current: ScrollBoxRenderable | null } = { current: null };
  let updateScene: (state: { anchor: number; request: number }) => void = () => {
    throw new Error("Scene is not mounted");
  };
  function Scene() {
    const [state, setState] = useState({ anchor, request: 0 });
    updateScene = setState;
    const nextAnchor = state.anchor;
    const request = state.request;
    return (
      <box width="100%" height="100%">
        <VirtualDiff
          scroll={scroll}
          texts={texts}
          anchor={nextAnchor}
          request={request}
          identity="test-file"
          above={null}
          below={null}
          renderRow={(index) =>
            texts[index]?.length === 2 ? (
              <box flexDirection="row">
                <box width="50%">
                  <text wrapMode="word">{texts[index]?.[0]}</text>
                </box>
                <box width="50%">
                  <text wrapMode="word">{texts[index]?.[1]}</text>
                </box>
              </box>
            ) : (
              <text wrapMode="none">{texts[index]?.[0]}</text>
            )
          }
        />
      </box>
    );
  }
  const draw = (anchor: number, request: number) =>
    flushSync(() => updateScene({ anchor, request }));
  flushSync(() => root.render(<Scene />));
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await testRenderer.renderOnce();
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  };
  await settle();
  return {
    ...testRenderer,
    scroll,
    draw,
    settle,
    close: () => {
      flushSync(() => root.unmount());
      testRenderer.renderer.destroy();
    },
  };
}

test("expanded 10,000 rows stay bounded while scroll reaches either end", async () => {
  const scene = await fixture(Array.from({ length: 10000 }, (_, i) => [`ROW-${i}`]));
  try {
    expect(
      scene.scroll.current?.getChildren().filter((row) => row.id.startsWith("diff-row-")).length,
    ).toBeLessThan(50);
    expect(scene.captureCharFrame()).toContain("ROW-0");
    scene.scroll.current!.scrollTop = 9999;
    await scene.settle();
    expect(scene.captureCharFrame()).toContain("ROW-9999");
    expect(
      scene.scroll.current?.getChildren().filter((row) => row.id.startsWith("diff-row-")).length,
    ).toBeLessThan(50);
    scene.scroll.current!.scrollTop = 0;
    await scene.settle();
    expect(scene.captureCharFrame()).toContain("ROW-0");
    expect(scene.captureCharFrame()).not.toContain("ROW-9999");
  } finally {
    scene.close();
  }
});

test("offscreen split anchor uses native wrapped Unicode/tab geometry and survives resize", async () => {
  const texts = Array.from({ length: 600 }, (_, i) => [
    `${i} OLD 界 é\t words `.repeat(3),
    `${i} NEW 🧪 words `.repeat(2),
  ]);
  texts[550] = ["ANCHOR-OLD", "ANCHOR-NEW"];
  const scene = await fixture(texts, 550, 81);
  try {
    expect(scene.captureCharFrame()).toContain("ANCHOR-OLD");
    expect(scene.captureCharFrame()).toContain("ANCHOR-NEW");
    const verifyWrappedHeights = () => {
      for (const row of scene.scroll
        .current!.getChildren()
        .filter((row) => row.id.startsWith("diff-row-"))) {
        const walk = (child: Renderable) => {
          if (child instanceof TextRenderable) expect(child.height).toBeLessThanOrEqual(row.height);
          for (const descendant of child.getChildren()) walk(descendant);
        };
        walk(row);
      }
    };
    verifyWrappedHeights();
    const geometry = measureDiffRows(
      texts,
      scene.scroll.current!.viewport.width,
      scene.renderer.widthMethod,
    );
    expect(scene.scroll.current!.scrollHeight).toBe(geometry.offsets.at(-1)!);
    scene.resize(53, 20);
    await scene.settle();
    expect(scene.captureCharFrame()).toContain("ANCHOR-OLD");
    verifyWrappedHeights();
    const narrow = measureDiffRows(
      texts,
      scene.scroll.current!.viewport.width,
      scene.renderer.widthMethod,
    );
    expect(scene.scroll.current!.scrollHeight).toBe(narrow.offsets.at(-1)!);
    expect(narrow.offsets[550]).toBeGreaterThan(geometry.offsets[550]!);
    scene.draw(10, 1);
    await scene.settle();
    expect(scene.scroll.current!.scrollTop).toBe(
      Math.max(0, narrow.offsets[10]! - Math.floor(scene.scroll.current!.viewport.height / 2)),
    );
  } finally {
    scene.close();
  }
});

test("unified offscreen width preserves horizontal extent and virtual window boundaries", async () => {
  const texts = Array.from({ length: 1000 }, (_, i) => [`LINE ${i}`]);
  texts[999] = ["W".repeat(250)];
  const scene = await fixture(texts, 0);
  try {
    expect(scene.scroll.current!.scrollWidth).toBeGreaterThanOrEqual(250);
    scene.scroll.current!.scrollLeft = 100;
    await scene.settle();
    expect(scene.scroll.current!.scrollLeft).toBe(100);
    const geometry = measureDiffRows(texts, 40, "wcwidth");
    const window = visibleDiffRows(geometry.offsets, 500, 20);
    expect(window).toEqual({ start: 492, end: 529 });
  } finally {
    scene.close();
  }
});

test("native diff restores file/mode positions across component removal and fences changed content", async () => {
  const { DiffScrollPositions } = await import("../src/app/diffScrollPositions");
  const positions = new DiffScrollPositions();
  const testRenderer = await createTestRenderer({ width: 80, height: 20 });
  const root = createRoot(testRenderer.renderer);
  const scroll: { current: ScrollBoxRenderable | null } = { current: null };
  const texts = Array.from({ length: 1000 }, (_, i) => [`ROW-${i} ${"x".repeat(150)}`]);
  let updateScene: (state: { identity: string | null; anchor: number; request: number }) => void =
    () => {
      throw new Error("Scene is not mounted");
    };
  function Scene() {
    const [state, setState] = useState<{
      identity: string | null;
      anchor: number;
      request: number;
    }>({ identity: null, anchor: -1, request: 0 });
    updateScene = setState;
    const { identity, anchor, request } = state;
    return (
      <box width="100%" height="100%">
        {identity ? (
          <VirtualDiff
            scroll={scroll}
            texts={texts}
            anchor={anchor}
            request={request}
            identity={identity}
            positions={positions}
            above={null}
            below={null}
            renderRow={(index) => <text wrapMode="none">{texts[index]?.[0]}</text>}
          />
        ) : (
          <text>Loading another file…</text>
        )}
      </box>
    );
  }
  const draw = (identity: string | null, anchor = -1, request = 0) =>
    flushSync(() => updateScene({ identity, anchor, request }));
  flushSync(() => root.render(<Scene />));
  const settle = async () => {
    for (let i = 0; i < 5; i++) {
      await testRenderer.renderOnce();
      await Bun.sleep(0);
    }
  };
  try {
    draw("run:file-a:hash:unified");
    await settle();
    const initialFrameListeners = testRenderer.renderer.listenerCount("frame");
    scroll.current!.scrollTop = 222;
    scroll.current!.scrollLeft = 33;
    await settle();
    expect(positions.get("run:file-a:hash:unified")).toEqual({ top: 222, left: 33 });
    draw(null);
    await settle();
    expect(positions.get("run:file-a:hash:unified")).toEqual({ top: 222, left: 33 });
    draw("run:file-b:hash:unified");
    await settle();
    expect(positions.get("run:file-a:hash:unified")).toEqual({ top: 222, left: 33 });
    expect(scroll.current!.scrollTop).toBe(0);
    scroll.current!.scrollTop = 444;
    await settle();
    expect(positions.get("run:file-a:hash:unified")).toEqual({ top: 222, left: 33 });
    draw("run:file-a:hash:unified");
    await settle();
    expect(scroll.current!.scrollTop).toBe(222);
    expect(scroll.current!.scrollLeft).toBe(33);
    draw("run:file-a:hash:split");
    await settle();
    expect(scroll.current!.scrollTop).toBe(0);
    scroll.current!.scrollTop = 555;
    await settle();
    draw("run:file-a:hash:unified");
    await settle();
    expect(scroll.current!.scrollTop).toBe(222);
    draw("run:file-a:new-hash:unified");
    await settle();
    expect(scroll.current!.scrollTop).toBe(0);
    draw("run:file-a:hash:unified", 800, 1);
    await settle();
    expect(scroll.current!.scrollTop).toBeGreaterThan(750);
    expect(testRenderer.captureCharFrame()).toContain("ROW-800");
    draw("other-run:file-a:hash:unified");
    await settle();
    expect(scroll.current!.scrollTop).toBe(0);
    expect(testRenderer.renderer.listenerCount("frame")).toBeLessThanOrEqual(initialFrameListeners);
  } finally {
    flushSync(() => root.unmount());
    testRenderer.renderer.destroy();
  }
});
