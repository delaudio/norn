import { CliRenderEvents, type ScrollBoxRenderable } from "@opentui/core";
import { useRenderer } from "@opentui/react";
import {
  type ReactNode,
  type RefObject,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import { measureDiffRows, visibleDiffRows } from "../diffGeometry";

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
