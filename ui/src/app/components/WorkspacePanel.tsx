import { useKeyboard, usePaste } from "@opentui/react";
import { useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import { theme } from "../theme";
import type { WorkspaceStore } from "../workspaceStore";

export function WorkspacePanel({
  workspace,
  client,
  path,
  onClose,
}: {
  workspace: WorkspaceStore;
  client: BackendClient;
  path: string | undefined;
  onClose: () => void;
}) {
  const state = useSyncExternalStore(workspace.subscribe, workspace.getSnapshot);
  useKeyboard((key) => {
    key.preventDefault();
    key.stopPropagation();
    if (key.name === "escape") onClose();
    else if (key.ctrl && key.name === "s") void workspace.save(client);
    else if (key.ctrl && key.name === "u") workspace.clear();
    else if (key.ctrl && key.name === "d") workspace.discard();
    else if (key.ctrl && key.name === "v") void workspace.toggleViewed(client, path);
    else if (key.name === "backspace") workspace.backspace();
    else if (key.name === "return") workspace.append("\n");
    else if (!key.ctrl && !key.meta && !key.super && !key.option && key.sequence >= " ")
      workspace.append(key.sequence);
  });
  usePaste((event) => {
    event.preventDefault();
    workspace.append(new TextDecoder().decode(event.bytes));
  });
  return (
    <box flexGrow={1} flexDirection="column" paddingX={1} backgroundColor={theme.background}>
      <text fg={theme.primary}>{`Repository prompt · ${state.scope ?? "no selection"}`}</text>
      <text fg={theme.muted}>
        Ctrl+S save · Ctrl+D discard edits · Ctrl+U clear · Esc close/discard
      </text>
      <text fg={theme.muted}>
        A nonblank saved prompt replaces repository policy. Empty uses repository policy.
      </text>
      <text fg={theme.secondary}>
        {state.viewedKey
          ? `Ctrl+V toggle viewed: ${path ?? "no file"} · ${path && state.viewed.includes(path) ? "viewed" : "not viewed"} · ${state.viewed.length} saved marks`
          : "Viewed marks are stored per pull request; this is a working-tree target."}
      </text>
      <text fg={theme.warning}>
        {state.busy
          ? "Waiting for backend…"
          : state.text !== state.saved
            ? "Unsaved edits"
            : "Saved"}
      </text>
      {state.error ? <text fg={theme.error}>{state.error}</text> : null}
      {state.notice ? <text fg={theme.secondary}>{state.notice}</text> : null}
      <scrollbox flexGrow={1} minHeight={0}>
        <text fg={theme.text}>{state.text || "(empty override)"}</text>
      </scrollbox>
    </box>
  );
}
