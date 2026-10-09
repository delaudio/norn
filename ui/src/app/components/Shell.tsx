import { type ScrollBoxRenderable, TextAttributes } from "@opentui/core";
import { useKeyboard, usePaste, useTerminalDimensions } from "@opentui/react";
import { type RefObject, useRef, useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import type { ShellSnapshot, ShellStore } from "../store";
import { theme } from "../theme";

interface ShellProps {
  store: ShellStore;
  client: BackendClient;
  quit: () => void;
}

function useShellInput(
  store: ShellStore,
  client: BackendClient,
  quit: () => void,
  scroll: RefObject<ScrollBoxRenderable | null>,
) {
  useKeyboard((key) => {
    if (key.ctrl && key.name === "c") {
      quit();
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) {
      return;
    }
    key.preventDefault();
    if (key.name === "q") {
      quit();
    } else if (key.name === "tab") {
      store.toggleFocus();
    } else if (key.name === "down" || key.name === "j" || key.name === "up" || key.name === "k") {
      const delta = key.name === "down" || key.name === "j" ? 1 : -1;
      if (store.getSnapshot().focus === "detail" && scroll.current) {
        scroll.current.scrollBy(delta);
      } else {
        store.move(delta);
      }
    } else if (key.name === "pagedown" || key.name === "pageup") {
      const view = scroll.current;
      if (view) {
        view.scrollBy((key.name === "pagedown" ? 1 : -1) * Math.max(1, view.height - 2));
      }
    } else if (key.name === "return") {
      void store.startReview(client);
    } else if (key.name === "x") {
      void store.cancel(client);
    }
  });
  // No text editor exists in this slice, so paste is intentionally inert.
  usePaste(() => {});
}

function DetailContent({
  state,
  scroll,
}: {
  state: ShellSnapshot;
  scroll: RefObject<ScrollBoxRenderable | null>;
}) {
  if (state.status === "connecting") {
    return <text fg={theme.muted}>Connecting to the Norn backend…</text>;
  }
  if (state.status === "error") {
    return <text fg={theme.error}>Backend error: {state.error ?? "unknown"}</text>;
  }
  if (state.status === "closed") {
    return <text fg={theme.warning}>Backend connection closed. Press q to quit.</text>;
  }
  const repo = state.repositories[state.selected];
  if (!repo) {
    return <text fg={theme.muted}>No repositories configured. Run `norn setup` to add one.</text>;
  }
  const operation = state.operation;
  return (
    <>
      <text fg={theme.primary} attributes={TextAttributes.BOLD} height={1} wrapMode="none" truncate>
        {repo.workspace}/{repo.repo}
      </text>
      <text fg={theme.muted} height={1} wrapMode="none">
        {repo.provider} · {repo.localPath ?? "no local clone"}
      </text>
      {operation ? (
        <text fg={theme.secondary} marginTop={1} height={1} wrapMode="none">
          review {operation.id} · {operation.state} · seq {operation.sequence}
        </text>
      ) : (
        <text fg={theme.muted} marginTop={1} height={1} wrapMode="none">
          Press Enter to start a review, x to cancel.
        </text>
      )}
      <scrollbox
        ref={scroll}
        id="detail-scroll"
        flexGrow={1}
        minHeight={0}
        marginTop={1}
        scrollX={false}
      >
        {(operation?.logs ?? []).map((entry) => (
          <text key={entry.id} fg={theme.text} wrapMode="word">
            {entry.text}
          </text>
        ))}
      </scrollbox>
    </>
  );
}

function Detail({
  state,
  scroll,
}: {
  state: ShellSnapshot;
  scroll: RefObject<ScrollBoxRenderable | null>;
}) {
  return (
    <>
      {state.error ? (
        <text fg={theme.error} height={1} wrapMode="none">
          Error: {state.error}
        </text>
      ) : null}
      <DetailContent state={state} scroll={scroll} />
    </>
  );
}

export function Shell({ store, client, quit }: ShellProps) {
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  const { width } = useTerminalDimensions();
  useShellInput(store, client, quit, scroll);
  const wide = width >= 80;

  return (
    <box width="100%" height="100%" flexDirection="column" backgroundColor={theme.background}>
      <box
        height={3}
        flexShrink={0}
        paddingX={2}
        justifyContent="center"
        backgroundColor={theme.panel}
      >
        <text fg={theme.primary}>
          Norn / OpenTUI workspace{state.serverVersion ? `  ·  backend ${state.serverVersion}` : ""}
        </text>
      </box>
      <box flexGrow={1} minHeight={0} flexDirection="row">
        {wide ? (
          <box
            id="repositories"
            width="30%"
            flexShrink={0}
            minWidth={0}
            paddingX={1}
            paddingTop={1}
            backgroundColor={theme.panel}
            border={["right"]}
            borderColor={theme.border}
          >
            <text
              id="repositories-label"
              fg={state.focus === "repositories" ? theme.primary : theme.muted}
            >
              REPOSITORIES
            </text>
            <select
              id="repositories-list"
              flexGrow={1}
              minHeight={0}
              marginTop={1}
              options={state.repositories.map((repo) => ({
                name: `${repo.workspace}/${repo.repo}`,
                description: repo.provider,
              }))}
              selectedIndex={state.selected}
              showDescription={false}
              showSelectionIndicator
              backgroundColor={theme.panel}
              textColor={theme.text}
              selectedBackgroundColor={theme.element}
              selectedTextColor={theme.primary}
            />
          </box>
        ) : null}
        <box
          id="detail"
          flexGrow={1}
          minWidth={0}
          paddingX={2}
          paddingTop={1}
          flexDirection="column"
        >
          <Detail state={state} scroll={scroll} />
        </box>
      </box>
      <box height={1} flexShrink={0} paddingX={1} backgroundColor={theme.panel}>
        <text fg={theme.muted}>
          {wide ? "j/k move  Tab pane  Enter review  x cancel  q quit" : "j/k  Tab  Enter  x  q"}
        </text>
      </box>
    </box>
  );
}
