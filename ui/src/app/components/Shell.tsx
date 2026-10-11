import {
  CliRenderEvents,
  resolveImageRenderProtocol,
  type ScrollBoxRenderable,
  TextAttributes,
} from "@opentui/core";
import { useKeyboard, usePaste, useRenderer, useTerminalDimensions } from "@opentui/react";
import {
  type ReactNode,
  type RefObject,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react";
import type { BackendClient } from "../../transport/backendClient";
import { DataStore } from "../dataStore";
import { DiffScrollPositions } from "../diffScrollPositions";
import { SettingsStore } from "../settingsStore";
import {
  type FilePreviewView,
  isImagePath,
  type ShellSnapshot,
  type ShellStore,
  visibleFindings,
} from "../store";
import type { DiffSyntax } from "../syntaxTypes";
import { theme } from "../theme";
import { WorkspaceStore } from "../workspaceStore";
import { ConversationPanel } from "./ConversationPanel";
import { DataPanel } from "./DataPanel";
import { FixPanel } from "./FixPanel";
import { SettingsPanel } from "./SettingsPanel";
import { VirtualDiff } from "./VirtualDiff";
import { WorkspacePanel } from "./WorkspacePanel";

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
  fixScroll: RefObject<ScrollBoxRenderable | null>,
) {
  const renderer = useRenderer();
  useKeyboard((key) => {
    if (
      key.defaultPrevented ||
      store.getSnapshot().settingsOpen ||
      store.getSnapshot().dataOpen ||
      store.getSnapshot().workspaceOpen
    )
      return;
    if (store.getSnapshot().searchOpen) {
      key.preventDefault();
      if (key.name === "escape") store.cancelSearch();
      else if (key.name === "return") store.applySearch();
      else if (key.name === "backspace") store.backspaceSearch();
      else if (key.ctrl && key.name === "u") store.clearSearchText();
      else if (!key.ctrl && !key.meta && !key.super && !key.option && key.sequence >= " ")
        store.appendSearch(key.sequence);
      return;
    }
    if (key.ctrl && key.name === "c") {
      quit();
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) {
      return;
    }
    if (store.getSnapshot().fixPreview) {
      key.preventDefault();
      if (key.name === "y") void store.confirmFix(client);
      else if (key.name === "escape" || key.name === "n") store.dismissFix();
      else if (key.name === "down" || key.name === "up")
        fixScroll.current?.scrollBy(key.name === "down" ? 1 : -1);
      else if (key.name === "pagedown" || key.name === "pageup")
        fixScroll.current?.scrollBy(key.name === "pagedown" ? 5 : -5);
      return;
    }
    if (store.getSnapshot().composing) {
      key.preventDefault();
      if (key.name === "escape") {
        store.cancelReply();
      } else if (key.name === "return") {
        if (store.getSnapshot().composerKind === "commit") void store.previewCommit(client);
        else void store.submitReply(client);
      } else if (key.name === "backspace") {
        store.backspaceReply();
      } else if (key.sequence.length > 0 && key.sequence >= " ") {
        store.appendReplyText(key.sequence);
      }
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
      if (store.getSnapshot().focus === "threads") void store.selectThread();
      else void store.startReview(client);
    } else if (key.name === "f") {
      store.cycleFindingFilter();
    } else if (key.name === "g") {
      store.cycleFindingGroup();
    } else if (key.name === "t") {
      store.toggleThreads();
    } else if (key.name === "v") {
      store.toggleConversation();
    } else if (key.name === "s") {
      store.toggleDiffMode();
    } else if (key.sequence === "/") {
      store.beginSearch();
    } else if (key.sequence === "]") {
      store.nextSearch(1);
    } else if (key.sequence === "[") {
      store.nextSearch(-1);
    } else if (key.name === "e") {
      store.toggleDiffExpanded();
    } else if (key.name === "c") {
      void store.beginCommit(client);
    } else if (key.name === "p") {
      void store.previewPush(client);
    } else if (key.name === "u") {
      void store.previewBranch(client, "sync");
    } else if (key.name === "o") {
      void store.previewBranch(client, "conflicts");
    } else if (key.name === "a") {
      void store.previewFix(client);
    } else if (key.name === "r") {
      store.beginReply();
    } else if (key.name === "b") {
      void store.openBrowser(client);
    } else if (key.name === "y") {
      void store.copyReviewOutput(client, (text) => renderer.copyToClipboardOSC52(text));
    } else if (key.name === "m") {
      store.toggleSettings();
    } else if (key.name === "n") {
      store.toggleWorkspace();
    } else if (key.name === "z") {
      void store.refreshOperation();
    } else if (key.name === "x") {
      void store.cancel(client);
    }
  });
  usePaste((event) => {
    if (
      store.getSnapshot().settingsOpen ||
      store.getSnapshot().dataOpen ||
      store.getSnapshot().workspaceOpen
    )
      return;
    if (store.getSnapshot().searchOpen) {
      event.preventDefault();
      store.appendSearch(new TextDecoder().decode(event.bytes));
      return;
    }
    if (store.getSnapshot().fixPreview) {
      event.preventDefault();
      return;
    }
    if (store.getSnapshot().composing) {
      event.preventDefault();
      store.appendReplyText(new TextDecoder().decode(event.bytes));
    }
  });
}

