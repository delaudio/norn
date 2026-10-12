import { type ScrollBoxRenderable, SyntaxStyle } from "@opentui/core";
import { useKeyboard, usePaste, useRenderer } from "@opentui/react";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { RepositoryOperationCheckout } from "../../../../src/protocol/generated/protocol";
import type { BackendClient } from "../../transport/backendClient";
import { EditorController } from "../editorController";
import {
  type ExplorerState,
  type ExplorerStore,
  explorerLabel,
  explorerText,
  treeRows,
} from "../explorerStore";
import { RepositoryOperationsStore } from "../repositoryOperationsStore";
import { theme } from "../theme";
import { RepositoryOperationsPanel } from "./RepositoryOperationsPanel";

function statusLabel(status: string) {
  return status[0]!.toUpperCase() + status.slice(1);
}
function unavailable(state: string) {
  return (
    (
      {
        binary: "Binary file · text preview unavailable",
        tooLarge: "File exceeds the 512KiB text preview limit",
        deleted: "File removed from the worktree · diff remains available",
        missing: "File no longer exists · refresh its inventory",
        unsupported: "This path cannot be previewed as a regular file",
      } as Record<string, string>
    )[state] ?? "No preview"
  );
}
export function ExplorerPanel({
  store,
  width,
  height,
  onClose,
  onSettings,
  quit,
  client,
  editorAvailable,
  editorBusy = false,
  operationsAvailable = false,
  onRepositoryObserved,
}: {
  store: ExplorerStore;
  width: number;
  height: number;
  onClose: () => void;
  onSettings: () => void;
  quit: () => void;
  client: BackendClient;
  editorAvailable: boolean;
  editorBusy?: boolean;
  operationsAvailable?: boolean;
  onRepositoryObserved?: (checkout: RepositoryOperationCheckout) => void | Promise<void>;
}) {
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const renderer = useRenderer();
  const [operationsOpen, setOperationsOpen] = useState(false);
  const observed = useRef(onRepositoryObserved);
  useEffect(() => {
    observed.current = onRepositoryObserved;
  }, [onRepositoryObserved]);
  const operations = useMemo(
    () =>
      new RepositoryOperationsStore(client, async (checkout) => {
        await observed.current?.(checkout);
      }),
    [client],
  );
  useEffect(() => () => operations.dispose(), [operations]);
  const editor = useMemo(() => new EditorController(client, renderer), [client, renderer]);
  const editorState = useSyncExternalStore(editor.subscribe, editor.getSnapshot);
  useEffect(() => () => editor.dispose(), [editor]);
  const editorScope = JSON.stringify([state.context, state.path]);
  useEffect(() => editor.resetScope(editorScope), [editor, editorScope]);
  const markdownScroll = useRef<ScrollBoxRenderable | null>(null);
  const blameScroll = useRef<ScrollBoxRenderable | null>(null);
  useKeyboard((key) => {
    if (key.defaultPrevented || operationsOpen) return;
    key.preventDefault();
    if (key.ctrl && key.name === "c") {
      quit();
      return;
    }
    if (state.input) {
      if (key.name === "escape") store.cancelInput();
      else if (key.name === "return") void store.applyInput();
      else if (key.name === "backspace") store.backspaceInput();
      else if (key.ctrl && key.name === "u") store.clearInput();
      else if (!key.ctrl && !key.meta && !key.super && !key.option && key.sequence >= " ")
        store.appendInput(key.sequence);
      return;
    }
    if (editorState.busy && key.name !== "escape" && key.name !== "f8" && key.name !== "q") return;
    if ((key.ctrl || key.meta || key.super) && key.name === "f") {
      store.openInput("find");
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (key.name === "escape" || key.name === "f8") onClose();
    else if (key.name === "q") quit();
    else if (key.name === "o") {
      if (!editorAvailable) editor.unavailable();
      else if (editorBusy)
        store.editorError("Finish the current operation before opening an editor.");
      else if (state.context && state.path)
        void editor.start({
          context: state.context,
          path: state.path,
          mode: key.shift ? "terminal" : "graphical",
          line:
            state.mode === "blame"
              ? (state.blame?.lines[state.blameCursor]?.line ?? null)
              : state.mode === "content" && !state.markdown && state.preview?.state === "text"
                ? state.preview.line + state.previewRow
                : null,
        });
      else store.editorError("Select a regular repository file first.");
    } else if (key.name === "b") {
      if (editorBusy || editorState.busy)
        store.editorError("Finish the current operation before changing Git state.");
      else {
        setOperationsOpen(true);
        void operations.load(state.repository, operationsAvailable);
      }
    } else if (key.name === "m") onSettings();
    else if (key.name === "r") void store.refresh();
    else if (key.name === "tab") store.cycleFocus();
    else if (key.name === "/") store.openInput("filter");
    else if (key.name === "c") void store.toggleChanged();
    else if (key.name === "1") void store.setMode("content");
    else if (key.name === "2") void store.setMode("diff");
    else if (key.name === "3") void store.setMode("blame");
    else if (key.name === "v") store.toggleMarkdown();
    else if (key.name === "n") void store.moveMatch(key.shift ? -1 : 1);
    else if (key.name === "home") {
      if (markdownScroll.current) markdownScroll.current.scrollTop = 0;
      if (blameScroll.current) blameScroll.current.scrollTop = 0;
      void store.firstPreview();
    } else if (key.name === "return") {
      if (state.focus === "tree") void store.choose();
    } else if (["down", "up", "j", "k"].includes(key.name)) {
      const delta = key.name === "down" || key.name === "j" ? 1 : -1;
      if (state.focus === "details") blameScroll.current?.scrollBy(delta);
      else if (state.focus === "preview" && state.markdown) markdownScroll.current?.scrollBy(delta);
      else store.move(delta);
    } else if (key.name === "left" || key.name === "right") {
      if (state.focus === "preview" && !state.markdown && state.mode !== "blame")
        store.moveHorizontal(key.name === "right" ? 12 : -12);
    } else if (key.name === "pagedown" || key.name === "pageup") {
      const direction = key.name === "pagedown" ? 1 : -1;
      if (state.focus === "tree") void store.pageFiles(direction);
      else if (state.focus === "details")
        blameScroll.current?.scrollBy(direction * Math.max(1, height - 10));
      else if (state.markdown) {
        const scroll = markdownScroll.current;
        const previous = scroll?.scrollTop;
        scroll?.scrollBy(direction * Math.max(1, height - 10));
        if (scroll && previous === scroll.scrollTop) void store.pagePreview(direction);
      } else
        void store.pageViewport(
          direction,
          Math.max(
            1,
            (width >= 90
              ? height - 7
              : Math.max(3, height - 7 - Math.max(3, Math.floor((height - 7) / 3)))) - 4,
          ),
        );
    }
  });
  usePaste((event) => {
    event.preventDefault();
    if (!operationsOpen && state.input) store.appendInput(new TextDecoder().decode(event.bytes));
  });
  if (operationsOpen)
    return (
      <RepositoryOperationsPanel
        store={operations}
        width={width}
        height={height}
        quit={quit}
        onClose={() => {
          if (operations.getSnapshot().busy || operations.getSnapshot().loading) return;
          operations.dispose();
          setOperationsOpen(false);
          void store.refresh();
        }}
      />
    );
  const rows = treeRows(state);
  const wide = width >= 90;
  const bodyHeight = Math.max(3, height - 7);
  const treeHeight = wide ? bodyHeight : Math.max(3, Math.floor(bodyHeight / 3));
  const previewHeight = wide ? bodyHeight : Math.max(3, bodyHeight - treeHeight);
  const treeWidth = wide ? Math.min(44, Math.floor(width / 3)) : width - 2;
  const treeStart = Math.max(0, state.cursor - Math.max(1, treeHeight - 4) + 1);
  const error = editorState.error ?? state.error ?? state.previewError;
  return (
    <box
      width={width}
      height={height}
      flexDirection="column"
      paddingX={1}
      backgroundColor={theme.background}
    >
      <text
        height={1}
        flexShrink={0}
        fg={theme.primary}
        truncate
      >{`FILES · ${state.repository ? `${state.repository.provider} ${explorerLabel(state.repository.workspace)}/${explorerLabel(state.repository.repo)}` : "No selected repository"}`}</text>
      <text
        height={1}
        flexShrink={0}
        fg={theme.muted}
        truncate
      >{`clone ${state.context?.rootId.slice(0, 12) ?? "unavailable"} · HEAD ${state.head?.slice(0, 12) ?? "unborn"} · ${explorerLabel(state.branch ?? "detached")}`}</text>
      <box height={bodyHeight} flexShrink={0} flexDirection={wide ? "row" : "column"}>
        <box
          id="explorer-tree"
          width={treeWidth}
          height={treeHeight}
          border
          borderColor={state.focus === "tree" ? theme.primary : theme.border}
          paddingX={1}
          flexDirection="column"
          overflow="hidden"
        >
          <text
            height={1}
            flexShrink={0}
            fg={theme.muted}
            truncate
          >{`${state.changedOnly ? "CHANGED" : "ALL"} · ${state.total} files · ${state.query ? explorerLabel(state.query) : "no filter"}`}</text>
          <box
            height={Math.max(1, treeHeight - 4)}
            flexShrink={0}
            flexDirection="column"
            overflow="hidden"
          >
            {state.loading ? <text>Loading local files…</text> : null}
            {!state.loading && !state.error && !rows.length ? <text>No matching files</text> : null}
            {!state.loading && !state.error
              ? rows.slice(treeStart, treeStart + Math.max(1, treeHeight - 4)).map((row, index) => (
                  <text
                    key={`${row.kind}:${row.path}`}
                    height={1}
                    flexShrink={0}
                    wrapMode="none"
                    truncate
                    fg={treeStart + index === state.cursor ? theme.primary : theme.text}
                  >
                    {`${treeStart + index === state.cursor ? "›" : " "}${" ".repeat(Math.min(8, row.depth) * 2)}${row.kind === "folder" ? `${state.collapsed.includes(row.path) ? "▸" : "▾"} ` : ""}${explorerLabel(row.name)}${row.file ? ` · ${statusLabel(row.file.status)}` : "/"}`}
                  </text>
                ))
              : null}
          </box>
          <text
            height={1}
            flexShrink={0}
            fg={theme.muted}
            truncate
          >{`${state.total ? state.offset + 1 : 0}–${state.offset + state.files.length}/${state.total} · PgUp/PgDn`}</text>
        </box>
        <box
          id="explorer-preview"
          width={wide ? width - treeWidth - 2 : width - 2}
          height={previewHeight}
          border
          borderColor={state.focus !== "tree" ? theme.primary : theme.border}
          paddingX={1}
          flexDirection="column"
          overflow="hidden"
        >
          <text
            height={1}
            flexShrink={0}
            fg={theme.primary}
            truncate
          >{`${state.mode.toUpperCase()} · ${state.path ? explorerLabel(state.path) : "Select a file"}${state.markdown ? " · rendered" : ""}`}</text>
          {state.previewLoading ? (
            <text>Loading verified preview…</text>
          ) : state.mode === "blame" ? (
            <Blame state={state} height={Math.max(1, previewHeight - 3)} scroll={blameScroll} />
          ) : state.preview ? (
            state.preview.state === "text" || state.preview.state === "diff" ? (
              state.markdown ? (
                <Markdown
                  key={`${state.preview.sha}:${state.preview.offset}`}
                  text={state.preview.text}
                  height={Math.max(1, previewHeight - 3)}
                  scroll={markdownScroll}
                />
              ) : (
                <Source state={state} height={Math.max(1, previewHeight - 3)} />
              )
            ) : (
              <text fg={theme.warning}>{unavailable(state.preview.state)}</text>
            )
          ) : (
            <text fg={theme.muted}>
              {state.previewError
                ? "Refresh to retry this preview"
                : "Enter a file · 1 content · 2 diff · 3 blame"}
            </text>
          )}
        </box>
      </box>
      <text height={1} flexShrink={0} fg={error ? theme.error : theme.muted} truncate>
        {error
          ? explorerLabel(error)
          : editorState.message
            ? explorerLabel(editorState.message)
            : state.input
              ? `${state.input === "filter" ? "Path filter" : "Find"}: ${explorerLabel(state.inputText)}█ · Enter apply · Esc cancel`
              : state.searchLoading
                ? "Finding matches…"
                : state.search
                  ? `Find ${explorerLabel(state.search.request.query)} · ${state.search.total ? state.search.request.offset + state.searchCursor + 1 : 0}/${state.search.total} · n/N next/previous`
                  : "Tab pane · j/k · Enter · / filter · c changed"}
      </text>
      <text height={1} flexShrink={0} fg={theme.muted} truncate>
        1 text · 2 diff · 3 blame · Ctrl-F find · v MD
      </text>
      <text height={1} flexShrink={0} fg={theme.muted} truncate>
        {editorBusy
          ? "Workspace busy · wait before opening an editor"
          : "o open · O terminal · b branches · r refresh · m settings"}
      </text>
      <text height={1} flexShrink={0} fg={theme.muted} truncate>
        F8/Esc return to review · q quit
      </text>
    </box>
  );
}
function Source({ state, height }: { state: ExplorerState; height: number }) {
  const preview = state.preview!;
  const lines = preview.text.split("\n");
  const start = Math.min(state.previewRow, Math.max(0, lines.length - 1));
  const match = state.search?.matches[state.searchCursor];
  const highlight =
    match && match.startByte === preview.offset ? match.endUtf16 - match.startUtf16 : 0;
  let byteOffset = preview.offset;
  const sourceRows = lines.map((text, index) => {
    const row = {
      id: `${preview.sha}:${byteOffset}`,
      text,
      line: preview.line + index,
      offset: byteOffset,
    };
    byteOffset += Buffer.byteLength(text) + 1;
    return row;
  });
  return (
    <box height={height} flexShrink={0} flexDirection="column" overflow="hidden">
      <text
        height={1}
        flexShrink={0}
        fg={theme.muted}
        truncate
      >{`${preview.state === "diff" ? "Preview row" : "Line"} ${preview.line + start}${preview.column ? ` · starts at column ${preview.column + 1}` : ""}${state.previewColumn ? ` · pan ${state.previewColumn}` : ""} · ${preview.next !== null ? "more ↓" : "end"}${state.previous.length ? " · previous ↑" : ""}`}</text>
      {sourceRows.slice(start, start + Math.max(1, height - 1)).map((row) => (
        <text key={row.id} height={1} flexShrink={0} truncate wrapMode="none" fg={theme.text}>
          <span fg={theme.muted}>{`${String(row.line).padStart(5)} │ `}</span>
          {state.previewColumn ? (
            Array.from(explorerText(row.text)).slice(state.previewColumn).join("")
          ) : row.offset === preview.offset && highlight ? (
            <>
              <span fg={theme.primary}>{explorerText(row.text.slice(0, highlight))}</span>
              {explorerText(row.text.slice(highlight))}
            </>
          ) : (
            explorerText(row.text)
          )}
        </text>
      ))}
    </box>
  );
}
function Blame({
  state,
  height,
  scroll,
}: {
  state: ExplorerState;
  height: number;
  scroll: React.RefObject<ScrollBoxRenderable | null>;
}) {
  const page = state.blame;
  if (!page) return <text>No blame loaded</text>;
  if (!page.available)
    return <text fg={theme.warning}>Blame is unavailable for this uncommitted file</text>;
  if (!page.lines.length) return <text>No lines in this file</text>;
  const selected = page.lines[state.blameCursor];
  const rows = Math.max(1, Math.floor(height / 3));
  const start = Math.max(0, state.blameCursor - rows + 1);
  return (
    <box height={height} flexShrink={0} flexDirection="column" overflow="hidden">
      <box height={rows} flexShrink={0} flexDirection="column" overflow="hidden">
        {page.lines.slice(start, start + rows).map((line, index) => (
          <text
            key={line.line}
            height={1}
            flexShrink={0}
            truncate
            fg={start + index === state.blameCursor ? theme.primary : theme.text}
          >{`${line.line} ${line.shortSha} ${explorerLabel(line.author ?? "Unknown author")} · ${explorerLabel(line.summary ?? "Uncommitted")}`}</text>
        ))}
      </box>
      <scrollbox
        ref={scroll}
        key={`${selected?.sha}:${selected?.line}`}
        height={Math.max(1, height - rows)}
      >
        <text fg={state.focus === "details" ? theme.primary : theme.muted}>
          Tab commit details · j/k scroll
        </text>
        <text fg={theme.primary}>{selected?.sha}</text>
        <text>
          {explorerText(
            `${selected?.author ?? "Unknown author"} <${selected?.authorEmail ?? "no email"}>`,
          )}
        </text>
        <text fg={theme.muted}>
          {selected?.authorTime === null
            ? "No author time"
            : `Author time: ${selected?.authorTime}`}
        </text>
        <text>
          {explorerText(selected?.message ?? selected?.summary ?? "Uncommitted working-tree line")}
        </text>
      </scrollbox>
    </box>
  );
}
function Markdown({
  text,
  height,
  scroll,
}: {
  text: string;
  height: number;
  scroll: React.RefObject<ScrollBoxRenderable | null>;
}) {
  const style = useMemo(
    () =>
      SyntaxStyle.fromStyles({
        default: { fg: theme.text },
        "markup.heading": { fg: theme.primary, bold: true },
        "markup.strong": { bold: true },
        "markup.italic": { italic: true },
        "markup.link": { fg: theme.secondary },
      }),
    [],
  );
  useEffect(() => () => style.destroy(), [style]);
  // Preview code fences remain plain; source mode retains the exact info string.
  // A readonly file preview must not fetch grammar assets while rendering.
  const content = explorerText(text).replace(/^(\s*)(`{3,}|~{3,})[^\n]*$/gm, "$1$2");
  return (
    <scrollbox ref={scroll} height={height}>
      <markdown content={content} syntaxStyle={style} />
    </scrollbox>
  );
}
