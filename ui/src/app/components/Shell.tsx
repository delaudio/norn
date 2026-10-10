import { type ScrollBoxRenderable, TextAttributes } from "@opentui/core";
import { useKeyboard, usePaste, useTerminalDimensions } from "@opentui/react";
import {
  type ReactNode,
  type RefObject,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import type { BackendClient } from "../../transport/backendClient";
import { type FilePreviewView, isImagePath, type ShellSnapshot, type ShellStore } from "../store";
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
    } else if (key.name === "s") {
      store.toggleDiffMode();
    } else if (key.name === "e") {
      store.toggleDiffExpanded();
    } else if (key.name === "b") {
      void store.openBrowser(client);
    } else if (key.name === "x") {
      void store.cancel(client);
    }
  });
  usePaste(() => {});
}

export type DiffLineKind = "hunk" | "context" | "add" | "del" | "meta";

interface DiffLine {
  text: string;
  fg: string;
  highlighted: boolean;
  kind: DiffLineKind;
  oldNumber: number | null;
  newNumber: number | null;
}

/// Render a unified diff while tracking both old- and new-side line numbers so a
/// finding anchored to either side highlights the correct row and so the split
/// view can place each line in the right column.
export function diffLines(text: string, highlight: number | null, side: "old" | "new"): DiffLine[] {
  const lines: DiffLine[] = [];
  let oldLine = 0;
  let newLine = 0;
  let inHunk = false;
  for (const raw of text.split("\n")) {
    let fg: string = theme.text;
    let highlighted = false;
    let kind: DiffLineKind = "context";
    let oldNumber: number | null = null;
    let newNumber: number | null = null;
    const hunk = raw.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (hunk) {
      fg = theme.muted;
      kind = "hunk";
      inHunk = true;
      oldLine = Number.parseInt(hunk[1] ?? "0", 10);
      newLine = Number.parseInt(hunk[2] ?? "0", 10);
    } else if (!inHunk) {
      // File headers and other pre-hunk metadata are not source lines.
      fg = theme.muted;
      kind = "meta";
    } else if (raw.startsWith("\\")) {
      fg = theme.muted;
      kind = "meta";
    } else if (raw.startsWith("+")) {
      fg = theme.secondary;
      kind = "add";
      newNumber = newLine;
      highlighted = highlight !== null && side === "new" && newLine === highlight;
      newLine += 1;
    } else if (raw.startsWith("-")) {
      fg = theme.error;
      kind = "del";
      oldNumber = oldLine;
      highlighted = highlight !== null && side === "old" && oldLine === highlight;
      oldLine += 1;
    } else if (raw.startsWith(" ")) {
      kind = "context";
      oldNumber = oldLine;
      newNumber = newLine;
      highlighted =
        highlight !== null &&
        ((side === "new" && newLine === highlight) || (side === "old" && oldLine === highlight));
      oldLine += 1;
      newLine += 1;
    }
    lines.push({ text: raw, fg, highlighted, kind, oldNumber, newNumber });
  }
  return lines;
}

type SplitRow =
  | { kind: "hunk"; text: string }
  | { kind: "note"; text: string; side: "old" | "new" | null }
  | { kind: "pair"; left: DiffLine | null; right: DiffLine | null };

/// Pair removed and added runs side by side so the split view keeps old/new line
/// mapping and finding anchors intact. Newline annotations are attached after
/// their run instead of breaking replacement pairing.
export function splitRows(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    if (!line) {
      break;
    }
    if (line.kind === "hunk") {
      rows.push({ kind: "hunk", text: line.text });
      index += 1;
      continue;
    }
    if (line.kind === "del" || line.kind === "add") {
      const removed: DiffLine[] = [];
      const added: DiffLine[] = [];
      const notes: Array<{ text: string; side: "old" | "new" | null }> = [];
      let lastSide: "old" | "new" | null = null;
      while (index < lines.length) {
        const current = lines[index];
        if (!current) {
          break;
        }
        if (current.kind === "del") {
          removed.push(current);
          lastSide = "old";
        } else if (current.kind === "add") {
          added.push(current);
          lastSide = "new";
        } else if (current.kind === "meta") {
          notes.push({ text: current.text, side: lastSide });
        } else {
          break;
        }
        index += 1;
      }
      const height = Math.max(removed.length, added.length);
      for (let offset = 0; offset < height; offset += 1) {
        rows.push({ kind: "pair", left: removed[offset] ?? null, right: added[offset] ?? null });
      }
      for (const note of notes) {
        rows.push({ kind: "note", text: note.text, side: note.side });
      }
      continue;
    }
    if (line.kind === "meta") {
      rows.push({ kind: "note", text: line.text, side: null });
      index += 1;
      continue;
    }
    rows.push({ kind: "pair", left: line, right: line });
    index += 1;
  }
  return rows;
}

