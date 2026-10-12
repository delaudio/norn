import type { ShellSnapshot } from "../store";
import { theme } from "../theme";
export function SavedLocalPanel({
  form,
  width,
  height,
}: {
  form: NonNullable<ShellSnapshot["savedLocal"]>;
  width: number;
  height: number;
}) {
  const rows = Math.max(1, height - 12);
  const start = Math.max(0, form.cursor - rows + 1);
  const selected = form.items[form.cursor];
  return (
    <box
      width={width}
      height={height}
      border
      borderColor={theme.border}
      padding={1}
      flexDirection="column"
    >
      <text height={1} flexShrink={0} fg={theme.primary}>
        SAVED LOCAL SNAPSHOTS
      </text>
      <text height={1} flexShrink={0} fg={theme.muted}>
        Saved evidence opens without its checkout
      </text>
      <text height={1} flexShrink={0} fg={theme.muted}>
        Live review starts require the original source
      </text>
      <text
        height={1}
        flexShrink={0}
        fg={theme.muted}
      >{`Selected clone ${selected?.snapshot.rootId.slice(0, 16) ?? "—"}`}</text>
      <text
        height={1}
        flexShrink={0}
        fg={theme.muted}
      >{`Snapshot ${selected?.snapshot.snapshotId.slice(0, 16) ?? "—"}`}</text>
      <box height={rows} flexShrink={0} flexDirection="column" overflow="hidden">
        {form.loading ? <text>Loading verified saved snapshots…</text> : null}
        {!form.loading && form.total === 0 && !form.error ? (
          <text>No saved local snapshots</text>
        ) : null}
        {(!form.loading && !form.error ? form.items : [])
          .slice(start, start + rows)
          .map((item, index) => (
            <text
              key={JSON.stringify(item.snapshot)}
              height={1}
              flexShrink={0}
              wrapMode="none"
              truncate
              fg={start + index === form.cursor ? theme.primary : theme.text}
            >
              {start + index === form.cursor ? "› " : "  "}
              {item.snapshot.repository.provider} {item.snapshot.repository.workspace}/
              {item.snapshot.repository.repo}
              {` · ${item.branch} · ${item.layers.join("+")} · clone ${item.snapshot.rootId.slice(0, 12)} · snapshot ${item.snapshot.snapshotId.slice(0, 12)}`}
            </text>
          ))}
        {form.error ? (
          <text fg={theme.error} wrapMode="word">
            {form.error}
          </text>
        ) : null}
      </box>
      <text
        height={1}
        flexShrink={0}
        fg={theme.muted}
      >{`${form.total ? form.offset + 1 : 0}–${form.offset + form.items.length} of ${form.total}`}</text>
      <text height={1} flexShrink={0} fg={theme.muted}>
        ↑/↓ choose · PgUp/PgDn page · r refresh
      </text>
      <text height={1} flexShrink={0} fg={theme.muted}>
        Enter open saved snapshot · Esc close
      </text>
    </box>
  );
}
