import {
  CliRenderEvents,
  type ScrollBoxRenderable,
  TextBuffer,
  TextBufferView,
  type WidthMethod,
} from "@opentui/core";
import { useRenderer } from "@opentui/react";
import {
  type ReactNode,
  type RefObject,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";

export interface RowGeometry {
  offsets: number[];
  width: number;
}

// Use the renderer's native word wrapper instead of estimating text.length:
// wide characters, combining marks, tabs and word boundaries change row height.
export function measureDiffRows(
  texts: string[][],
  width: number,
  method: WidthMethod,
): RowGeometry {
  const heights = texts.map(() => 1);
  let maximumWidth = width;
  const columns = texts[0]?.length ?? 1;
  for (let column = 0; column < columns; column += 1) {
    const buffer = TextBuffer.create(method);
    const view = TextBufferView.create(buffer);
    try {
      buffer.setText(texts.map((row) => row[column] ?? "").join("\n"));
      view.setWrapMode(columns === 2 ? "word" : "none");
      view.setWrapWidth(
        columns === 2
          ? Math.max(1, column === 0 ? Math.floor(width / 2) : Math.ceil(width / 2))
          : width,
      );
      if (columns === 1) maximumWidth = Math.max(width, view.lineInfo.lineWidthColsMax);
      const counts = new Array<number>(texts.length).fill(0);
      for (const source of view.getLineSources(0, view.getVirtualLineCount())) {
        if (source < counts.length) counts[source] = (counts[source] ?? 0) + 1;
      }
      for (let index = 0; index < heights.length; index += 1)
        heights[index] = Math.max(heights[index] ?? 1, counts[index] ?? 1);
    } finally {
      view.destroy();
      buffer.destroy();
    }
  }
  const offsets = [0];
  for (const height of heights) offsets.push((offsets.at(-1) ?? 0) + height);
  return { offsets, width: maximumWidth };
}

export function visibleDiffRows(offsets: number[], top: number, height: number, overscan = 8) {
  const count = Math.max(0, offsets.length - 1);
  const lowerBound = (position: number) => {
    let left = 0;
    let right = count;
    while (left < right) {
      const middle = (left + right) >>> 1;
      if ((offsets[middle + 1] ?? 0) <= position) left = middle + 1;
      else right = middle;
    }
    return left;
  };
  const start = Math.max(0, lowerBound(Math.max(0, top)) - overscan);
  const end = Math.min(count, lowerBound(Math.max(0, top) + Math.max(1, height)) + overscan + 1);
  return { start, end };
}

export function VirtualDiff({
  scroll,
  texts,
  anchor,
  request,
  identity,
  above,
  below,
  renderRow,
}: {
  scroll: RefObject<ScrollBoxRenderable | null>;
  texts: string[][];
  anchor: number;
  request: number;
  identity: string;
  above: ReactNode;
  below: ReactNode;
  renderRow: (index: number) => ReactNode;
}) {
  const renderer = useRenderer();
  const metrics = useMemo(() => {
    let value = `0:${Math.max(20, Math.floor(renderer.width / 2))}:${Math.max(1, renderer.height - 12)}`;
    const listeners = new Set<() => void>();
    const update = () => {
      const view = scroll.current;
      if (!view) return;
      const next = `${view.scrollTop}:${Math.max(1, view.viewport.width)}:${Math.max(1, view.viewport.height)}`;
      if (next === value) return;
      value = next;
      for (const listener of listeners) listener();
    };
    return {
      update,
      get: () => value,
      subscribe: (listener: () => void) => {
        listeners.add(listener);
        renderer.on(CliRenderEvents.FRAME, update);
        renderer.on(CliRenderEvents.RESIZE, update);
        return () => {
          listeners.delete(listener);
          renderer.off(CliRenderEvents.FRAME, update);
          renderer.off(CliRenderEvents.RESIZE, update);
        };
      },
    };
  }, [renderer, scroll]);
  const snapshot = useSyncExternalStore(metrics.subscribe, metrics.get, metrics.get);
  const [top = 0, width = 1, height = 1] = snapshot.split(":").map(Number);
  const geometry = useMemo(
    () => measureDiffRows(texts, width, renderer.widthMethod),
    [texts, width, renderer.widthMethod],
  );
  const marker = above ? 1 : 0;
  const window = visibleDiffRows(geometry.offsets, top - marker, height);
  const pending = useRef<number | null>(null);
  const previousIdentity = useRef(identity);
  useEffect(() => {
    if (anchor >= 0 && request >= 0) pending.current = (geometry.offsets[anchor] ?? 0) + marker;
    else if (previousIdentity.current !== identity) pending.current = 0;
    previousIdentity.current = identity;
    renderer.requestRender();
  }, [anchor, request, identity, geometry, marker, renderer]);
  useEffect(() => {
    const update = () => {
      const view = scroll.current;
      const target = pending.current;
      if (!view || target === null || view.viewport.height <= 0) return;
      // Wait for the spacer layout: clamping before content height exists loses
      // an offscreen request. The actual anchor need never be mounted first.
      const extent = (geometry.offsets.at(-1) ?? 0) + marker + (below ? 1 : 0);
      if (view.scrollHeight < extent) return;
      pending.current = null;
      view.scrollTop = Math.max(0, target - Math.floor(view.viewport.height / 2));
      metrics.update();
    };
    renderer.on(CliRenderEvents.FRAME, update);
    return () => {
      renderer.off(CliRenderEvents.FRAME, update);
    };
  }, [geometry, marker, below, renderer, scroll, metrics]);
  const totalHeight = geometry.offsets.at(-1) ?? 0;
  return (
    <scrollbox
      ref={scroll}
      id="detail-scroll"
      flexGrow={1}
      minHeight={0}
      marginTop={1}
      scrollX
      viewportCulling
      contentOptions={{ width: geometry.width, minWidth: geometry.width }}
    >
      {above}
      {window.start > 0 ? (
        <box id="diff-spacer-top" height={geometry.offsets[window.start]} flexShrink={0} />
      ) : null}
      {Array.from({ length: window.end - window.start }, (_, offset) => {
        const index = window.start + offset;
        return (
          <box
            key={index}
            id={`diff-row-${index}`}
            height={(geometry.offsets[index + 1] ?? 0) - (geometry.offsets[index] ?? 0)}
            flexShrink={0}
          >
            {renderRow(index)}
          </box>
        );
      })}
      {window.end < texts.length ? (
        <box
          id="diff-spacer-bottom"
          height={totalHeight - (geometry.offsets[window.end] ?? 0)}
          flexShrink={0}
        />
      ) : null}
      {below}
    </scrollbox>
  );
}
