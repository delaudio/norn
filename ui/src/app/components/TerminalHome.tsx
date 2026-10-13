/** biome-ignore-all lint/a11y/noStaticElementInteractions: OpenTUI box/text are native terminal renderables without DOM or ARIA roles; useKeyboard supplies keyboard navigation for every mouse action. */
import { type ScrollBoxRenderable, TextAttributes } from "@opentui/core";
import { useKeyboard, usePaste, useTerminalDimensions } from "@opentui/react";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { BackendClient } from "../../transport/backendClient";
import { type ProviderStore, providerPrKey } from "../providerStore";
import { type ShellSnapshot, type ShellStore, visibleFindings } from "../store";
import { theme } from "../theme";
import { TerminalMarkdown } from "./TerminalMarkdown";

const filters = ["OPEN", "DRAFT", "MERGED", "LOCAL"] as const;
type Focus = ShellSnapshot["terminalHomeFocus"];
const panes: Focus[] = ["filters", "list", "description", "review"];
type Action = "load" | "diff" | "browser" | "review";

// Keep terminal workflows together: top filters, review output on the right,
// PR list and independent description on the right.
export function TerminalHome({
  shell,
  providers,
  client,
  quit,
}: {
  shell: ShellStore;
  providers: ProviderStore;
  client: BackendClient;
  quit: () => void;
}) {
  const state = useSyncExternalStore(shell.subscribe, shell.getSnapshot);
  const remote = useSyncExternalStore(providers.subscribe, providers.getSnapshot);
  const { width, height } = useTerminalDimensions();
  const focus = state.terminalHomeFocus;
  const setFocus = (focus: Focus) => shell.setTerminalHomeFocus(focus);
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const revision = useRef(0);
  const list = useRef<ScrollBoxRenderable | null>(null);
  const review = useRef<ScrollBoxRenderable | null>(null);
  const description = useRef<ScrollBoxRenderable | null>(null);
  const local = state.terminalFilter === "LOCAL";
  const rows = providers.rows().filter((row) => row.kind !== "repository");
  const row = providers.selectedRow();
  const selection = local ? "local:current" : row?.kind === "pr" ? row.key : null;
  useEffect(
    () => () => {
      revision.current++;
    },
    [],
  );
  useEffect(() => {
    if (selection) list.current?.scrollChildIntoView(`home-${selection}`);
  }, [selection]);

  useEffect(() => {
    if (!local && selection && remote.detailKey !== selection) providers.showDetail(client, false);
  }, [client, providers, local, selection, remote.detailKey]);
  useEffect(() => {
    if (selection) {
      review.current?.scrollTo(0);
      description.current?.scrollTo(0);
    }
  }, [selection]);

  const changeFilter = (filter: ShellSnapshot["terminalFilter"]) => {
    if (
      pending.current ||
      shell.getSnapshot().settingsOpen ||
      shell.getSnapshot().terminalDiffPrompt
    )
      return;
    revision.current++;
    shell.setTerminalFilter(filter);
    if (filter !== "LOCAL") providers.setFilter(client, filter);
  };
  const move = (delta: number) => {
    if (local || !rows.length || pending.current) return;
    const index = rows.findIndex((item) => item.key === remote.selected);
    providers.selectRow(rows[(Math.max(0, index) + delta + rows.length) % rows.length]!.key);
  };
  const act = async (action: Action) => {
    if (pending.current) return;
    const row = providers.selectedRow();
    if (!local && row?.kind === "page") {
      providers.loadMore(client);
      return;
    }
    if (!local && row?.kind !== "pr") {
      providers.notice("Select a pull request first.");
      return;
    }
    if (action === "load" && !local) providers.showDetail(client, false);
    pending.current = true;
    setBusy(true);
    const request = ++revision.current;
    const selectedKey = providers.getSnapshot().selected;
    const filter = shell.getSnapshot().terminalFilter;
    const current = () =>
      request === revision.current &&
      filter === shell.getSnapshot().terminalFilter &&
      (local || providers.getSnapshot().selected === selectedKey) &&
      shell.getSnapshot().status !== "closed" &&
      !shell.getSnapshot().settingsOpen;
    try {
      if (local) {
        if (action === "load") {
          await shell.openLocalComparison(client, { kind: "current" });
          return;
        }
        if (
          shell.getSnapshot().picker[shell.getSnapshot().selectedPicker]?.kind !== "working-tree" ||
          shell.getSnapshot().localInfo?.comparison.source.kind !== "current"
        ) {
          await shell.openLocalComparison(client, { kind: "current" });
          if (!current()) return;
          await shell.applyLocalComparison(client);
          if (shell.getSnapshot().localForm || !shell.getSnapshot().localInfo) return;
        }
      } else if (row?.kind === "pr") {
        const selected = shell.getSnapshot().picker[shell.getSnapshot().selectedPicker];
        if (
          action === "load" ||
          shell.getSnapshot().error ||
          selected?.provider !== row.group.repository.provider ||
          selected.workspace !== row.group.repository.workspace ||
          selected.repo !== row.group.repository.repo ||
          selected.prId !== row.pr.id ||
          shell.getSnapshot().snapshotLoading
        ) {
          await shell.selectProviderPr(row.group.repository, row.pr.id, row.pr.title, row.pr.state);
        }
      }
      if (!current() || shell.getSnapshot().error || shell.getSnapshot().snapshotLoading) return;
      shell.promptTerminalDiff(false);
      if (action === "load" && !local) await shell.loadTerminalReview(client);
      else if (action === "diff") shell.showTerminalDiff();
      else if (action === "browser") await shell.openBrowser(client);
      else if (action === "review") await shell.startReview(client);
    } catch (error) {
      if (current()) shell.setError(error instanceof Error ? error.message : String(error));
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };
  const dismissDiffPrompt = () => {
    revision.current++;
    shell.promptTerminalDiff(false);
  };
  useKeyboard((key) => {
    const state = shell.getSnapshot();
    if (
      key.defaultPrevented ||
      state.settingsOpen ||
      !state.terminalHomeOpen ||
      state.localForm ||
      state.savedLocal
    )
      return;
    key.preventDefault();
    key.stopPropagation();
    if (key.ctrl && key.name === "c") {
      quit();
      return;
    }
    if (key.ctrl || key.meta || key.super || key.option) return;
    if (state.terminalDiffPrompt) {
      if (key.name === "escape" || key.name === "q") dismissDiffPrompt();
      else if (key.name === "g") void act("diff");
      else if (key.name === "b") void act("browser");
      return;
    }
    if (key.name === "q") {
      quit();
      return;
    }
    if (key.name === "s" || key.name === "m") {
      if (!pending.current) shell.toggleSettings();
      return;
    }
    if (key.name === "f8") {
      if (!pending.current) shell.toggleExplorer();
      return;
    }
    if (key.name === "f7") {
      if (!pending.current) void shell.openSavedLocal(client);
      return;
    }
    if (key.name === "f6") {
      if (!pending.current) void shell.openReviewProfiles(client);
      return;
    }
    if (key.name === "f" || key.name === "left" || key.name === "right") {
      const delta = key.name === "left" ? -1 : 1;
      changeFilter(
        filters[(filters.indexOf(state.terminalFilter) + delta + filters.length) % filters.length]!,
      );
      return;
    }
    if (key.name === "tab") {
      const delta = key.shift ? -1 : 1;
      setFocus(panes[(panes.indexOf(focus) + delta + panes.length) % panes.length]!);
      return;
    }
    if (["j", "down", "k", "up", "pagedown", "pageup"].includes(key.name)) {
      const delta = ["j", "down", "pagedown"].includes(key.name) ? 1 : -1;
      if (focus === "filters")
        changeFilter(
          filters[
            (filters.indexOf(state.terminalFilter) + delta + filters.length) % filters.length
          ]!,
        );
      else if (focus === "review" || focus === "description") {
        const pane = focus === "review" ? review.current : description.current;
        pane?.scrollBy(
          delta * (key.name.startsWith("page") ? Math.max(1, (pane.height ?? 5) - 2) : 1),
        );
      } else move(delta);
      return;
    }
    if (key.name === "g") {
      if (!pending.current) shell.promptTerminalDiff(true);
    } else if (key.name === "b") void act("browser");
    else if (key.name === "return") void act("load");
    else if (key.name === "space" || key.sequence === " ") void act("diff");
    else if (key.name === "a") void act("review");
    else if (key.name === "x") void shell.cancel(client);
    else if (key.name === "r") {
      if (!pending.current) {
        if (local) void shell.openLocalComparison(client, { kind: "current" });
        else providers.refresh(client);
      }
    } else if (key.name === "n" && !local) providers.loadMore(client);
  });
  usePaste((event) => {
    if (!shell.getSnapshot().settingsOpen) event.preventDefault();
  });

  const mouseAvailable = () =>
    !shell.getSnapshot().settingsOpen &&
    !shell.getSnapshot().terminalDiffPrompt &&
    !pending.current;
  const focusPane = (pane: Focus) => {
    if (mouseAvailable()) setFocus(pane);
  };
  const select = (key: string) => {
    if (mouseAvailable()) {
      providers.selectRow(key);
      setFocus("list");
      void act("load");
    }
  };
  const entry = state.picker[state.selectedPicker];
  const loaded = local
    ? !!state.localInfo && entry?.kind === "working-tree"
    : row?.kind === "pr" && !!entry && providerPrKey(entry, entry.prId ?? 0) === row.key;
  const findings = loaded ? (state.terminalReview?.findings ?? visibleFindings(state)) : [];
  const reviewWidth = Math.max(18, Math.floor(width * 0.4));
  const repo = state.repositories[state.selected];
  const active = state.operation;
  return (
    <box width="100%" height="100%" flexDirection="column" backgroundColor={theme.background}>
      <box
        height={3}
        flexShrink={0}
        paddingX={1}
        justifyContent="center"
        backgroundColor={theme.panel}
      >
        <text
          fg={theme.primary}
          attributes={TextAttributes.BOLD}
          truncate
        >{`Norn · ${repo?.workspace ?? ""}/${repo?.repo ?? ""}`}</text>
      </box>
      <box
        id="home-filters"
        height={1}
        flexShrink={0}
        flexDirection="row"
        backgroundColor={theme.panel}
        marginBottom={1}
        onMouseDown={() => focusPane("filters")}
      >
        {filters.map((filter) => (
          <text
            id={`home-filter-${filter}`}
            key={filter}
            height={1}
            width={filter.length + 4}
            fg={state.terminalFilter === filter ? theme.primary : theme.muted}
            bg={state.terminalFilter === filter ? theme.element : theme.panel}
            attributes={
              state.terminalFilter === filter
                ? TextAttributes.BOLD | (focus === "filters" ? TextAttributes.UNDERLINE : 0)
                : 0
            }
            onMouseDown={() => {
              focusPane("filters");
              changeFilter(filter);
            }}
          >{` ${state.terminalFilter === filter ? "●" : " "} ${filter[0]}${filter.slice(1).toLowerCase()} `}</text>
        ))}
      </box>
      <box flexGrow={1} minHeight={0} flexDirection="row">
        <box id="home-center" flexGrow={1} minWidth={0} flexDirection="column">
          <box
            id="home-pr-list"
            height={Math.max(5, Math.floor((height - 6) * 0.34))}
            flexShrink={0}
            border
            borderColor={focus === "list" ? theme.primary : theme.muted}
            flexDirection="column"
            onMouseDown={() => focusPane("list")}
          >
            <text fg={theme.primary} height={1} truncate>
              {local ? "LOCAL CHANGES" : "PULL REQUESTS"}
            </text>
            <scrollbox ref={list} flexGrow={1} minHeight={0} scrollY>
              {local ? (
                <text
                  id="home-local:current"
                  onMouseDown={(event) => {
                    event.stopPropagation();
                    if (event.button === 0 && mouseAvailable()) {
                      setFocus("list");
                      void act("load");
                    }
                  }}
                  fg={theme.primary}
                  height={1}
                  truncate
                >
                  ▶ Working tree · Enter configure base/layers
                </text>
              ) : (
                <>
                  {remote.groups.some((group) => group.loading) && (
                    <text fg={theme.muted} height={1}>
                      Loading pull requests…
                    </text>
                  )}
                  {remote.error && <text fg={theme.error}>{remote.error}</text>}
                  {remote.groups
                    .filter((group) => group.error)
                    .map((group) => (
                      <text key={JSON.stringify(group.repository)} fg={theme.error}>
                        {group.error}
                      </text>
                    ))}
                  {!rows.length &&
                    !remote.groups.some((group) => group.loading) &&
                    !remote.error &&
                    !remote.groups.some((group) => group.error) && (
                      <text fg={theme.muted}>No pull requests in this filter.</text>
                    )}
                  {rows.map((item) => (
                    <text
                      id={`home-${item.key}`}
                      key={item.key}
                      height={1}
                      truncate
                      fg={item.key === remote.selected ? theme.primary : theme.text}
                      onMouseDown={() => select(item.key)}
                    >{`${item.key === remote.selected ? "▶ " : "  "}${item.kind === "pr" ? `#${item.pr.id} ${item.pr.title}` : "Next page · Enter/n"}`}</text>
                  ))}
                </>
              )}
            </scrollbox>
          </box>
          <box
            id="home-description"
            flexGrow={1}
            minHeight={0}
            border
            borderColor={focus === "description" ? theme.primary : theme.muted}
            flexDirection="column"
            onMouseDown={() => focusPane("description")}
          >
            <text fg={theme.primary} height={1}>
              {local ? "LOCAL COMPARISON" : "PR DESCRIPTION"}
            </text>
            <scrollbox
              id="home-description-scroll"
              contentOptions={{ paddingRight: 1 }}
              ref={description}
              flexGrow={1}
              minHeight={0}
              scrollY
            >
              <text width="100%" wrapMode="word" fg={theme.text}>
                {local
                  ? "Current repository · tracked local changes"
                  : row?.kind === "pr"
                    ? `#${row.pr.id} ${row.pr.title}\n${row.pr.sourceBranch} → ${row.pr.destinationBranch}`
                    : "Select a review target."}
              </text>
              {!local &&
                row?.kind === "pr" &&
                (remote.detailKey === row.key && remote.detailLoading ? (
                  <text width="100%" wrapMode="word" fg={theme.muted}>
                    Loading PR description…
                  </text>
                ) : remote.detailKey === row.key && remote.detailError ? (
                  <text
                    fg={theme.error}
                  >{`PR description unavailable: ${remote.detailError} · Enter retry`}</text>
                ) : remote.detailKey === row.key && remote.detail ? (
                  remote.detail.detail.descriptionRaw.trim() ? (
                    <TerminalMarkdown content={remote.detail.detail.descriptionRaw} />
                  ) : (
                    <text fg={theme.muted}>No PR description.</text>
                  )
                ) : (
                  <text width="100%" wrapMode="word" fg={theme.muted}>
                    Loading PR description…
                  </text>
                ))}
              {local && (
                <text width="100%" wrapMode="word" fg={theme.muted}>
                  Enter configures the base and tracked layers. Use g for the diff or a to start a
                  review.
                </text>
              )}
            </scrollbox>
          </box>
        </box>
        <box
          id="home-review"
          width={reviewWidth}
          flexShrink={0}
          minHeight={0}
          border
          borderColor={focus === "review" ? theme.primary : theme.muted}
          flexDirection="column"
          onMouseDown={() => focusPane("review")}
        >
          <text fg={theme.primary} height={1}>
            AI REVIEW
          </text>
          <scrollbox id="home-review-scroll" ref={review} flexGrow={1} minHeight={0} scrollY>
            {busy || state.snapshotLoading ? (
              <text width="100%" wrapMode="word" fg={theme.muted}>
                Loading selected evidence…
              </text>
            ) : !loaded ? (
              <text width="100%" wrapMode="word" fg={theme.muted}>
                Enter load selection · a start AI review
              </text>
            ) : (
              <text
                fg={theme.muted}
              >{`${state.files.length} changed files · ${state.terminalReview?.findings === null ? "findings unavailable" : `${findings.length} findings`} · ${state.history.length} saved runs`}</text>
            )}
            {active && (
              <>
                <text
                  fg={active.state === "failed" ? theme.error : theme.secondary}
                >{`Review ${active.state} · ${active.targetLabel}`}</text>
                {active.logs.map((log) => (
                  <text width="100%" wrapMode="word" key={log.id} fg={theme.muted}>
                    {log.text}
                  </text>
                ))}
              </>
            )}
            {loaded && state.terminalReview && (
              <>
                <text
                  width="100%"
                  wrapMode="word"
                  fg={theme.muted}
                >{`Saved AI review · ${state.terminalReview.runId}`}</text>
                {state.terminalReview.output === null ? (
                  <text fg={theme.muted}>
                    Stored output unavailable; see findings and status below.
                  </text>
                ) : (
                  <TerminalMarkdown content={state.terminalReview.output} />
                )}
              </>
            )}
            {findings.map((finding) => (
              <box key={finding.id} flexDirection="column" flexShrink={0}>
                <text
                  width="100%"
                  wrapMode="word"
                  fg={theme.primary}
                >{`${finding.severity} · ${finding.title}`}</text>
                <TerminalMarkdown content={finding.summary} />
              </box>
            ))}
            {state.error && (
              <text width="100%" wrapMode="word" fg={theme.error}>
                {state.error}
              </text>
            )}
            {state.notice && (
              <text width="100%" wrapMode="word" fg={theme.muted}>
                {state.notice}
              </text>
            )}
            {remote.notice && (
              <text width="100%" wrapMode="word" fg={theme.muted}>
                {remote.notice}
              </text>
            )}
          </scrollbox>
        </box>
      </box>
      <box height={2} flexShrink={0} flexDirection="column">
        <text height={1} fg={theme.muted} truncate>
          f filter · Tab pane · j/k move · Enter load · a AI review · g diff · b web diff · s
          settings · q quit
        </text>
        <box height={1} flexDirection="row">
          {[
            { label: "AI", run: () => void act("review") },
            { label: "Diff", run: () => shell.promptTerminalDiff(true) },
            { label: "Settings", run: () => shell.toggleSettings() },
            {
              label: "Refresh",
              run: () =>
                local
                  ? void shell.openLocalComparison(client, { kind: "current" })
                  : providers.refresh(client),
            },
            { label: "Files", run: () => shell.toggleExplorer() },
            { label: "Quit", run: quit },
          ].map((action) => (
            <text
              id={`home-action-${action.label}`}
              key={action.label}
              width={action.label.length + 2}
              fg={theme.primary}
              bg={theme.element}
              onMouseDown={(event) => {
                event.stopPropagation();
                if (event.button === 0 && mouseAvailable()) action.run();
              }}
            >{` ${action.label} `}</text>
          ))}
        </box>
      </box>
      {state.terminalDiffPrompt && (
        <box
          position="absolute"
          left={Math.max(0, Math.floor((width - Math.min(64, width - 2)) / 2))}
          top={Math.max(1, Math.floor(height / 2) - 3)}
          width={Math.min(64, width - 2)}
          height={6}
          border
          borderColor={theme.primary}
          backgroundColor={theme.panel}
          paddingX={1}
          flexDirection="column"
        >
          <text fg={theme.primary}>Open diff</text>
          <text
            id="home-choice-native"
            fg={theme.text}
            onMouseDown={(event) => {
              event.stopPropagation();
              if (event.button === 0 && !pending.current) void act("diff");
            }}
          >
            g · Native terminal Git diff
          </text>
          <text
            id="home-choice-browser"
            fg={theme.text}
            onMouseDown={(event) => {
              event.stopPropagation();
              if (event.button === 0 && !pending.current) void act("browser");
            }}
          >
            b · Browser diff viewer
          </text>
          <text
            id="home-choice-cancel"
            fg={theme.muted}
            onMouseDown={(event) => {
              event.stopPropagation();
              if (event.button === 0) dismissDiffPrompt();
            }}
          >
            Esc · Cancel
          </text>
        </box>
      )}
    </box>
  );
}
