import { type ScrollBoxRenderable, TextAttributes } from "@opentui/core";
import { useKeyboard, usePaste, useTerminalDimensions } from "@opentui/react";
import { type RefObject, useEffect, useRef, useSyncExternalStore } from "react";
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
      store.cycleFocus();
    } else if (key.name === "down" || key.name === "j" || key.name === "up" || key.name === "k") {
      const delta = key.name === "down" || key.name === "j" ? 1 : -1;
      if (store.getSnapshot().focus === "diff") {
        scroll.current?.scrollBy(delta);
      } else {
        store.move(delta);
      }
    } else if (key.name === "left" || key.name === "h") {
      scroll.current?.scrollBy({ x: -4, y: 0 });
    } else if (key.name === "right" || key.name === "l") {
      scroll.current?.scrollBy({ x: 4, y: 0 });
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
  usePaste(() => {});
}

interface DiffLine {
  text: string;
  fg: string;
  highlighted: boolean;
}

/// Render a unified diff while tracking both old- and new-side line numbers so a
/// finding anchored to either side highlights the correct row.
export function diffLines(text: string, highlight: number | null, side: "old" | "new"): DiffLine[] {
  const lines: DiffLine[] = [];
  let oldLine = 0;
  let newLine = 0;
  for (const raw of text.split("\n")) {
    let fg: string = theme.text;
    let highlighted = false;
    const hunk = raw.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (hunk) {
      fg = theme.muted;
      oldLine = Number.parseInt(hunk[1] ?? "0", 10);
      newLine = Number.parseInt(hunk[2] ?? "0", 10);
    } else if (raw.startsWith("+")) {
      fg = theme.secondary;
      highlighted = highlight !== null && side === "new" && newLine === highlight;
      newLine += 1;
    } else if (raw.startsWith("-")) {
      fg = theme.error;
      highlighted = highlight !== null && side === "old" && oldLine === highlight;
      oldLine += 1;
    } else if (raw.startsWith(" ")) {
      highlighted =
        highlight !== null &&
        ((side === "new" && newLine === highlight) || (side === "old" && oldLine === highlight));
      oldLine += 1;
      newLine += 1;
    }
    lines.push({ text: raw, fg, highlighted });
  }
  return lines;
}

function DiffPanel({
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
  if (state.repositories.length === 0) {
    return <text fg={theme.muted}>No repositories configured. Run `norn setup` to add one.</text>;
  }
  if (state.diffLoading) {
    return <text fg={theme.muted}>Loading diff…</text>;
  }
  if (!state.diff) {
    return (
      <text fg={theme.muted}>
        {state.files.length === 0 ? "No changed files." : "Select a file to view its diff."}
      </text>
    );
  }
  const lines = diffLines(state.diff.text, state.highlightLine, state.highlightSide);
  return (
    <>
      <text fg={theme.primary} attributes={TextAttributes.BOLD} height={1} wrapMode="none" truncate>
        {state.diff.path}
        {state.diff.truncated ? " (truncated)" : ""}
      </text>
      <scrollbox ref={scroll} id="detail-scroll" flexGrow={1} minHeight={0} marginTop={1} scrollX>
        {lines.map((line, index) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: diff lines are immutable and position-keyed
          <text key={index} fg={line.highlighted ? theme.primary : line.fg} wrapMode="none">
            {line.text}
          </text>
        ))}
      </scrollbox>
    </>
  );
}

interface ListOption {
  name: string;
  description: string;
}

function ListPanel({
  id,
  title,
  focused,
  options,
  selectedIndex,
  showDescription = false,
  showSelectionIndicator = true,
}: {
  id: string;
  title: string;
  focused: boolean;
  options: ListOption[];
  selectedIndex: number;
  showDescription?: boolean;
  showSelectionIndicator?: boolean;
}) {
  return (
    <>
      <text fg={focused ? theme.primary : theme.muted}>{title}</text>
      <select
        id={id}
        flexGrow={1}
        minHeight={0}
        options={options}
        selectedIndex={selectedIndex}
        showDescription={showDescription}
        showSelectionIndicator={showSelectionIndicator}
        backgroundColor={theme.panel}
        textColor={theme.text}
        selectedBackgroundColor={theme.element}
        selectedTextColor={theme.primary}
      />
    </>
  );
}

function pickerOptions(state: ShellSnapshot): ListOption[] {
  return state.picker.map((entry) => ({
    name:
      entry.kind === "review"
        ? `${entry.workspace}/${entry.repo} #${entry.prId} · ${entry.status}`
        : `${entry.workspace}/${entry.repo}`,
    description: entry.kind === "review" ? `${entry.title} · ${entry.runId}` : entry.title,
  }));
}

function fileOptions(state: ShellSnapshot): ListOption[] {
  return state.files.map((file) => ({
    name: `${file.status[0]?.toUpperCase() ?? "?"} ${file.path}`,
    description: `+${file.additions} -${file.deletions}`,
  }));
}

function findingOptions(state: ShellSnapshot): ListOption[] {
  return state.findings.length === 0
    ? [{ name: "No findings", description: "" }]
    : state.findings.map((finding) => ({
        name: `[${finding.severity}] ${finding.title}`,
        description: finding.anchor?.path ?? "",
      }));
}

