import type { BoxRenderable, CliRenderer, ScrollBoxRenderable } from "@opentui/core";
import { createRoot, flushSync } from "@opentui/react";
import type { BackendClient } from "../transport/backendClient";
import { Shell } from "./components/Shell";
import type { ShellStore } from "./store";

export interface MountedShell {
  readonly root: BoxRenderable | null;
  readonly scroll: ScrollBoxRenderable | null;
  unmount: () => void;
}

export function mountApp(
  renderer: CliRenderer,
  store: ShellStore,
  client: BackendClient,
  quit: () => void = () => renderer.destroy(),
): MountedShell {
  const reactRoot = createRoot(renderer);
  flushSync(() => {
    reactRoot.render(<Shell store={store} client={client} quit={quit} />);
  });
  return {
    get root() {
      return renderer.root.findDescendantById("repositories") as BoxRenderable | null;
    },
    get scroll() {
      return renderer.root.findDescendantById("detail-scroll") as ScrollBoxRenderable | null;
    },
    unmount: () => {
      flushSync(() => reactRoot.unmount());
    },
  };
}
