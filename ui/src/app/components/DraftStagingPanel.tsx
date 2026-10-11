import type { ScrollBoxRenderable } from "@opentui/core";
import { useKeyboard, usePaste } from "@opentui/react";
import { useRef, useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import type { DraftStagingStore } from "../draftStagingStore";
import { theme } from "../theme";

export function DraftStagingPanel({
  staging,
  client,
  onClose,
  onDrafts,
}: {
  staging: DraftStagingStore;
  client: BackendClient;
  onClose: () => void;
  onDrafts: () => void;
}) {
  const state = useSyncExternalStore(staging.subscribe, staging.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  useKeyboard((key) => {
    key.preventDefault();
    key.stopPropagation();
    if (key.name === "escape" || (key.name === "q" && !key.ctrl)) {
      onClose();
      return;
    }
    if (state.busy || key.ctrl || key.meta || key.super || key.option) return;
    if (key.name === "d") {
      onDrafts();
      return;
    }
    if (["pagedown", "pageup"].includes(key.name)) {
      const view = scroll.current;
      view?.scrollBy((key.name === "pagedown" ? 1 : -1) * Math.max(1, view.height - 2));
    } else if (key.name === "j" || key.name === "down") staging.moveMode(1);
    else if (key.name === "k" || key.name === "up") staging.moveMode(-1);
    else if (key.name === "return") void staging.preview(client);
    else if (key.name === "y" && state.prepared) void staging.stage(client);
    else if (key.name === "n") staging.dismissPreview();
  });
  usePaste((event) => event.preventDefault());
  const draft = state.prepared?.draft;
  return (
    <box flexDirection="column" flexGrow={1} paddingX={1} backgroundColor={theme.background}>
      <text fg={theme.primary} height={1} flexShrink={0} truncate>
        {`Stage finding · ${state.target?.provider ?? "no provider"} · PR #${state.target?.prId ?? "?"}`}
      </text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        j/k mode · Enter preview/refresh · y stage preview · n dismiss · d drafts · Esc close
      </text>
      <text fg={theme.secondary} height={1} flexShrink={0}>{`Selected mode: ${state.mode}`}</text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        Local staging. Publication needs separate approval and remote checks.
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
      {state.prepared?.alreadyStaged ? (
        <text fg={theme.warning} height={2} flexShrink={0}>
          Already staged. Existing saved edits and mode are retained.
        </text>
      ) : null}
      {state.done ? (
        <text fg={theme.secondary} height={2} flexShrink={0}>
          Draft staging confirmed locally. Open drafts to edit it.
        </text>
      ) : null}
      <scrollbox ref={scroll} flexGrow={1} minHeight={0}>
        {draft ? (
          <>
            <text
              fg={theme.secondary}
            >{`Preview mode: ${draft.publicationMode ?? "imported"}`}</text>
            <text
              fg={theme.text}
            >{`Path: ${draft.path || "(general/local-only)"} · new-side ${draft.to ?? "none"} · old-side ${draft.from ?? "none"}`}</text>
            <text
              fg={theme.muted}
            >{`Finding: ${draft.findingRef?.findingId} · run ${draft.findingRef?.reviewRunId}`}</text>
            <text
              fg={theme.muted}
            >{`Reviewed: ${draft.reviewBaseSha ?? "unavailable"} → ${draft.reviewHeadSha ?? "unavailable"}`}</text>
            <text fg={theme.text}>{draft.raw.replace(/\r\n?/g, "\n")}</text>
          </>
        ) : (
          <text fg={theme.muted}>
            Choose inline, file, general or local-only, then prepare the actual saved finding text.
          </text>
        )}
      </scrollbox>
      <text fg={theme.muted} height={2} flexShrink={0}>
        PgUp/PgDn scroll. If a pending save is interrupted, inspect drafts before retrying.
      </text>
    </box>
  );
}