export function Shell({ store, client, quit }: ShellProps) {
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  const { width } = useTerminalDimensions();
  useShellInput(store, client, quit, scroll);
  const wide = width >= 100;

  useEffect(() => {
    if (state.scrollRequest === 0 || !state.diff) {
      return;
    }
    const index = diffLines(state.diff.text, state.highlightLine, state.highlightSide).findIndex(
      (line) => line.highlighted,
    );
    if (index >= 0 && scroll.current) {
      scroll.current.scrollTop = index;
    }
  }, [state.scrollRequest, state.diff, state.highlightLine, state.highlightSide]);

  const repo = state.repositories[state.selected];
  const selectedFinding = state.findings[state.selectedFinding];
  const entry = state.picker[state.selectedPicker];
  const detailTitle = entry
    ? entry.kind === "review"
      ? `${entry.workspace}/${entry.repo} #${entry.prId}`
      : `${entry.workspace}/${entry.repo}`
    : repo
      ? `${repo.workspace}/${repo.repo}`
      : "Norn";
  const compactPanel =
    state.focus === "repositories"
      ? {
          id: "repositories-list",
          title: "TARGETS",
          options: pickerOptions(state),
          selectedIndex: state.selectedPicker,
        }
      : state.focus === "files"
        ? {
            id: "files-list",
            title: "FILES",
            options: fileOptions(state),
            selectedIndex: state.selectedFile,
          }
        : {
            id: "findings-list",
            title: "FINDINGS",
            options: findingOptions(state),
            selectedIndex: state.selectedFinding,
          };

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
      {state.error ? (
        <text fg={theme.error} height={1} wrapMode="none">
          Error: {state.error}
        </text>
      ) : null}
      {state.diffError ? (
        <text fg={theme.error} height={1} wrapMode="none">
          Diff error: {state.diffError}
        </text>
      ) : null}
      {state.notice ? (
        <text fg={theme.warning} height={1} wrapMode="none">
          {state.notice}
        </text>
      ) : null}
      {state.operation ? (
        <text fg={theme.secondary} height={1} wrapMode="none" truncate>
          review {state.operation.id} · {state.operation.state}
          {state.operation.logs.length > 0
            ? ` · ${state.operation.logs[state.operation.logs.length - 1]?.text ?? ""}`
            : ""}
        </text>
      ) : null}
      {selectedFinding ? (
        <text fg={theme.text} height={1} wrapMode="none" truncate>
          {`[${selectedFinding.severity}] ${selectedFinding.title} — ${selectedFinding.summary}`}
        </text>
      ) : null}
      <box flexGrow={1} minHeight={0} flexDirection="row">
        {wide ? (
          <>
            <box
              id="repositories"
              width="28%"
              flexShrink={0}
              minWidth={0}
              paddingX={1}
              paddingTop={1}
              backgroundColor={theme.panel}
              border={["right"]}
              borderColor={theme.border}
            >
              <ListPanel
                id="repositories-list"
                title="TARGETS"
                focused={state.focus === "repositories"}
                options={pickerOptions(state)}
                selectedIndex={state.selectedPicker}
                showDescription
              />
            </box>
            <box
              id="files"
              width="28%"
              flexShrink={0}
              minWidth={0}
              paddingX={1}
              paddingTop={1}
              backgroundColor={theme.panel}
              border={["right"]}
              borderColor={theme.border}
            >
              <ListPanel
                id="files-list"
                title="FILES"
                focused={state.focus === "files"}
                options={fileOptions(state)}
                selectedIndex={state.selectedFile}
              />
              <ListPanel
                id="findings-list"
                title="FINDINGS"
                focused={state.focus === "findings"}
                options={findingOptions(state)}
                selectedIndex={state.selectedFinding}
              />
            </box>
          </>
        ) : (
          <box
            id="focused-list"
            width="42%"
            flexShrink={0}
            minWidth={0}
            paddingX={1}
            paddingTop={1}
            backgroundColor={theme.panel}
            border={["right"]}
            borderColor={theme.border}
          >
            <ListPanel
              id={compactPanel.id}
              title={compactPanel.title}
              focused
              options={compactPanel.options}
              selectedIndex={compactPanel.selectedIndex}
              showDescription={state.focus === "repositories"}
            />
          </box>
        )}
        <box
          id="detail"
          flexGrow={1}
          minWidth={0}
          paddingX={2}
          paddingTop={1}
          flexDirection="column"
        >
          <text
            fg={theme.primary}
            attributes={TextAttributes.BOLD}
            height={1}
            wrapMode="none"
            truncate
          >
            {detailTitle}
          </text>
          <DiffPanel state={state} scroll={scroll} />
        </box>
      </box>
      <box height={1} flexShrink={0} paddingX={1} backgroundColor={theme.panel}>
        <text fg={theme.muted}>
          {wide
            ? "j/k move  ←/→ scroll  Tab pane  Enter review  x cancel  q quit"
            : "j/k  ←/→  Tab  q"}
        </text>
      </box>
    </box>
  );
}
