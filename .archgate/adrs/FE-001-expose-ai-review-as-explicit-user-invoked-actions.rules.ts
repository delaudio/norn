/// <reference path="../rules.d.ts" />

const ALLOWED_FILES = new Set([
  "src/components/review/ReviewActions.tsx",
  "src/mock-tauri/mock-handlers.ts",
]);

function extractInlineObjectArgument(source: string, start: number): string | null {
  let openingBrace = start;
  while (/\s/.test(source[openingBrace] ?? "")) openingBrace += 1;
  if (source[openingBrace] !== ",") return null;
  openingBrace += 1;
  while (/\s/.test(source[openingBrace] ?? "")) openingBrace += 1;
  if (source[openingBrace] !== "{") return null;

  let depth = 0;
  let quote: "'" | '"' | "`" | null = null;
  let escaped = false;

  for (let index = openingBrace; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === "'" || character === '"' || character === "`") {
      quote = character;
    } else if (character === "{") {
      depth += 1;
    } else if (character === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openingBrace, index + 1);
    }
  }

  return null;
}

export default {
  rules: {
    "claude-launch-remains-explicit": {
      description:
        "The native Claude launch command must only be referenced from the explicit review action surface",
      async check(ctx) {
        const files = [...(await ctx.glob("src/**/*.ts")), ...(await ctx.glob("src/**/*.tsx"))];

        for (const file of files) {
          if (ALLOWED_FILES.has(file)) continue;
          const matches = await ctx.grep(file, /\blaunch_claude_review\b/g);
          for (const match of matches) {
            ctx.report.violation({
              message:
                "Keep launch_claude_review confined to ReviewActions so AI review stays user-invoked and explicit.",
              file: match.file,
              line: match.line,
            });
          }
        }
      },
    },
    "gui-ai-review-skips-analyzers": {
      description:
        "GUI AI review entry points must skip repository analyzers already run by the development flow",
      async check(ctx) {
        const entryPoints = ["src/App.tsx", "src/hooks/useAiReview.ts"];

        await Promise.all(
          entryPoints.map(async (file) => {
            const source = await ctx.readFile(file);
            const callPattern = /tauriCall(?:<[^>]*>)?\s*\(\s*["']start_inline_review["']/g;
            const calls = [...source.matchAll(callPattern)];

            for (const call of calls) {
              const callIndex = call.index ?? 0;
              const argument = extractInlineObjectArgument(source, callIndex + call[0].length);
              if (!argument || !/\bskipAnalyzers\s*:\s*true\b/.test(argument)) {
                ctx.report.violation({
                  message: "Every GUI start_inline_review call must send skipAnalyzers: true.",
                  file,
                  line: source.slice(0, callIndex).split("\n").length,
                });
              }
            }
            if (calls.length === 0) {
              ctx.report.violation({
                message: "Expected this GUI AI review entry point to invoke start_inline_review.",
                file,
              });
            }
          }),
        );
      },
    },
  },
} satisfies RuleSet;
