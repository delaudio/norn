import type { LocalComparisonForm } from "../localComparison";
import { LOCAL_LAYERS } from "../localComparison";
import { theme } from "../theme";
export function LocalComparisonPanel({
  form,
  width,
  height,
}: {
  form: LocalComparisonForm;
  width: number;
  height: number;
}) {
  const source = form.sources[form.sourceIndex];
  const base =
    form.baseIndex === 0
      ? "Working tree"
      : (form.options?.baseRefs[form.baseIndex - 1] ?? "Unavailable base");
  const sourceRows = Math.max(1, Math.min(width < 70 ? 1 : 4, height - 14));
  const start = Math.max(0, form.sourceIndex - sourceRows + 1);
  return (
    <box
      width={width}
      height={height}
      border
      borderColor={theme.border}
      padding={1}
      flexDirection="column"
    >
      <text fg={theme.primary}>LOCAL COMPARISON</text>
      <text fg={form.focus === "source" ? theme.primary : theme.muted}>
        Source · ↑/↓ choose · Tab next
      </text>
      {form.sources.slice(start, start + sourceRows).map((entry, index) => (
        <text
          key={JSON.stringify(entry.source)}
          height={1}
          wrapMode="none"
          truncate
          fg={start + index === form.sourceIndex ? theme.primary : theme.text}
        >
          {start + index === form.sourceIndex ? "› " : "  "}
          {entry.label}
        </text>
      ))}
      <text fg={theme.muted} height={1} wrapMode="none" truncate>
        {form.options
          ? `${form.options.repository.provider} ${form.options.repository.workspace}/${form.options.repository.repo} · ${form.options.branch ?? "Detached"}`
          : source?.label}
      </text>
      <text fg={form.focus === "base" ? theme.primary : theme.text} wrapMode="word">
        Comparison · {base}
      </text>
      <text fg={theme.muted} height={1} wrapMode="none" truncate>
        {form.baseIndex === 0
          ? "Tracked selected layers; untracked contents excluded"
          : "Committed branch changes; dirty layers excluded"}
      </text>
      <box flexDirection={width < 70 ? "column" : "row"} gap={width < 70 ? 0 : 1}>
        {LOCAL_LAYERS.map((layer, index) => (
          <text
            key={layer}
            fg={form.focus === "layers" && form.layerCursor === index ? theme.primary : theme.text}
          >
            {form.baseIndex === 0
              ? form.layers.includes(layer)
                ? "[✓]"
                : "[ ]"
              : layer === "committed"
                ? "[✓]"
                : "[–]"}{" "}
            {layer}
          </text>
        ))}
      </box>
      <box flexGrow={1} flexDirection="column">
        {form.loading ? <text>Loading native source options…</text> : null}
        {form.capturing ? <text>Capturing immutable local evidence…</text> : null}
        {form.error ? (
          <text fg={theme.error} wrapMode="word">
            {form.error}
          </text>
        ) : null}
      </box>
      <text fg={theme.muted}>↑/↓ choose · Tab field · Space layer</text>
      <text fg={theme.muted}>Enter capture · r refresh · Esc cancel</text>
    </box>
  );
}
