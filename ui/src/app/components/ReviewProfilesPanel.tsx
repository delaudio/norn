import type { ShellSnapshot } from "../store";
import { theme } from "../theme";

export function ReviewProfilesPanel({
  state,
  width,
  height,
}: {
  state: ShellSnapshot;
  width: number;
  height: number;
}) {
  const catalog = state.profileCatalog;
  const options = catalog
    ? [`Repository default (${catalog.defaultProfile ?? "base"})`, ...catalog.profiles]
    : [];
  const selected = options[state.profileCursor] ?? "";
  // Reserve enough cells for the full selected name, including wide Unicode.
  const detailRows = selected
    ? Math.ceil((Array.from(selected).length * 2 + 2) / Math.max(1, width - 4))
    : 0;
  const rows = Math.max(1, height - 10 - detailRows);
  const start = Math.max(0, state.profileCursor - rows + 1);
  const target = state.profileTarget;
  const owner =
    target?.kind === "provider"
      ? `${target.provider} ${target.workspace}/${target.repo}${target.prId ? ` #${target.prId}` : ""}`
      : "Local repository";
  return (
    <box
      width={width}
      height={height}
      flexDirection="column"
      padding={1}
      border
      borderColor={theme.border}
    >
      <text fg={theme.primary}>REVIEW PROFILES</text>
      <text fg={theme.muted}>{owner}</text>
      {state.profileLoading ? <text>Loading repository policy…</text> : null}
      {state.profileError ? <text fg={theme.error}>{state.profileError}</text> : null}
      {catalog?.notice ? <text fg={theme.muted}>{catalog.notice}</text> : null}
      <box flexGrow={1} flexDirection="column">
        {options.slice(start, start + rows).map((name, index) => (
          <text
            key={start + index === 0 ? "repository-default" : `profile:${name}`}
            height={1}
            wrapMode="none"
            truncate
            fg={start + index === state.profileCursor ? theme.primary : theme.text}
          >
            {start + index === state.profileCursor ? "› " : "  "}
            {name}
          </text>
        ))}
      </box>
      {selected ? (
        <text fg={theme.text} wrapMode="word" flexShrink={0}>
          {selected}
        </text>
      ) : null}
      <text fg={theme.muted}>
        {options.length ? `${state.profileCursor + 1}/${options.length} · ` : ""}↑/↓ select · Enter
        choose
      </text>
      <text fg={theme.muted}>r refresh · Esc cancel</text>
    </box>
  );
}
