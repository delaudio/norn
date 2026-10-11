/// <reference path="../rules.d.ts" />

const EXPORT_BRIDGE = "src/lib/desktopDataExport.ts";

const ALLOWED_FILES = new Set([
  EXPORT_BRIDGE,
  "src/hooks/useTheme.ts",
  "src/lib/reviewPrompt.ts",
  "src/hooks/useDraftComments.ts",
  "src/lib/reviewReferencesStorage.ts",
  "src/lib/viewedFilesStorage.ts",
  "src/lib/menuBarPrSnapshotStorage.ts",
  "src/components/pr-sidebar/PrSidebar.tsx",
  "src/mock-tauri/mock-handlers.ts",
]);

export default {
  rules: {
    "localstorage-stays-in-approved-modules": {
      description:
        "Direct localStorage access is restricted to approved preference and draft owner modules",
      async check(ctx) {
        const files = [...(await ctx.glob("src/**/*.ts")), ...(await ctx.glob("src/**/*.tsx"))];

        for (const file of files) {
          if (file === EXPORT_BRIDGE) {
            const writes = await ctx.grep(file, /\b(?:setItem|removeItem|clear)\s*\(/g);
            for (const match of writes)
              ctx.report.violation({
                message: "The temporary desktop export bridge must remain read-only.",
                file: match.file,
                line: match.line,
              });
          }
          if (ALLOWED_FILES.has(file)) continue;
          const matches = await ctx.grep(file, /\blocalStorage\b/g);
          for (const match of matches) {
            ctx.report.violation({
              message:
                "Move localStorage access into an approved owner module or update FE-002 intentionally.",
              file: match.file,
              line: match.line,
            });
          }
        }
      },
    },
  },
} satisfies RuleSet;
