import { CliRenderEvents, type ScrollBoxRenderable } from "@opentui/core";
import { useKeyboard, usePaste, useRenderer } from "@opentui/react";
import { type RefObject, useEffect, useRef, useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import { type DraftStore, draftFields } from "../draftStore";
import { theme } from "../theme";

const fieldLabel = {
  publicationMode: "Mode",
  path: "Path",
  to: "New-side line",
  from: "Old-side line",
  parentId: "Reply to comment",
  raw: "Body",
};
function useDraftInput(
  drafts: DraftStore,
  client: BackendClient,
  onClose: () => void,
  state: ReturnType<DraftStore["getSnapshot"]>,
  scroll: RefObject<ScrollBoxRenderable | null>,
) {
  useKeyboard((key) => {
    key.preventDefault();
    key.stopPropagation();
    if (state.busy) return;
    if (key.name === "pagedown" || key.name === "pageup") {
      const view = scroll.current;
      view?.scrollBy((key.name === "pagedown" ? 1 : -1) * Math.max(1, view.height - 2));
      return;
    }
    if (state.removing) {
      if (key.name === "y") void drafts.remove(client);
      else if (key.name === "escape" || key.name === "n") drafts.cancel();
      return;
    }
    if (state.editing) {
      if (key.name === "escape") drafts.cancel();
      else if (key.ctrl && key.name === "u") drafts.clearField();
      else if (key.name === "backspace") drafts.backspace();
      else if (key.name === "return" && key.shift && draftFields[state.field] === "raw")
        drafts.append("\n");
      else if (key.name === "return") drafts.finishField();
      else if (!key.ctrl && !key.meta && !key.super && !key.option && key.sequence >= " ")
        drafts.append(key.sequence);
      return;
    }
    if (state.draft) {
      if (key.name === "escape") drafts.cancel();
      else if (key.ctrl && key.name === "s") void drafts.save(client);
      else if (key.name === "return") drafts.beginField();
      else if (["j", "down", "tab"].includes(key.name)) drafts.move(1);
      else if (["k", "up"].includes(key.name)) drafts.move(-1);
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (key.name === "escape" || key.name === "q") onClose();
    else if (key.name === "n") drafts.add();
    else if (key.name === "e" || key.name === "return") drafts.edit();
    else if (key.name === "d") drafts.requestRemoval();
    else if (["j", "down"].includes(key.name)) drafts.move(1);
    else if (["k", "up"].includes(key.name)) drafts.move(-1);
  });
  usePaste((event) => {
    event.preventDefault();
    if (state.editing) drafts.append(new TextDecoder().decode(event.bytes));
  });
}

function useDraftFieldFollow(
  state: ReturnType<DraftStore["getSnapshot"]>,
  scroll: RefObject<ScrollBoxRenderable | null>,
) {
  const renderer = useRenderer();
  const draftOpen = state.draft !== null;
  useEffect(() => {
    if (!draftOpen) return;
    let previous = "";
    const followField = () => {
      const view = scroll.current;
      const id = `draft-field-${state.field}`;
      const child = view?.findDescendantById(id);
      if (!view || !child) return;
      // Observe completed native layout: React's commit can precede wrapping
      // measurement. The signature excludes scroll offset to avoid a draw loop.
      const signature = [
        view.viewport.width,
        view.viewport.height,
        view.scrollHeight,
        child.width,
        child.height,
        child.y - view.content.y,
        state.text.length,
      ].join(":");
      if (signature === previous) return;
      previous = signature;
      if (state.editing && draftFields[state.field] === "raw" && state.text)
        view.scrollTo(view.scrollHeight);
      else view.scrollChildIntoView(id);
    };
    renderer.on(CliRenderEvents.FRAME, followField);
    return () => {
      renderer.off(CliRenderEvents.FRAME, followField);
    };
  }, [renderer, state.field, draftOpen, state.text, state.editing, scroll]);
}

export function DraftPanel({
  drafts,
  client,
  onClose,
}: {
  drafts: DraftStore;
  client: BackendClient;
  onClose: () => void;
}) {
  const state = useSyncExternalStore(drafts.subscribe, drafts.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  useDraftFieldFollow(state, scroll);
  useDraftInput(drafts, client, onClose, state, scroll);
  const selected = state.items[state.selected];
  return (
    <box flexDirection="column" flexGrow={1} paddingX={1} backgroundColor={theme.background}>
      <text
        fg={theme.primary}
        height={1}
        flexShrink={0}
        truncate
      >{`Staged comments · ${state.key?.slice("drafts.".length).replace(/\/(\d+)$/, " PR #$1") ?? "no PR selected"}`}</text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        {state.draft
          ? "Tab/j/k field · Enter edit/cycle · Ctrl+S save · Esc discard"
          : "j/k select · n add · e edit · d remove · Esc close"}
      </text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        Save stages a local draft. Publication requires a separate preview and approval.
      </text>
      {state.busy ? (
        <text fg={theme.warning} height={1} flexShrink={0}>
          Waiting for backend…
        </text>
      ) : null}
      {state.error ? (
        <text fg={theme.error} height={3} flexShrink={0}>
          {state.error}
        </text>
      ) : null}
      {state.notice ? (
        <text fg={theme.secondary} height={3} flexShrink={0}>
          {state.notice}
        </text>
      ) : null}
      {state.removing ? (
        <text
          fg={theme.warning}
          height={2}
          flexShrink={0}
        >{`Remove ${selected?.path || "general comment"} from this PR? y confirm · n/Esc cancel`}</text>
      ) : null}
      <scrollbox ref={scroll} flexGrow={1} minHeight={0}>
        {state.draft ? <DraftForm state={state} /> : <DraftList state={state} />}
      </scrollbox>
      <text fg={theme.muted} height={2} flexShrink={0}>
        {state.editing
          ? "Enter keeps field · Shift+Enter body newline · Ctrl+U clear · Esc undo field"
          : "Changes remain local. Imported identity, finding references and reviewed revisions are retained."}
      </text>
    </box>
  );
}

function DraftForm({ state }: { state: ReturnType<DraftStore["getSnapshot"]> }) {
  if (!state.draft) return null;
  return (
    <>
      <text
        fg={theme.secondary}
        height={1}
      >{`Source: ${state.draft.source ?? "imported"} · edits are not saved yet`}</text>
      {draftFields.map((field, index) => (
        <text
          id={`draft-field-${index}`}
          key={field}
          fg={index === state.field ? theme.primary : theme.text}
        >{`${index === state.field ? "›" : " "} ${fieldLabel[field]}: ${index === state.field && state.editing ? state.text : String(state.draft![field] ?? "(empty)")}`}</text>
      ))}
    </>
  );
}
function DraftList({ state }: { state: ReturnType<DraftStore["getSnapshot"]> }) {
  const selected = state.items[state.selected];
  const start = Math.max(0, state.selected - 8);
  const visible = state.items.slice(start, start + 20);
  return (
    <>
      <text
        fg={theme.muted}
      >{`${state.items.length} staged comments${state.items.length > 20 ? ` · showing ${start + 1}–${Math.min(start + 20, state.items.length)}` : ""}`}</text>
      {visible.map((item, index) => (
        <text
          key={item.localId}
          fg={start + index === state.selected ? theme.primary : theme.text}
          height={1}
          truncate
        >{`${start + index === state.selected ? "›" : " "} ${item.publicationMode ?? "legacy"}: ${item.path || "general comment"} ${item.parentId ? "(reply)" : ""}`}</text>
      ))}
      {selected ? (
        <>
          <text
            fg={theme.secondary}
          >{`${selected.publicationMode ?? "legacy"} · ${selected.source ?? "imported"}`}</text>
          {draftFields
            .filter((field) => field !== "publicationMode")
            .map((field) =>
              selected[field] != null ? (
                <text
                  key={`detail-${field}`}
                  fg={theme.text}
                >{`${fieldLabel[field]}: ${selected[field]}`}</text>
              ) : null,
            )}
          {selected.findingRef ? (
            <text
              fg={theme.muted}
            >{`Finding: ${selected.findingRef.findingId} · run ${selected.findingRef.reviewRunId} · fingerprint ${selected.findingRef.findingFingerprint}`}</text>
          ) : null}
          {selected.reviewBaseSha || selected.reviewHeadSha ? (
            <text
              fg={theme.muted}
            >{`Reviewed: ${selected.reviewBaseSha ?? "unknown"} → ${selected.reviewHeadSha ?? "unknown"}`}</text>
          ) : null}
        </>
      ) : null}
    </>
  );
}