export type DiffLineKind = "hunk" | "context" | "add" | "del" | "meta";

interface DiffLine {
  rawIndex: number;
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
    lines.push({ rawIndex: lines.length, text: raw, fg, highlighted, kind, oldNumber, newNumber });
  }
  return lines;
}

type SplitRow =
  | { kind: "hunk"; text: string; highlighted?: boolean }
  | { kind: "note"; text: string; side: "old" | "new" | null; highlighted?: boolean }
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
      rows.push({
        kind: "hunk",
        text: line.text,
        ...(line.highlighted ? { highlighted: true } : {}),
      });
      index += 1;
      continue;
    }
    if (line.kind === "del" || line.kind === "add") {
      const removed: DiffLine[] = [];
      const added: DiffLine[] = [];
      const notes: Array<{ text: string; side: "old" | "new" | null; highlighted?: boolean }> = [];
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
          notes.push({
            text: current.text,
            side: lastSide,
            ...(current.highlighted ? { highlighted: true } : {}),
          });
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
        rows.push({ kind: "note", ...note });
      }
      continue;
    }
    if (line.kind === "meta") {
      rows.push({
        kind: "note",
        text: line.text,
        side: null,
        ...(line.highlighted ? { highlighted: true } : {}),
      });
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

const syntaxColors: Record<string, string> = {
  comment: "#8892a6",
  string: "#a5d6a7",
  keyword: "#c792ea",
  function: "#82aaff",
  number: "#f78c6c",
  boolean: "#f78c6c",
  operator: "#89ddff",
  punctuation: "#a6accd",
  "class-name": "#ffcb6b",
  property: "#82aaff",
  tag: "#f07178",
  "attr-name": "#c792ea",
  "attr-value": "#a5d6a7",
  regex: "#a5d6a7",
};
function sourceContent(
  line: DiffLine | null,
  syntax: DiffSyntax | null,
  side?: "old" | "new",
): ReactNode {
  if (!line) return "";
  const prefix = side
    ? `${String((side === "old" ? line.oldNumber : line.newNumber) ?? "").padStart(4, " ")} `
    : "";
  const parts = syntax?.rows[line.rawIndex];
  if (!parts || line.highlighted) return `${prefix}${line.text}`;
  let sourceOffset = 0;
  return (
    <>
      {prefix}
      {line.text.slice(0, 1)}
      {parts.map((part) => {
        const start = sourceOffset;
        sourceOffset += part.text.length;
        return (
          <span
            key={`${line.rawIndex}:${start}:${part.token}`}
            fg={part.token ? (syntaxColors[part.token] ?? line.fg) : line.fg}
          >
            {part.text}
          </span>
        );
      })}
    </>
  );
}
function splitRowElement(row: SplitRow, key: number, syntax: DiffSyntax | null): ReactNode {
  if (row.kind === "hunk") {
    return (
      <text
        key={key}
        id={row.highlighted ? "diff-highlight" : undefined}
        fg={row.highlighted ? theme.primary : theme.muted}
        wrapMode="none"
      >
        {row.text}
      </text>
    );
  }
  if (row.kind === "note") {
    if (row.side === null) {
      return (
        <text
          key={key}
          id={row.highlighted ? "diff-highlight" : undefined}
          fg={row.highlighted ? theme.primary : theme.muted}
          wrapMode="none"
        >
          {row.text}
        </text>
      );
    }
    return (
      <box
        key={key}
        id={row.highlighted ? "diff-highlight" : undefined}
        flexDirection="row"
        height={1}
      >
        <box width="50%" minWidth={0}>
          <text fg={row.highlighted ? theme.primary : theme.muted} wrapMode="none" truncate>
            {row.side === "old" ? row.text : ""}
          </text>
        </box>
        <box width="50%" minWidth={0}>
          <text fg={row.highlighted ? theme.primary : theme.muted} wrapMode="none" truncate>
            {row.side === "new" ? row.text : ""}
          </text>
        </box>
      </box>
    );
  }
  const highlighted = row.left?.highlighted === true || row.right?.highlighted === true;
  return (
    <box
      key={key}
      id={highlighted ? "diff-highlight" : undefined}
      flexDirection="row"
      height="100%"
    >
      <box width="50%" minWidth={0} height="100%">
        <text
          fg={row.left?.highlighted ? theme.primary : (row.left?.fg ?? theme.text)}
          wrapMode="word"
          height="100%"
          width="100%"
        >
          {sourceContent(row.left, syntax, "old")}
        </text>
      </box>
      <box width="50%" minWidth={0} height="100%">
        <text
          fg={row.right?.highlighted ? theme.primary : (row.right?.fg ?? theme.text)}
          wrapMode="word"
          height="100%"
          width="100%"
        >
          {sourceContent(row.right, syntax, "new")}
        </text>
      </box>
    </box>
  );
}

/// Diff rows rendered at once before collapsing; bounds React mounts on very
/// large diffs while keeping the highlighted region visible.
const MAX_RENDERED_DIFF_ROWS = 2000;

function splitHighlightIndex(rows: SplitRow[]): number {
  return rows.findIndex((row) =>
    row.kind === "pair"
      ? row.left?.highlighted === true || row.right?.highlighted === true
      : row.highlighted === true,
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

function useImageProtocol() {
  const renderer = useRenderer();
  const subscribe = useMemo(
    () => (listener: () => void) => {
      renderer.on(CliRenderEvents.CAPABILITIES, listener);
      renderer.on(CliRenderEvents.RESIZE, listener);
      return () => {
        renderer.off(CliRenderEvents.CAPABILITIES, listener);
        renderer.off(CliRenderEvents.RESIZE, listener);
      };
    },
    [renderer],
  );
  return useSyncExternalStore(subscribe, () =>
    resolveImageRenderProtocol("auto", renderer.capabilities, renderer.resolution !== null),
  );
}

function ImageComparison({
  file,
  preview,
  onError,
}: {
  file: { path: string; status: string };
  preview: import("../store").ImageComparisonView | null;
  onError: (side: "old" | "new", message: string, requestId: number) => void;
}) {
  const { width } = useTerminalDimensions();
  const protocol = useImageProtocol();
  const panes = [
    { side: "old" as const, label: "BEFORE", value: preview?.before, error: preview?.beforeError },
    { side: "new" as const, label: "AFTER", value: preview?.after, error: preview?.afterError },
  ];
  return (
    <box flexDirection="column" flexGrow={1} minHeight={0}>
      <text
        fg={theme.primary}
        height={1}
        wrapMode="none"
        truncate
      >{`${file.path} · ${file.status} image comparison`}</text>
      <box flexDirection={width >= 80 ? "row" : "column"} flexGrow={1} minHeight={0}>
        {panes.map((pane) => (
          <box
            key={pane.side}
            flexDirection="column"
            flexGrow={1}
            flexBasis={0}
            minWidth={0}
            minHeight={0}
            paddingX={1}
          >
            <text fg={theme.secondary} height={1} truncate>
              {pane.label}
            </text>
            <box flexGrow={1} minHeight={0} alignItems="center" justifyContent="center">
              {pane.error ? (
                <text fg={theme.warning}>{`Preview unavailable: ${pane.error}`}</text>
              ) : pane.value ? (
                pane.value.available ? (
                  <DiffImage
                    preview={pane.value}
                    onError={(message) => onError(pane.side, message, preview!.requestId)}
                  />
                ) : (
                  <text fg={theme.muted}>
                    {pane.side === "old"
                      ? "No before image (new file)."
                      : "No after image (deleted file)."}
                  </text>
                )
              ) : (
                <text fg={theme.muted}>Loading image…</text>
              )}
            </box>
          </box>
        ))}
      </box>
      <text fg={theme.muted} height={1} truncate>
        {protocol === "blocks"
          ? "No native graphics protocol detected: showing block previews; b opens the browser comparison."
          : `${protocol} image previews · b opens the authenticated browser comparison.`}
      </text>
    </box>
  );
}

function DiffPanel({
  state,
  positions,
  identity,
  scroll,
  lines,
  splitAll,
  windowStart,
  windowEnd,
  collapsed,
  total,
  onPreviewError,
  anchor,
}: {
  state: ShellSnapshot;
  positions: DiffScrollPositions;
  identity: string;
  scroll: RefObject<ScrollBoxRenderable | null>;
  lines: DiffLine[];
  onPreviewError: (side: "old" | "new", message: string, requestId: number) => void;
  splitAll: SplitRow[];
  windowStart: number;
  windowEnd: number;
  collapsed: boolean;
  total: number;
  anchor: number;
}) {
  const texts = useMemo(
    () =>
      state.diffMode === "split"
        ? splitAll
            .slice(windowStart, windowEnd)
            .map((row) =>
              row.kind === "pair"
                ? [lineLabel(row.left, "old"), lineLabel(row.right, "new")]
                : [row.text],
            )
        : lines.slice(windowStart, windowEnd).map((line) => [line.text]),
    [state.diffMode, splitAll, lines, windowStart, windowEnd],
  );
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
    return (
      <box flexDirection="column">
        <text fg={theme.primary}>Welcome to Norn</text>
        <text fg={theme.muted}>
          No repositories configured. Press m for settings or run `norn setup`.
        </text>
        <text fg={theme.text}>
          Add a provider, owner and repository; credentials are saved separately.
        </text>
        <text fg={theme.muted}>
          Existing credentials stay in Rust. Settings show presence, never stored values.
        </text>
        {state.readiness ? (
          <text fg={theme.warning}>
            {`Selected agent: ${state.readiness.selectedAgent} · ${state.readiness.tools.find((tool) => tool.name === state.readiness?.selectedAgent)?.available ? "found on PATH" : "missing on PATH"}. Tool presence does not verify login.`}
          </text>
        ) : null}
        {state.readiness?.credentialStore === "unsupported" ? (
          <text fg={theme.warning}>
            This build has no native credential store. Use existing environment credentials; no
            plaintext fallback.
          </text>
        ) : null}
        <text fg={theme.muted}>
          Ctrl+D in settings previews/imports desktop data with a backup. No review starts until
          Enter.
        </text>
      </box>
    );
  }
  if (state.diffLoading) {
    return <text fg={theme.muted}>Loading diff…</text>;
  }
  const previewFile = state.files[state.selectedFile];
  if (previewFile && isImagePath(previewFile.path)) {
    const preview = state.preview?.path === previewFile.path ? state.preview : null;
    return <ImageComparison file={previewFile} preview={preview} onError={onPreviewError} />;
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
        {state.syntaxLoading
          ? "  ·  coloring…"
          : state.syntax?.status === "ready"
            ? `  ·  ${state.syntax.language}`
            : "  ·  plain text"}
      </text>
      {state.syntax && state.syntax.status !== "ready" && state.syntax.reason ? (
        <text fg={theme.muted} height={1} truncate>
          {state.syntax.reason}
        </text>
      ) : null}
      <VirtualDiff
        scroll={scroll}
        texts={texts}
        anchor={anchor < 0 ? -1 : anchor - windowStart}
        request={state.scrollRequest}
        identity={identity}
        positions={positions}
        above={
          hiddenAbove > 0 ? (
            <text
              fg={theme.muted}
              wrapMode="none"
            >{`… ${hiddenAbove} lines above (collapsed)`}</text>
          ) : null
        }
        below={
          hiddenBelow > 0 ? (
            <text
              fg={theme.muted}
              wrapMode="none"
            >{`… ${hiddenBelow} lines below — press e to expand`}</text>
          ) : null
        }
        renderRow={(index) => {
          const position = windowStart + index;
          if (split) return splitRowElement(splitAll[position]!, position, state.syntax);
          const line = lines[position]!;
          return (
            <text
              id={line.highlighted ? "diff-highlight" : undefined}
              fg={line.highlighted ? theme.primary : line.fg}
              wrapMode="none"
            >
              {sourceContent(line, state.syntax)}
            </text>
          );
        }}
      />
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
  const viewed = new Set(state.viewedFiles);
  return state.files.map((file) => ({
    name: `${viewed.has(file.path) ? "✓ " : ""}${file.status[0]?.toUpperCase() ?? "?"} ${file.path}`,
    description: `+${file.additions} -${file.deletions}`,
  }));
}

function findingOptions(state: ShellSnapshot): ListOption[] {
  const findings = visibleFindings(state);
  return findings.length === 0
    ? [{ name: "No findings", description: "" }]
    : findings.map((finding) => ({
        name: `${state.findingGroup === "file" ? `${finding.anchor?.path ?? "Unanchored"} · ` : ""}[${finding.severity}] ${finding.title}`,
        description: finding.anchor?.path ?? "",
      }));
}

function WorkspaceStatus({ state }: { state: ShellSnapshot }) {
  const selectedFinding = visibleFindings(state)[state.selectedFinding];
  return (
    <>
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
            {`review ${state.operation.state} · ${state.operation.targetLabel} · x cancel · z refresh`}
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
      {state.findingFilter !== "all" || state.findingGroup !== "none" ? (
        <text fg={theme.muted} height={1} wrapMode="none" truncate>
          {`Findings: ${visibleFindings(state).length}/${state.findings.length} · severity ${state.findingFilter} · group ${state.findingGroup}`}
        </text>
      ) : null}
    </>
  );
}

export function Shell({ store, client, quit }: ShellProps) {
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const positions = useMemo(() => new DiffScrollPositions(), []);
  useEffect(() => () => positions.clear(), [positions]);
  useEffect(() => () => store.cancelSyntax(), [store]);
  const workspace = useMemo(() => new WorkspaceStore(), []);
  const workspaceEntry = state.picker[state.selectedPicker];
  useEffect(() => {
    if (!state.workspaceOpen) void store.refreshViewed(client, state.generation);
  }, [store, client, state.generation, state.workspaceOpen]);
  useEffect(() => {
    if (!state.workspaceOpen) return;
    void workspace.load(client, workspaceEntry);
    return () => workspace.dispose();
  }, [workspace, client, state.workspaceOpen, workspaceEntry]);
  const data = useMemo(() => new DataStore(), []);
  useEffect(() => {
    if (!state.dataOpen) return;
    return () => data.dispose();
  }, [data, state.dataOpen]);
  const settings = useMemo(() => new SettingsStore(), []);
  useEffect(() => {
    if (!state.settingsOpen) return;
    void settings.load(client);
    return () => settings.dispose();
  }, [settings, client, state.settingsOpen]);
  useEffect(() => {
    if (!["accepted", "running"].includes(state.operation?.state ?? "")) return;
    const timer = setInterval(() => void store.pollOperation(), 1000);
    return () => clearInterval(timer);
  }, [store, state.operation?.state]);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  const { width } = useTerminalDimensions();
  const fixScroll = useRef<ScrollBoxRenderable | null>(null);
  useShellInput(store, client, quit, scroll, fixScroll);
  const wide = width >= 100;

  const diffLinesData = useMemo(() => {
    const lines = state.diff
      ? diffLines(state.diff.text, state.highlightLine, state.highlightSide)
      : [];
    return state.searchRow === null
      ? lines
      : lines.map((line, index) => ({ ...line, highlighted: index === state.searchRow }));
  }, [state.diff, state.highlightLine, state.highlightSide, state.searchRow]);
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

  const repo = state.repositories[state.selected];
  const entry = state.picker[state.selectedPicker];
  const contentHash = useMemo(
    () => (state.diff ? new Bun.CryptoHasher("sha256").update(state.diff.text).digest("hex") : ""),
    [state.diff],
  );
  const diffIdentity = JSON.stringify([
    entry?.provider,
    entry?.workspace,
    entry?.repo,
    entry?.prId,
    entry?.runId,
    state.diff?.path,
    contentHash,
    state.diffMode,
    state.diffExpanded,
  ]);
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
      : state.focus === "threads"
        ? {
            id: "threads-list",
            title: "THREADS",
            options: state.threads.map((thread) => ({
              name: thread.title,
              description: thread.latestRunId ?? "No stored run",
            })),
            selectedIndex: state.selectedThread,
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

  if (state.workspaceOpen)
    return (
      <WorkspacePanel
        workspace={workspace}
        client={client}
        path={state.files[state.selectedFile]?.path}
        onClose={() => store.toggleWorkspace()}
      />
    );
  if (state.dataOpen)
    return <DataPanel data={data} client={client} onClose={() => store.toggleData()} />;
  if (state.settingsOpen)
    return <SettingsPanel settings={settings} shell={store} client={client} />;

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
      <WorkspaceStatus state={state} />
      <ConversationPanel state={state} wide={wide} />
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
            positions={positions}
            identity={diffIdentity}
            scroll={scroll}
            lines={diffLinesData}
            splitAll={splitAll}
            windowStart={diffWindowInfo.start}
            windowEnd={diffWindowInfo.end}
            collapsed={diffCollapsed}
            total={diffTotal}
            anchor={diffAnchor}
            onPreviewError={(side, message, requestId) =>
              store.setPreviewError(side, message, requestId)
            }
          />
          {state.searchOpen ? (
            <text
              fg={theme.primary}
              height={1}
              truncate
            >{`Search: ${state.searchText} · Enter apply · Esc cancel · Ctrl+U clear`}</text>
          ) : state.searchQuery ? (
            <text
              fg={theme.muted}
              height={1}
              truncate
            >{`Search ${state.searchQuery} · ${state.searchMatches.length ? state.searchSelected + 1 : 0}/${state.searchMatches.length} · ] next · [ previous`}</text>
          ) : null}
        </box>
      </box>
      <FixPanel
        scroll={fixScroll}
        preview={state.fixPreview}
        result={state.fixState}
        loading={state.fixPreviewLoading}
      />
      {state.composing ? (
        <box height={1} flexShrink={0} paddingX={1} backgroundColor={theme.panel}>
          <text fg={theme.primary} wrapMode="none" truncate>
            {`${state.composerKind === "commit" ? "Commit" : "Reply"} › ${state.composerText}`}
          </text>
        </box>
      ) : null}
      <box height={1} flexShrink={0} paddingX={1} backgroundColor={theme.panel}>
        <text fg={theme.muted}>
          {diffCollapsed ? "large diff · e expand  " : ""}
          {state.fixPreview
            ? "y approve · Esc/n dismiss · ↑/↓ scroll effects"
            : wide
              ? "j/k move  s split  e expand  Tab pane  a fix  u sync · o conflicts · c commit  p push  r reply  t threads  v chat  f filter  g group  x cancel  m settings  n prompt/viewed  q quit"
              : "j/k  ←/→  s  e  Tab  m settings  n prompt  q"}
        </text>
      </box>
    </box>
  );
}