function lineLabel(line: DiffLine | null, side: "old" | "new"): string {
  if (!line) {
    return "";
  }
  const number = side === "old" ? line.oldNumber : line.newNumber;
  return `${String(number ?? "").padStart(4, " ")} ${line.text}`;
}

function splitRowElement(row: SplitRow, key: number): ReactNode {
  if (row.kind === "hunk") {
    return (
      <text key={key} fg={theme.muted} wrapMode="none">
        {row.text}
      </text>
    );
  }
  if (row.kind === "note") {
    if (row.side === null) {
      return (
        <text key={key} fg={theme.muted} wrapMode="none">
          {row.text}
        </text>
      );
    }
    return (
      <box key={key} flexDirection="row" height={1}>
        <box width="50%" minWidth={0}>
          <text fg={theme.muted} wrapMode="none" truncate>
            {row.side === "old" ? row.text : ""}
          </text>
        </box>
        <box width="50%" minWidth={0}>
          <text fg={theme.muted} wrapMode="none" truncate>
            {row.side === "new" ? row.text : ""}
          </text>
        </box>
      </box>
    );
  }
  const highlighted = row.left?.highlighted === true || row.right?.highlighted === true;
  return (
    <box key={key} id={highlighted ? "diff-highlight" : undefined} flexDirection="row">
      <box width="50%" minWidth={0}>
        <text
          fg={row.left?.highlighted ? theme.primary : (row.left?.fg ?? theme.text)}
          wrapMode="word"
        >
          {lineLabel(row.left, "old")}
        </text>
      </box>
      <box width="50%" minWidth={0}>
        <text
          fg={row.right?.highlighted ? theme.primary : (row.right?.fg ?? theme.text)}
          wrapMode="word"
        >
          {lineLabel(row.right, "new")}
        </text>
      </box>
    </box>
  );
}

/// Diff rows rendered at once before collapsing; bounds React mounts on very
/// large diffs while keeping the highlighted region visible.
const MAX_RENDERED_DIFF_ROWS = 2000;

function splitHighlightIndex(rows: SplitRow[]): number {
  return rows.findIndex(
    (row) =>
      row.kind === "pair" && (row.left?.highlighted === true || row.right?.highlighted === true),
  );
}

/// The row window to render, centred on the anchor when a large diff is
/// collapsed. Split out so it can be unit tested.
export function diffWindow(
  total: number,
  anchorIndex: number,
  expanded: boolean,
  max: number = MAX_RENDERED_DIFF_ROWS,
): { start: number; end: number; collapsed: boolean } {
  if (expanded || total <= max) {
    return { start: 0, end: total, collapsed: false };
  }
  const anchor = anchorIndex >= 0 ? anchorIndex : 0;
  const start = Math.max(0, Math.min(anchor - Math.floor(max / 2), total - max));
  return { start, end: start + max, collapsed: true };
}

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function DiffImage({
  preview,
  onError,
}: {
  preview: FilePreviewView;
  onError: (message: string) => void;
}) {
  const bytes = useMemo(() => decodeBase64(preview.dataBase64), [preview.dataBase64]);
  return (
    <image
      source={bytes}
      fit="fit"
      onError={() => onError("this terminal does not support inline images")}
    />
  );
}

