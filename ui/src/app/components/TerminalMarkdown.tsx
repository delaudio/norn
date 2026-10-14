import { SyntaxStyle } from "@opentui/core";
import { useEffect, useMemo } from "react";
import { theme } from "../theme";

/** Native Markdown shares the active OpenTUI palette, including theme changes. */
export function TerminalMarkdown({
  content,
  palette = theme,
}: {
  content: string;
  palette?: typeof theme;
}) {
  const style = useMemo(
    () =>
      SyntaxStyle.fromStyles({
        default: { fg: palette.text },
        "markup.heading": { fg: palette.primary, bold: true },
        ...Object.fromEntries(
          [1, 2, 3, 4, 5, 6].map((level) => [
            `markup.heading.${level}`,
            { fg: palette.primary, bold: true },
          ]),
        ),
        "markup.strong": { fg: palette.warning, bold: true },
        "markup.italic": { fg: palette.text, italic: true },
        "markup.raw": { fg: palette.secondary },
        "markup.link": { fg: palette.primary, underline: true },
        "markup.link.label": { fg: palette.primary, underline: true },
        "markup.link.url": { fg: palette.muted },
        "markup.list": { fg: palette.primary },
        "markup.quote": { fg: palette.muted, italic: true },
      }),
    [palette],
  );
  useEffect(() => () => style.destroy(), [style]);
  // As in the native explorer, unknown fence languages must not fetch grammar
  // assets while displaying remote text. Keep code content, render fences plain.
  const safeContent = content.replace(/^(\s*)(`{3,}|~{3,})[^\n]*$/gm, "$1$2");
  return (
    <markdown
      width="100%"
      flexShrink={0}
      content={safeContent}
      syntaxStyle={style}
      fg={palette.text}
      conceal
    />
  );
}
