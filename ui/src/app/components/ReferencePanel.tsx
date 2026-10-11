import { CliRenderEvents, type ScrollBoxRenderable } from "@opentui/core";
import { useKeyboard, usePaste, useRenderer } from "@opentui/react";
import { useEffect, useRef, useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import { type ReferenceStore, referenceFields } from "../referenceStore";
import { theme } from "../theme";

export function ReferencePanel({
  references,
  client,
  onClose,
}: {
  references: ReferenceStore;
  client: BackendClient;
  onClose: () => void;
}) {
  const state = useSyncExternalStore(references.subscribe, references.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  const renderer = useRenderer();
  const draftOpen = state.draft !== null;
  useEffect(() => {
    if (!draftOpen) return;
    let previous = "";
    const followField = () => {
      const view = scroll.current;
      const id = `reference-field-${state.field}`;
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
      if (state.editing && referenceFields[state.field] === "body" && state.text)
        view.scrollTo(view.scrollHeight);
      else view.scrollChildIntoView(id);
    };
    renderer.on(CliRenderEvents.FRAME, followField);
    return () => {
      renderer.off(CliRenderEvents.FRAME, followField);
    };
  }, [renderer, state.field, draftOpen, state.text, state.editing]);
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
      if (key.name === "y") void references.remove(client);
      else if (key.name === "escape" || key.name === "n") references.cancel();
      return;
    }
    if (state.editing) {
      if (key.name === "escape") references.cancel();
      else if (key.ctrl && key.name === "u") references.clearField();
      else if (key.name === "backspace") references.backspace();
      else if (key.name === "return" && key.shift && referenceFields[state.field] === "body")
        references.append("\n");
      else if (key.name === "return") references.finishField();
      else if (!key.ctrl && !key.meta && !key.super && !key.option && key.sequence >= " ")
        references.append(key.sequence);
      return;
    }
    if (state.draft) {
      if (key.name === "escape") references.cancel();
      else if (key.ctrl && key.name === "s") void references.save(client);
      else if (key.name === "return") references.beginField();
      else if (["j", "down", "tab"].includes(key.name)) references.move(1);
      else if (["k", "up"].includes(key.name)) references.move(-1);
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (key.name === "escape" || key.name === "q") onClose();
    else if (key.name === "n") references.add();
    else if (key.name === "e" || key.name === "return") references.edit();
    else if (key.name === "d") references.requestRemoval();
    else if (["j", "down"].includes(key.name)) references.move(1);
    else if (["k", "up"].includes(key.name)) references.move(-1);
  });
  usePaste((event) => {
    event.preventDefault();
    if (state.editing) references.append(new TextDecoder().decode(event.bytes));
  });
  const selected = state.items[state.selected];
  const start = Math.max(0, state.selected - 8);
  const visible = state.items.slice(start, start + 20);
  return (
    <box flexDirection="column" flexGrow={1} paddingX={1} backgroundColor={theme.background}>
      <text
        fg={theme.primary}
        height={1}
        flexShrink={0}
        truncate
      >{`References · ${state.key?.slice("references.".length).replace(/\/(\d+)$/, " PR #$1") ?? "no PR selected"}`}</text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        {state.draft
          ? "Tab/j/k field · Enter edit/cycle · Ctrl+S save · Esc discard"
          : "j/k select · n add · e edit · d remove · Esc close"}
      </text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        Saved context is used by new PR reviews and fresh-session replies. This panel does not fetch
        URLs, read local paths or publish.
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
        >{`Remove ${selected?.title || selected?.type || "reference"} from this PR? y confirm · n/Esc cancel`}</text>
      ) : null}
      <scrollbox ref={scroll} flexGrow={1} minHeight={0}>
        {state.draft ? (
          <>
            <text
              fg={theme.secondary}
              height={1}
            >{`Source: ${state.draft.source} · edits are not saved yet`}</text>
            {referenceFields.map((field, index) => (
              <text
                id={`reference-field-${index}`}
                key={field}
                fg={index === state.field ? theme.primary : theme.text}
              >{`${index === state.field ? "›" : " "} ${field === "prId" ? "Related PR number" : field === "localPath" ? "Local context path" : field}: ${index === state.field && state.editing ? state.text : String(state.draft![field] ?? "(empty)")}`}</text>
            ))}
          </>
        ) : (
          <>
            <text
              fg={theme.muted}
            >{`${state.items.length} saved references${state.items.length > 20 ? ` · showing ${start + 1}–${Math.min(start + 20, state.items.length)}` : ""}`}</text>
            {visible.map((item, index) => (
              <text
                key={item.id}
                fg={start + index === state.selected ? theme.primary : theme.text}
                height={1}
                truncate
              >{`${start + index === state.selected ? "›" : " "} ${item.type}: ${item.title || item.key || item.url || "untitled"}`}</text>
            ))}
            {selected ? (
              <>
                <text fg={theme.secondary}>{`${selected.type} · ${selected.source}`}</text>
                {referenceFields
                  .filter((field) => field !== "type")
                  .map((field) =>
                    selected[field] != null ? (
                      <text
                        key={`detail-${field}`}
                        fg={theme.text}
                      >{`${field}: ${selected[field]}`}</text>
                    ) : null,
                  )}
              </>
            ) : null}
          </>
        )}
      </scrollbox>
      <text fg={theme.muted} height={2} flexShrink={0}>
        {state.editing
          ? "Enter keeps field · Shift+Enter body newline · Ctrl+U clear · Esc undo field"
          : "Changes remain local. Existing imported records and other PR scopes are retained."}
      </text>
    </box>
  );
}