function DiffPanel({
  state,
  scroll,
  lines,
  splitAll,
  windowStart,
  windowEnd,
  collapsed,
  total,
  onPreviewError,
}: {
  state: ShellSnapshot;
  scroll: RefObject<ScrollBoxRenderable | null>;
  lines: DiffLine[];
  onPreviewError: (message: string) => void;
  splitAll: SplitRow[];
  windowStart: number;
  windowEnd: number;
  collapsed: boolean;
  total: number;
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
  const previewFile = state.files[state.selectedFile];
  if (previewFile && isImagePath(previewFile.path)) {
    const preview = state.preview?.path === previewFile.path ? state.preview : null;
    return (
      <>
        <text
          fg={theme.primary}
          attributes={TextAttributes.BOLD}
          height={1}
          wrapMode="none"
          truncate
        >
          {previewFile.path}
          {"  ·  image"}
        </text>
        <box flexGrow={1} minHeight={0} marginTop={1} alignItems="center" justifyContent="center">
          {preview ? (
            <DiffImage preview={preview} onError={onPreviewError} />
          ) : state.previewError ? (
            <text fg={theme.warning}>{`Image preview unavailable: ${state.previewError}`}</text>
          ) : (
            <text fg={theme.muted}>Loading image preview…</text>
          )}
        </box>
      </>
    );
  }
  if (!state.diff) {
    return (
      <text fg={theme.muted}>
        {state.files.length === 0 ? "No changed files." : "Select a file to view its diff."}
      </text>
    );
  }
  const split = state.diffMode === "split";
  const hiddenAbove = windowStart;
  const hiddenBelow = total - windowEnd;
  return (
    <>
      <text fg={theme.primary} attributes={TextAttributes.BOLD} height={1} wrapMode="none" truncate>
        {state.diff.path}
        {state.diff.truncated ? " (truncated)" : ""}
        {`  ·  ${state.diffMode}`}
        {collapsed ? "  ·  collapsed" : ""}
      </text>
      <scrollbox
        ref={scroll}
        id="detail-scroll"
        flexGrow={1}
        minHeight={0}
        marginTop={1}
        scrollX
        viewportCulling
      >
        {hiddenAbove > 0 ? (
          <text fg={theme.muted} wrapMode="none">{`… ${hiddenAbove} lines above (collapsed)`}</text>
        ) : null}
        {split
          ? splitAll.slice(windowStart, windowEnd).map((row, index) => splitRowElement(row, index))
          : lines.slice(windowStart, windowEnd).map((line, index) => (
              <text
                // biome-ignore lint/suspicious/noArrayIndexKey: diff lines are immutable and position-keyed
                key={index}
                id={line.highlighted ? "diff-highlight" : undefined}
                fg={line.highlighted ? theme.primary : line.fg}
                wrapMode="none"
              >
                {line.text}
              </text>
            ))}
        {hiddenBelow > 0 ? (
          <text
            fg={theme.muted}
            wrapMode="none"
          >{`… ${hiddenBelow} lines below — press e to expand`}</text>
        ) : null}
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

  const diffLinesData = useMemo(
    () => (state.diff ? diffLines(state.diff.text, state.highlightLine, state.highlightSide) : []),
    [state.diff, state.highlightLine, state.highlightSide],
  );
  const splitAll = useMemo(
    () => (state.diffMode === "split" ? splitRows(diffLinesData) : []),
    [state.diffMode, diffLinesData],
  );
  const diffTotal = state.diffMode === "split" ? splitAll.length : diffLinesData.length;
  const diffAnchor =
    state.diffMode === "split"
      ? splitHighlightIndex(splitAll)
      : diffLinesData.findIndex((line) => line.highlighted);
  const diffWindowInfo = diffWindow(diffTotal, diffAnchor, state.diffExpanded);
  const diffCollapsed = state.diff ? diffWindowInfo.collapsed : false;

  // Keep the highlighted region visible. `scrollChildIntoView` uses the real
  // layout offset, so wrapped split rows scroll correctly; a numeric fallback
  // covers a not-yet-laid-out child and the non-wrapping unified view.
  useEffect(() => {
    const view = scroll.current;
    if (!view || !state.diff) {
      return;
    }
    if (diffAnchor < 0) {
      // No anchor: preserve the reviewer's scroll position (for example when
      // expanding an unanchored diff).
      return;
    }
    // A new scroll request re-centres even when the anchor is unchanged.
    if (state.scrollRequest < 0) {
      return;
    }
    const aboveMarker = diffWindowInfo.start > 0 ? 1 : 0;
    const target = diffAnchor - diffWindowInfo.start + aboveMarker;
    const centered = () => Math.max(0, target - Math.floor(view.height / 2));
    if (state.diffMode === "split" || state.diffExpanded) {
      try {
        view.scrollChildIntoView("diff-highlight");
      } catch {
        view.scrollTop = centered();
      }
    } else {
      view.scrollTop = centered();
    }
  }, [
    state.scrollRequest,
    state.diff,
    diffAnchor,
    diffWindowInfo.start,
    state.diffMode,
    state.diffExpanded,
  ]);

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
        <box flexDirection="column" flexShrink={0} paddingX={1} backgroundColor={theme.panel}>
          <text fg={theme.secondary} height={1} wrapMode="none" truncate>
            {`review ${state.operation.state} · ${state.operation.targetLabel}`}
          </text>
          {state.operation.logs.slice(-5).map((log) => (
            <text key={log.id} fg={theme.muted} height={1} wrapMode="none" truncate>
              {log.text}
            </text>
          ))}
        </box>
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
                title={
                  state.history.length > 0 ? `FINDINGS · ${state.history.length} runs` : "FINDINGS"
                }
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
          <DiffPanel
            state={state}
            scroll={scroll}
            lines={diffLinesData}
            splitAll={splitAll}
            windowStart={diffWindowInfo.start}
            windowEnd={diffWindowInfo.end}
            collapsed={diffCollapsed}
            total={diffTotal}
            onPreviewError={(message) => store.setPreviewError(message)}
          />
        </box>
      </box>
      <box height={1} flexShrink={0} paddingX={1} backgroundColor={theme.panel}>
        <text fg={theme.muted}>
          {diffCollapsed ? "large diff · e expand  " : ""}
          {wide
            ? "j/k move  ←/→ scroll  s split  e expand  Tab pane  Enter review  x cancel  q quit"
            : "j/k  ←/→  s  e  Tab  q"}
        </text>
      </box>
    </box>
  );
}
