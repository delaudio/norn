import type { ScrollBoxRenderable } from "@opentui/core";
import { useKeyboard, usePaste } from "@opentui/react";
import { type RefObject, useEffect, useRef, useSyncExternalStore } from "react";
import type { RemoteComment } from "../../../../src/protocol/generated/protocol";
import type { PublicationStore } from "../publicationStore";
import { theme } from "../theme";

function usePublicationInput(
  publication: PublicationStore,
  onClose: () => void,
  onReply: (comment: RemoteComment) => void,
  state: ReturnType<PublicationStore["getSnapshot"]>,
  scroll: RefObject<ScrollBoxRenderable | null>,
) {
  useKeyboard((key) => {
    key.preventDefault();
    key.stopPropagation();
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (key.name === "pageup" || key.name === "pagedown") {
      const view = scroll.current;
      view?.scrollBy((key.name === "pagedown" ? 1 : -1) * Math.max(1, view.height - 2));
      return;
    }
    if (key.name === "escape" || key.name === "q") {
      void publication.cancel();
      if (!state.sending) onClose();
      return;
    }
    if (state.sending) {
      if (key.name === "n") void publication.cancel();
      return;
    }
    if (state.busy) return;
    if (state.preview) {
      if (key.name === "y") void publication.approve();
      else if (key.name === "n") void publication.cancel();
      return;
    }
    if (key.name === "t") void publication.show("threads");
    else if (key.name === "s") void publication.show("status");
    else if (key.name === "d") void publication.show("drafts");
    else if (["j", "down"].includes(key.name)) publication.move(1);
    else if (["k", "up"].includes(key.name)) publication.move(-1);
    else if (state.view === "drafts" && key.name === "space") publication.toggle();
    else if (state.view === "drafts" && key.name === "return") void publication.prepare();
    else if (state.view === "status" && key.name === "r") void publication.reconcile();
    else if (state.view === "threads" && key.name === "r") {
      const comment = state.comments[state.selected];
      if (
        comment &&
        !comment.deleted &&
        !comment.parentId &&
        !(state.scope?.provider === "github" && comment.mode === "general")
      )
        onReply(comment);
    }
  });
  usePaste((event) => event.preventDefault());
}
export function PublicationPanel({
  publication,
  onClose,
  onReply,
}: {
  publication: PublicationStore;
  onClose: () => void;
  onReply: (comment: RemoteComment) => void;
}) {
  const state = useSyncExternalStore(publication.subscribe, publication.getSnapshot);
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  usePublicationInput(publication, onClose, onReply, state, scroll);
  const detailIdentity = state.scope
    ? JSON.stringify([state.view, state.selected, state.preview?.approvalId])
    : null;
  useEffect(() => {
    if (detailIdentity !== null) scroll.current?.scrollTo(0);
  }, [detailIdentity]);
  return (
    <box flexDirection="column" flexGrow={1} paddingX={1} backgroundColor={theme.background}>
      <text
        fg={theme.primary}
        height={1}
        flexShrink={0}
        truncate
      >{`Publish comments · ${state.drafts.length} staged · ${state.scope ? `${state.scope.provider}:${state.scope.workspace}/${state.scope.repo} #${state.scope.prId}` : "No PR"}`}</text>
      <text fg={theme.muted} height={2} flexShrink={0}>
        {state.preview
          ? "Exact preview · y approve selected comments · n cancel · PgUp/PgDn read"
          : state.sending
            ? "Sending one item at a time · Esc/n stop remaining writes · in-flight results remain journaled"
            : "d drafts · Space select · Enter preview · s status · r reconcile/reply · t threads · Esc close"}
      </text>
      {state.busy ? (
        <text fg={theme.warning} height={1} flexShrink={0}>
          Waiting for backend…
        </text>
      ) : null}
      {state.error ? (
        <text fg={theme.error} flexShrink={0}>
          {state.error}
        </text>
      ) : null}
      {state.notice ? (
        <text fg={theme.secondary} flexShrink={0}>
          {state.notice}
        </text>
      ) : null}
      <scrollbox ref={scroll} flexGrow={1} scrollY>
        {state.results.map((result) => (
          <text
            key={result.entry.comment.localId}
            fg={result.entry.state === "published" ? theme.secondary : theme.warning}
            flexShrink={0}
          >{`${result.entry.comment.localId}: ${result.entry.state} · remote ${result.entry.remoteId ?? "unknown"} · local metadata ${result.localSaved ? "saved" : "retained/pending"} · draft ${result.draftRemoved ? "removed" : "retained"}`}</text>
        ))}

        {state.preview ? (
          <PublicationPreview items={state.preview.items} />
        ) : (
          <PublicationSelection state={state} />
        )}
      </scrollbox>
    </box>
  );
}

function PublicationPreview({
  items,
}: {
  items: NonNullable<ReturnType<PublicationStore["getSnapshot"]>["preview"]>["items"];
}) {
  return (
    <>
      {items.map((item) => (
        <box key={item.comment.localId} flexDirection="column" flexShrink={0} marginBottom={1}>
          <text
            fg={theme.primary}
          >{`${item.comment.mode} · ${item.comment.path || "General"} · ${item.comment.to ? `new ${item.comment.to}` : item.comment.from ? `old ${item.comment.from}` : "no line"} · parent ${item.comment.parentId ?? "none"}`}</text>
          <text
            fg={theme.muted}
          >{`Head ${item.comment.headSha} · Base ${item.comment.baseSha}`}</text>
          <text fg={theme.text}>{item.markdown.replace(/\r\n?/g, "\n")}</text>
        </box>
      ))}
    </>
  );
}
function publicationSelection(state: ReturnType<PublicationStore["getSnapshot"]>) {
  const draft = state.drafts[state.selected];
  const entry = state.journal[state.selected];
  const comment = state.comments[state.selected];
  const selectedDrafts = new Set(state.checked);
  const options =
    state.view === "drafts"
      ? state.drafts.map(
          (item) =>
            `${selectedDrafts.has(item.localId) ? "[x]" : "[ ]"} ${item.publicationMode ?? "auto"} · ${item.path || "General"} · ${item.localId}`,
        )
      : state.view === "status"
        ? state.journal.map(
            (item) =>
              `${item.state} · ${item.comment.localId} · remote ${item.remoteId ?? "unknown"}`,
          )
        : state.comments.map(
            (item) =>
              `${item.parentId ? "↳ " : ""}${item.authorName || item.authorId} · ${item.mode} · #${item.id}${item.deleted ? " (deleted)" : ""}`,
          );
  const start = Math.max(0, state.selected - 10);
  const ids =
    state.view === "drafts"
      ? state.drafts.map((draft) => draft.localId)
      : state.view === "status"
        ? state.journal.map((entry) => entry.comment.localId)
        : state.comments.map((comment) => `${comment.mode}:${comment.id}`);
  const raw =
    (state.view === "drafts"
      ? draft?.raw
      : state.view === "status"
        ? entry?.comment.raw
        : comment?.raw
    )?.replace(/\r\n?/g, "\n") ?? "";
  const thread =
    state.view === "threads" && comment
      ? `Path ${comment.path || "General"} · new ${comment.to ?? "—"} · old ${comment.from ?? "—"} · parent ${comment.parentId ?? "none"}${state.scope?.provider === "github" && comment.mode === "general" ? " · GitHub general comments do not support threaded replies" : ""}`
      : null;
  return { options, ids, raw, thread, start };
}
function PublicationSelection({ state }: { state: ReturnType<PublicationStore["getSnapshot"]> }) {
  const { options, ids, raw, thread, start } = publicationSelection(state);
  return (
    <>
      {options.slice(start, start + 21).map((option, relative) => {
        const index = start + relative;
        return (
          <text
            key={ids[index]}
            fg={index === state.selected ? theme.primary : theme.text}
          >{`${index === state.selected ? ">" : " "} ${option}`}</text>
        );
      })}
      {options.length > 21 ? (
        <text
          fg={theme.muted}
        >{`${start + 1}–${Math.min(options.length, start + 21)} of ${options.length} · j/k select`}</text>
      ) : null}
      {!options.length ? <text fg={theme.muted}>No items in this view.</text> : null}
      <text fg={theme.text} marginTop={1}>
        {raw}
      </text>
      {thread ? <text fg={theme.muted}>{thread}</text> : null}
    </>
  );
}
