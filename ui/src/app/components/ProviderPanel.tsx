import { type ScrollBoxRenderable, TextAttributes } from "@opentui/core";
import { useKeyboard, usePaste, useRenderer, useTerminalDimensions } from "@opentui/react";
import { useRef, useSyncExternalStore } from "react";
import type { PrRepositoryIdentity } from "../../../../src/protocol/generated/protocol";
import type { BackendClient } from "../../transport/backendClient";
import {
  type ProviderRow,
  type ProviderStore,
  providerPrKey,
  repositoryKey,
} from "../providerStore";
import { theme } from "../theme";

export function ProviderPanel({
  store,
  client,
  onClose,
  onWorkspace,
  onSettings,
  quit,
  browserAvailable = false,
}: {
  store: ProviderStore;
  client: BackendClient;
  onClose: () => void;
  onWorkspace: (
    repository: PrRepositoryIdentity,
    prId: number,
    title: string,
    state: string,
  ) => Promise<void>;
  onSettings: () => void;
  quit: () => void;
  browserAvailable?: boolean;
}) {
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const { width, height } = useTerminalDimensions();
  const renderer = useRenderer();
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  const openWorkspace = () => {
    const state = store.getSnapshot();
    const row = store.selectedRow();
    if (row?.kind !== "pr") return;
    const detail =
      state.detailKey === providerPrKey(row.group.repository, row.pr.id) ? state.detail : null;
    void onWorkspace(
      row.group.repository,
      row.pr.id,
      detail?.detail.title ?? row.pr.title,
      detail?.detail.state ?? row.pr.state,
    ).catch(() => store.notice("This PR could not be selected. Refresh or inspect Settings."));
  };
  useKeyboard((key) => {
    const state = store.getSnapshot();
    key.preventDefault();
    key.stopPropagation();
    if (key.ctrl && key.name === "c") {
      quit();
      return;
    }
    if (state.editing) {
      if (key.name === "escape") store.cancelQuery();
      else if (key.ctrl && key.name === "u") store.clearQueryText();
      else if (key.name === "backspace") store.backspace();
      else if (key.name === "return") store.applyQuery(client);
      else if (!key.ctrl && !key.meta && !key.super && !key.option && key.sequence >= " ")
        store.append(key.sequence);
      return;
    }
    if (key.name === "escape" || key.name === "f2") {
      if (state.detailOpen) store.closeDetail();
      else onClose();
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (key.name === "q") {
      quit();
      return;
    }
    if (key.name === "m") {
      onSettings();
      return;
    }
    if (key.name === "b") {
      if (browserAvailable) void store.openBrowser(client);
      else store.notice("Provider browser opening is unavailable with this backend.");
      return;
    }
    if (state.detailOpen) {
      if (key.name === "r") store.showDetail(client);
      else if (key.name === "return" || key.name === "space" || key.sequence === " ")
        openWorkspace();
      else if (key.name === "y" && state.detail)
        store.notice(
          renderer.copyToClipboardOSC52(state.detail.browserUrl)
            ? "Provider URL sent to the terminal clipboard. Paste to verify; terminal policies may block it."
            : "Terminal clipboard is unavailable.",
        );
      else if (["j", "down", "k", "up", "pagedown", "pageup"].includes(key.name)) {
        const amount = ["pagedown", "pageup"].includes(key.name)
          ? Math.max(1, (scroll.current?.height ?? 3) - 2)
          : 1;
        scroll.current?.scrollBy(["j", "down", "pagedown"].includes(key.name) ? amount : -amount);
      }
      return;
    }
    if (key.name === "tab") {
      store.cycleFocus();
      return;
    }
    if (key.sequence === "/") {
      store.beginQuery();
      return;
    }
    if (key.name === "r") {
      store.refresh(client);
      return;
    }
    if (key.name === "n") {
      store.loadMore(client);
      return;
    }
    if (key.name === "c") {
      store.toggleGroup();
      return;
    }
    const direction = ["right", "down", "j"].includes(key.name)
      ? 1
      : ["left", "up", "k"].includes(key.name)
        ? -1
        : 0;
    if (state.focus === "repository" && direction) store.cycleRepository(client, direction);
    else if (state.focus === "state" && direction) store.cycleFilter(client, direction);
    else if (state.focus === "author" && direction) store.cycleAuthor(client, direction);
    else if (state.focus === "query" && key.name === "return") store.beginQuery();
    else if (state.focus === "list") {
      if (direction) store.move(direction);
      else if (key.name === "space" || key.sequence === " ") openWorkspace();
      else if (key.name === "return") {
        const row = store.selectedRow();
        if (row?.kind === "pr") store.showDetail(client);
        else if (row?.kind === "page") store.loadMore(client);
        else store.toggleGroup();
      }
    }
  });
  usePaste((event) => {
    event.preventDefault();
    if (store.getSnapshot().editing) store.append(new TextDecoder().decode(event.bytes));
  });
  const row = store.selectedRow();
  const rows = store.rows();
  const selected = store.selectedIndex();
  const count = Math.max(
    1,
    Math.min(
      32,
      height -
        9 -
        (state.error ? 2 : 0) -
        (state.notice ? 2 : 0) -
        (row?.group.error ? 2 : 0) -
        (state.userErrors.length ? 1 : 0),
    ),
  );
  const start = Math.max(0, Math.min(selected - Math.floor(count / 2), rows.length - count));
  const visible = rows.slice(start, start + count);
  const repo = state.repositories.find((repo) => repositoryKey(repo) === state.repository);
  const field = (name: string, value: string, active: boolean) => (
    <text
      key={name}
      height={1}
      flexShrink={0}
      truncate
      fg={active ? theme.primary : theme.muted}
    >{`${active ? "›" : " "} ${name}: ${value}`}</text>
  );
  const heading = state.detailOpen
    ? `${row?.group.repository.provider ?? "Provider"} ${row?.group.repository.workspace ?? ""}/${row?.group.repository.repo ?? ""} PR #${row?.kind === "pr" ? row.pr.id : ""}`
    : "PULL REQUESTS · F2/Esc close · q quit";
  return (
    <box
      width="100%"
      height="100%"
      paddingX={1}
      flexDirection="column"
      backgroundColor={theme.background}
    >
      <text height={1} flexShrink={0} truncate fg={theme.primary} attributes={TextAttributes.BOLD}>
        {heading}
      </text>
      {state.detailOpen ? (
        <>
          <text height={2} flexShrink={0} fg={theme.muted}>
            Enter workspace · r refresh · y copy URL · b provider page · Esc list · arrows scroll
          </text>
          {state.detailLoading ? (
            <text height={1} flexShrink={0} fg={theme.warning}>
              Loading actual provider metadata…
            </text>
          ) : null}
          {state.detailError ? (
            <text height={3} flexShrink={0} fg={theme.error}>
              {state.detailError}
            </text>
          ) : null}
          <scrollbox
            id="provider-detail"
            ref={scroll}
            flexGrow={1}
            minHeight={0}
            scrollY
            scrollX={false}
          >
            {state.detail ? (
              <>
                <text flexShrink={0} fg={theme.text} attributes={TextAttributes.BOLD}>
                  {state.detail.detail.title}
                </text>
                <text
                  flexShrink={0}
                  fg={theme.secondary}
                >{`${state.detail.detail.state}${state.detail.detail.draft ? " · DRAFT" : ""}`}</text>
                <text
                  flexShrink={0}
                  fg={theme.text}
                >{`Author: ${state.detail.detail.authorDisplayName || "Unidentified"} · ${state.detail.authorId ?? "identity unavailable"}`}</text>
                <text
                  flexShrink={0}
                  fg={theme.text}
                >{`Branches: ${state.detail.detail.sourceBranch || "unavailable"} → ${state.detail.detail.destinationBranch || "unavailable"}`}</text>
                <text
                  flexShrink={0}
                  fg={theme.muted}
                >{`Head: ${state.detail.detail.sourceCommitHash ?? "unavailable"}\nBase: ${state.detail.detail.destinationCommitHash ?? "unavailable"}`}</text>
                <text
                  flexShrink={0}
                  fg={theme.muted}
                >{`Created: ${state.detail.detail.createdOn || "unavailable"}\nUpdated: ${state.detail.detail.updatedOn || "unavailable"}`}</text>
                <text flexShrink={0} fg={theme.primary}>
                  Reviewers / participants
                </text>
                {state.detail.detail.reviewers.length ? (
                  [
                    ...new Map(
                      state.detail.detail.reviewers.map((reviewer) => [
                        JSON.stringify(reviewer),
                        reviewer,
                      ]),
                    ).values(),
                  ].map((reviewer) => (
                    <text
                      key={JSON.stringify(reviewer)}
                      flexShrink={0}
                      fg={reviewer.approved ? theme.secondary : theme.text}
                    >{`${reviewer.displayName || reviewer.accountId || "Unidentified participant"} · ${reviewer.role || "participant"} · ${reviewer.approved ? "approved" : "not approved"}`}</text>
                  ))
                ) : (
                  <text flexShrink={0} fg={theme.muted}>
                    No reviewer approval evidence.
                  </text>
                )}
                <text flexShrink={0} fg={theme.primary}>
                  Description
                </text>
                <text flexShrink={0} fg={theme.text}>
                  {state.detail.detail.descriptionRaw || "No description."}
                </text>
                <text flexShrink={0} fg={theme.muted}>
                  {state.detail.browserUrl}
                </text>
                <text flexShrink={0} fg={theme.muted}>
                  Workspace opens an existing saved run when available. A PR without a reviewed
                  range never uses the working-tree diff.
                </text>
              </>
            ) : null}
          </scrollbox>
        </>
      ) : (
        <>
          {field(
            "Repository",
            repo ? `${repo.provider} ${repo.workspace}/${repo.repo}` : "All repositories",
            state.focus === "repository",
          )}
          {field("State", state.filter, state.focus === "state")}
          {field(
            "Author",
            state.author
              ? `${state.author.provider} ${state.author.label}${state.author.me ? " (me)" : ""}`
              : "All authors",
            state.focus === "author",
          )}
          {field(
            "Query",
            state.editing ? `${state.text}▌` : state.query || "Any · / edit",
            state.focus === "query",
          )}
          <text height={1} flexShrink={0} truncate fg={theme.muted}>
            {width >= 80
              ? "Tab field · ←/→ change filter · j/k select · Enter detail · c fold group"
              : "Tab fields · arrows/j/k · Enter detail"}
          </text>
          <text height={1} flexShrink={0} truncate fg={theme.muted}>
            {width >= 80
              ? "Space workspace · b provider page · n next page · r refresh · / query · m settings"
              : "Space select · b page · n more · r refresh · m settings"}
          </text>
          {state.error ? (
            <text height={2} flexShrink={0} fg={theme.error}>
              {state.error}
            </text>
          ) : null}
          {!state.repositories.length && !state.error ? (
            <text height={2} flexShrink={0} fg={theme.muted}>
              No provider repositories. Press m to configure one; imported PR scopes remain
              accessible in the workspace.
            </text>
          ) : null}
          <box flexGrow={1} minHeight={0} flexDirection="column">
            {visible.map((row, index) => (
              <text
                key={row.key}
                height={1}
                flexShrink={0}
                truncate
                fg={
                  start + index === selected
                    ? theme.primary
                    : row.kind === "repository"
                      ? theme.secondary
                      : theme.text
                }
              >{`${start + index === selected ? "›" : " "} ${rowLabel(row)}`}</text>
            ))}
          </box>
          <text
            height={1}
            flexShrink={0}
            truncate
            fg={theme.muted}
          >{`${state.groups.reduce((sum, g) => sum + g.values.length, 0)} loaded PRs · ${state.groups.filter((g) => g.loading).length} loading · ${state.groups.filter((g) => g.error).length} repository errors`}</text>
        </>
      )}
      {state.notice ? (
        <text height={2} flexShrink={0} fg={theme.warning}>
          {state.notice}
        </text>
      ) : null}
      {!state.detailOpen && row?.group.error ? (
        <text height={2} flexShrink={0} fg={theme.error}>
          {row.group.error}
        </text>
      ) : null}
      {!state.detailOpen && state.userErrors.length ? (
        <text height={1} flexShrink={0} truncate fg={theme.warning}>
          Current-user identity unavailable; author names are not treated as IDs.
        </text>
      ) : null}
    </box>
  );
}
function rowLabel(row: ProviderRow): string {
  if (row.kind === "repository")
    return `${row.group.collapsed ? "+" : "−"} ${row.group.repository.provider} ${row.group.repository.workspace}/${row.group.repository.repo} · ${row.group.values.length} loaded · p${row.group.page || 1}${row.group.loading ? " loading" : row.group.error ? " error" : row.group.values.length === 0 ? " no matches" : ""}`;
  if (row.kind === "page")
    return row.group.loading
      ? "Loading next page…"
      : `Next provider page ${row.group.nextPage} · Enter/n load`;
  return `#${row.pr.id} ${row.pr.draft ? "DRAFT " : ""}${row.pr.title} · ${row.pr.authorDisplayName || row.pr.authorAccountId || "unidentified author"}`;
}
