import { useKeyboard, usePaste } from "@opentui/react";
import { useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import type { DataStore } from "../dataStore";
import { theme } from "../theme";
export function DataPanel({
  data,
  client,
  onClose,
}: {
  data: DataStore;
  client: BackendClient;
  onClose: () => void;
}) {
  const state = useSyncExternalStore(data.subscribe, data.getSnapshot);
  useKeyboard((key) => {
    key.preventDefault();
    key.stopPropagation();
    if (state.editing) {
      if (key.name === "escape") data.cancelEdit();
      else if (key.name === "return") data.finishEdit();
      else if (key.name === "backspace") data.backspace();
      else if (key.ctrl && key.name === "u") data.clearEdit();
      else if (!key.ctrl && !key.meta && !["tab", "up", "down", "left", "right"].includes(key.name))
        data.append(key.sequence);
      return;
    }
    if (key.name === "escape" || key.name === "q") onClose();
    else if (key.name === "tab" || key.name === "j" || key.name === "down") data.move(1);
    else if (key.name === "k" || key.name === "up") data.move(-1);
    else if (key.name === "return") data.beginEdit();
    else if (key.ctrl && key.name === "p") void data.preview(client);
    else if (key.ctrl && key.name === "y") void data.confirm(client);
    else if (key.ctrl && key.name === "e") void data.export(client);
  });
  usePaste((event) => {
    event.preventDefault();
    if (state.editing) data.append(new TextDecoder().decode(event.bytes));
  });
  const fields = [
    `Import file: ${state.path || "(enter exported JSON path)"}`,
    `Historical provider: ${state.provider || "unset — ambiguous entries reported"}`,
    `New export file: ${state.exportPath || "(choose a file that does not exist)"}`,
  ];
  return (
    <box flexGrow={1} flexDirection="column" paddingX={1} backgroundColor={theme.background}>
      <text fg={theme.primary} height={1} truncate>
        User data · Enter edit/cycle · Tab navigate · Esc close
      </text>
      <text fg={theme.muted} height={1} truncate>
        Ctrl+P preview · Ctrl+Y confirm import · Ctrl+E export
      </text>
      <text fg={theme.muted}>
        Source files remain intact. Existing conflicting scopes are kept. Changed destinations
        receive a backup.
      </text>
      <scrollbox flexGrow={1} minHeight={0}>
        {fields.map((field, index) => (
          <text
            key={field.split(":")[0]}
            fg={state.selected === index ? theme.primary : theme.text}
          >{`${state.selected === index ? "›" : " "} ${state.editing && state.selected === index ? state.editText : field}`}</text>
        ))}
        {state.busy ? <text fg={theme.warning}>Waiting for backend confirmation…</text> : null}
        {state.error ? <text fg={theme.error}>{state.error}</text> : null}
        {state.notice ? <text fg={theme.secondary}>{state.notice}</text> : null}
        {state.preview ? (
          <>
            <text
              fg={theme.primary}
            >{`${state.preview.additionsCount} additions · ${state.preview.unchangedCount} unchanged · ${state.preview.conflictsCount} conflicts kept · ${state.preview.rejectedCount} rejected`}</text>
            {state.preview.detailsTruncated ? (
              <text fg={theme.warning}>
                Detail lists are bounded to 200 entries; counts include the entire export.
              </text>
            ) : null}
            {state.preview.additions.map((key) => (
              <text key={`add-${key}`} fg={theme.secondary}>{`Add: ${key}`}</text>
            ))}
            {state.preview.conflicts.map((key) => (
              <text key={`conflict-${key}`} fg={theme.warning}>{`Keep existing: ${key}`}</text>
            ))}
            {state.preview.rejected.map((item) => (
              <text
                key={`reject-${item.entry}`}
                fg={theme.error}
              >{`Reject entry ${item.entry + 1}: ${item.reason}`}</text>
            ))}
          </>
        ) : null}
      </scrollbox>
      <text height={1} truncate fg={theme.muted}>
        {state.editing
          ? "Ctrl+U clear · Enter keep path · Esc undo field"
          : "Desktop: Settings → Export local data. CLI alternative: norn data --help"}
      </text>
    </box>
  );
}
