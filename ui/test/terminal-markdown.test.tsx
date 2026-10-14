import { expect, test } from "bun:test";
import { TextAttributes } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { createRoot, flushSync } from "@opentui/react";
import { TerminalMarkdown } from "../src/app/components/TerminalMarkdown";
import { setTheme, theme } from "../src/app/theme";

for (const width of [100, 32]) {
  test(`native Markdown conceals markers and follows dark/light colors at ${width} columns`, async () => {
    const renderer = await createTestRenderer({ width, height: 30 });
    const root = createRoot(renderer.renderer);
    const content =
      "# Heading\n\n**Major** normal and `inlineCode`.\n\n[Docs](https://example.invalid)\n\n```unknown-fence-language\nconst owned = true;\n```\n\n- First item\n- Second item";
    try {
      for (const mode of ["dark", "light"] as const) {
        setTheme(mode);
        flushSync(() => root.render(<TerminalMarkdown content={content} />));
        const deadline = Date.now() + 3000;
        const rgb = (color: string) =>
          color
            .slice(1)
            .match(/../g)!
            .map((part) => Number.parseInt(part, 16))
            .join(",");
        const colored = (text: string, color: string) =>
          renderer
            .captureSpans()
            .lines.flatMap((line) => line.spans)
            .some(
              (span) =>
                span.text.includes(text) && span.fg.toInts().slice(0, 3).join(",") === rgb(color),
            );
        while (
          (!colored("Heading", theme.primary) ||
            !renderer.captureCharFrame().includes("Second item")) &&
          Date.now() < deadline
        ) {
          await Bun.sleep(1);
          await renderer.renderOnce();
        }
        await renderer.renderOnce();
        const frame = renderer.captureCharFrame();
        expect(frame).toContain("Heading");
        expect(frame).toContain("const owned = true;");
        expect(frame).toContain("First item");
        expect(frame).not.toContain("**Major**");
        expect(frame).not.toContain("`inlineCode`");
        expect(frame).not.toContain("unknown-fence-language");
        expect(colored("Heading", theme.primary)).toBe(true);
        expect(colored("Major", theme.warning)).toBe(true);
        expect(colored("inlineCode", theme.secondary)).toBe(true);
        expect(colored("Docs", theme.primary)).toBe(true);
        expect(
          renderer
            .captureSpans()
            .lines.flatMap((line) => line.spans)
            .some(
              (span) =>
                span.text.includes("Major") && (span.attributes & TextAttributes.BOLD) !== 0,
            ),
        ).toBe(true);
      }
    } finally {
      flushSync(() => root.unmount());
      renderer.renderer.destroy();
      setTheme("dark");
    }
  });
}
