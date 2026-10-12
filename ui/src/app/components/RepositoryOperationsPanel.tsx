import type { ScrollBoxRenderable } from "@opentui/core";
import { useKeyboard, usePaste } from "@opentui/react";
import { useRef, useSyncExternalStore } from "react";
import { explorerLabel } from "../explorerStore";
import type { RepositoryOperationsStore } from "../repositoryOperationsStore";
import { theme } from "../theme";

export function RepositoryOperationsPanel({
  store,
  width,
  height,
  onClose,
  quit,
}: {
  store: RepositoryOperationsStore;
  width: number;
  height: number;
  onClose: () => void;
  quit: () => void;
}) {
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const details = useRef<ScrollBoxRenderable | null>(null);

  useKeyboard((key) => {
    if (key.defaultPrevented) return;
    key.preventDefault();
    key.stopPropagation();
    if (key.ctrl && key.name === "c") {
      if (state.busy) void store.cancel();
      else quit();
      return;
    }
    if (
      key.ctrl &&
      !key.meta &&
      !key.super &&
      !key.option &&
      (key.name === "d" || key.name === "u")
    ) {
      details.current?.scrollBy((key.name === "d" ? 1 : -1) * Math.max(1, height - 6));
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (state.busy) {
      if (key.name === "x") void store.cancel();
      return;
    }
    if (state.loading) return;
    if (state.prepared) {
      if (key.name === "y" && key.sequence.toLowerCase() === "y") void store.confirm();
      else if (key.name === "escape" || key.name === "n") store.dismiss();
      return;
    }
    if (key.name === "escape" || key.name === "b") onClose();
    else if (key.name === "q") quit();
    else if (key.name === "r") void store.refresh();
    else if (key.name === "j" || key.name === "down") store.move(1);
    else if (key.name === "k" || key.name === "up") store.move(-1);
    else if (key.name === "pagedown" || key.name === "pageup")
      void store.page(key.name === "pagedown" ? 1 : -1);
    else if (key.name === "return") void store.checkout();
    else if (key.name === "f") void store.preview({ kind: "fetch" });
    else if (key.name === "p") void store.preview({ kind: "pull" });
    else if (key.name === "s") void store.preview({ kind: "stash" });
  });
  usePaste((event) => {
    event.preventDefault();
  });
  const checkout = state.checkout;
  const rows = Math.max(1, height - 8),
    start = Math.max(0, state.cursor - rows + 1);
  return (
    <box
      width={width}
      height={height}
      flexDirection="column"
      paddingX={1}
      backgroundColor={theme.background}
    >
      <text
        fg={theme.primary}
        height={1}
        truncate
      >{`BRANCHES · ${state.repository ? `${state.repository.provider} ${explorerLabel(state.repository.workspace)}/${explorerLabel(state.repository.repo)}` : "No repository"}`}</text>
      <text
        height={1}
        fg={theme.muted}
        truncate
      >{`clone ${checkout?.context?.rootId.slice(0, 12) ?? "unavailable"} · ${checkout?.state ?? "loading"} · ${explorerLabel(checkout?.branch ?? "detached")} · HEAD ${checkout?.headSha?.slice(0, 12) ?? "unborn"}`}</text>
      <text
        height={1}
        fg={
          checkout?.dirty || checkout?.conflicted || checkout?.pending ? theme.warning : theme.muted
        }
        truncate
      >
        {checkout?.context
          ? `${checkout.dirty ? "Local changes" : "Clean"} · ${checkout.conflicted ? "conflicts" : "no conflicts"} · ${checkout.pending ? "Git operation pending" : "no pending Git operation"} · upstream ${explorerLabel(checkout.upstream ?? "none")}`
          : "Working state unavailable · configure a valid local clone"}
      </text>
      <scrollbox
        key={JSON.stringify([
          state.prepared?.token,
          state.operation?.operationId,
          state.offset,
          state.cursor,
        ])}
        ref={details}
        flexGrow={1}
        minHeight={0}
      >
        {state.loading ? <text>Reading verified repository state…</text> : null}
        {state.prepared ? (
          <>
            <text fg={theme.warning}>
              CONFIRM GIT OPERATION · single-use approval expires in 30 seconds
            </text>
            <text>{explorerLabel(state.prepared.summary)}</text>
            {state.prepared.warnings.map((warning) => (
              <text key={warning} fg={theme.warning}>
                {explorerLabel(warning)}
              </text>
            ))}
            <text fg={theme.primary}>y approve · n/Esc dismiss · pasted text cannot approve</text>
          </>
        ) : state.busy ? (
          <text fg={theme.warning}>
            {explorerLabel(state.operation?.message ?? "Submitting the approved operation…")}
          </text>
        ) : (
          <>
            {checkout?.message ? (
              <text fg={theme.warning}>{explorerLabel(checkout.message)}</text>
            ) : null}
            {!state.branches.length && !state.loading ? (
              <text>No branch references available</text>
            ) : null}
            {state.branches.slice(start, start + rows).map((branch, index) => (
              <text
                key={branch.reference}
                height={1}
                truncate
                fg={start + index === state.cursor ? theme.primary : theme.text}
              >{`${start + index === state.cursor ? "›" : " "}${branch.current ? "*" : " "} ${branch.kind} ${explorerLabel(branch.name)} · ${branch.oid.slice(0, 12)}`}</text>
            ))}
          </>
        )}
        {state.operation ? (
          <>
            <text
              fg={state.operation.state === "completed" ? theme.secondary : theme.warning}
            >{`${state.operation.action.kind} ${state.operation.state} · exit ${state.operation.exitCode ?? "unknown"} · ${explorerLabel(state.operation.message)}`}</text>
            {state.operation.recoveryRequired ? (
              <text fg={theme.warning}>
                Inspect actual state before continuing. No automatic retry, rollback or replay.
              </text>
            ) : null}
            {state.operation.after?.stashSha ? (
              <text>{`Current stash ${state.operation.after.stashSha}`}</text>
            ) : null}
          </>
        ) : null}
        {state.error ? <text fg={theme.error}>{explorerLabel(state.error)}</text> : null}
      </scrollbox>
      <text
        height={1}
        fg={theme.muted}
        truncate
      >{`${state.total ? state.offset + 1 : 0}–${state.offset + state.branches.length}/${state.total} · PgUp/PgDn · j/k select`}</text>
      <text height={1} fg={theme.muted} truncate>
        {state.busy
          ? `${state.cancelRequested ? "Cancel requested · waiting for observed outcome" : "x/Ctrl-C cancel owned operation"} · navigation waits for completion`
          : "Enter preview checkout · f fetch · p pull · s stash"}
      </text>
      <text height={1} fg={theme.muted} truncate>
        Ctrl-D/U details · r refresh · b/Esc files · q quit
      </text>
    </box>
  );
}
