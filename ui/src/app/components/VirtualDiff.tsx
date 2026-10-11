import { CliRenderEvents, type ScrollBoxRenderable } from "@opentui/core";
import { useRenderer } from "@opentui/react";
import {
  type ReactNode,
  type RefObject,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import { measureDiffRows, visibleDiffRows } from "../diffGeometry";
import type { DiffScrollPositions } from "../diffScrollPositions";

export function VirtualDiff({
  scroll,
  texts,
  anchor,
  request,
  identity,
  positions,
  above,
  below,
  renderRow,
}: {
  scroll: RefObject<ScrollBoxRenderable | null>;
  texts: string[][];
  anchor: number;
  request: number;
  identity: string;
  positions?: DiffScrollPositions;
  above: ReactNode;
  below: ReactNode;
  renderRow: (index: number) => ReactNode;
}) {
  const renderer = useRenderer();
  const pending = useRef<{ top: number; left: number; centered: boolean } | null>(null);
  const previousIdentity = useRef(identity);
  const previousRequest = useRef<number | null>(null);
  const active = useRef(true);
  const ownedView = useRef<ScrollBoxRenderable | null>(null);
  const frameUpdate = useRef<(() => void) | null>(null);
  const metrics = useMemo(() => {
    let value = `0:${Math.max(20, Math.floor(renderer.width / 2))}:${Math.max(1, renderer.height - 12)}`;
    const listeners = new Set<() => void>();
    let listening = false;
    const stop = () => {
      renderer.off(CliRenderEvents.FRAME, update);
      renderer.off(CliRenderEvents.RESIZE, update);
      listening = false;
    };
    const update = () => {
      if (!active.current) return;
      const view = scroll.current;
      if (!view || view !== ownedView.current) return;
      if (pending.current === null)
        positions?.set(previousIdentity.current, { top: view.scrollTop, left: view.scrollLeft });
      const next = `${view.scrollTop}:${Math.max(1, view.viewport.width)}:${Math.max(1, view.viewport.height)}`;
      if (next === value) return;
      value = next;
      for (const listener of listeners) listener();
    };
    return {
      stop,
      update,
      get: () => value,
      subscribe: (listener: () => void) => {
        listeners.add(listener);
        if (!listening) {
          renderer.on(CliRenderEvents.FRAME, update);
          renderer.on(CliRenderEvents.RESIZE, update);
          listening = true;
        }
        return () => {
          listeners.delete(listener);
          if (listeners.size === 0) stop();
        };
      },
    };
  }, [renderer, scroll, positions]);
  const snapshot = useSyncExternalStore(metrics.subscribe, metrics.get, metrics.get);
  const [top = 0, width = 1, height = 1] = snapshot.split(":").map(Number);
  const geometry = useMemo(
    () => measureDiffRows(texts, width, renderer.widthMethod),
    [texts, width, renderer.widthMethod],
  );
  const marker = above ? 1 : 0;
  const window = visibleDiffRows(geometry.offsets, top - marker, height);
  useEffect(() => {
    const changed = previousIdentity.current !== identity;
    if (changed) {
      const view = scroll.current;
      if (view)
        positions?.set(previousIdentity.current, { top: view.scrollTop, left: view.scrollLeft });
    }
    if (anchor >= 0 && request >= 0 && (request !== previousRequest.current || !changed))
      pending.current = { top: (geometry.offsets[anchor] ?? 0) + marker, left: 0, centered: true };
    else if (changed)
      pending.current = { ...(positions?.get(identity) ?? { top: 0, left: 0 }), centered: false };
    else if (previousRequest.current === null && positions?.get(identity))
      pending.current = { ...positions.get(identity)!, centered: false };
    previousIdentity.current = identity;
    previousRequest.current = request;
    renderer.requestRender();
  }, [anchor, request, identity, geometry, marker, renderer, scroll, positions]);
  useLayoutEffect(() => {
    active.current = true;
    ownedView.current = scroll.current;
    return () => {
      active.current = false;
      metrics.stop();
      if (frameUpdate.current) renderer.off(CliRenderEvents.FRAME, frameUpdate.current);
      const view = scroll.current;
      if (view && view === ownedView.current)
        positions?.set(previousIdentity.current, { top: view.scrollTop, left: view.scrollLeft });
    };
  }, [scroll, positions, metrics, renderer]);
  useEffect(() => {
    const update = () => {
      const view = scroll.current;
      const target = pending.current;
      if (
        !active.current ||
        !view ||
        view !== ownedView.current ||
        target === null ||
        view.viewport.height <= 0
      )
        return;
      // Wait for the spacer layout: clamping before content height exists loses
      // an offscreen request. The actual anchor need never be mounted first.
      const extent = (geometry.offsets.at(-1) ?? 0) + marker + (below ? 1 : 0);
      if (view.scrollHeight < extent) return;
      pending.current = null;
      view.scrollTop = Math.max(
        0,
        target.top - (target.centered ? Math.floor(view.viewport.height / 2) : 0),
      );
      view.scrollLeft = target.left;
      metrics.update();
    };
    frameUpdate.current = update;
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
