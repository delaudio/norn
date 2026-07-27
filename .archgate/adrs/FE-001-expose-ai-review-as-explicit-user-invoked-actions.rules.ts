/// <reference path="../rules.d.ts" />

const ALLOWED_FILES = new Set([
  "src/components/review/ReviewActions.tsx",
  "src/mock-tauri/mock-handlers.ts",
]);

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

        for (const file of entryPoints) {
          const source = await ctx.readFile(file);
          if (!/skipAnalyzers\s*:\s*true/.test(source)) {
            ctx.report.violation({
              message:
                "GUI AI review must send skipAnalyzers: true so it does not rerun repository gates.",
              file,
            });
          }
          const unsafeMatches = await ctx.grep(file, /skipAnalyzers\s*:\s*false/g);
          for (const match of unsafeMatches) {
            ctx.report.violation({
              message:
                "Do not enable analyzers from GUI AI review; validation runs before Lachesi review.",
              file: match.file,
              line: match.line,
            });
          }
        }
      },
    },
  },
} satisfies RuleSet;
