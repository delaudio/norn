/// <reference path="../rules.d.ts" />

const MAKEFILE = "Makefile";
const JUSTFILE = "justfile";

/**
 * Extract target names from a Makefile.
 * A target is a line starting at column 0 of the form `name:` (optionally
 * `name: prereqs`). Variable assignments (`VAR =`, `VAR :=`), comments,
 * recipe bodies (tab-indented), and special targets (`.PHONY`, `.DEFAULT`)
 * are ignored.
 */
function extractMakeTargets(source: string): Set<string> {
  const names = new Set<string>();
  for (const raw of source.split(/\r?\n/)) {
    // Recipe body lines are tab-indented; skip them.
    if (raw.startsWith("\t")) continue;
    const line = raw.trimEnd();
    if (!line || line.startsWith("#")) continue;
    // `name:` but not `name :=` (variable) — negative lookahead on `=`.
    const match = line.match(/^([A-Za-z0-9_][A-Za-z0-9_.-]*)\s*:(?!=)/);
    if (!match) continue;
    const name = match[1];
    // Skip special targets like `.PHONY`, `.DEFAULT`.
    if (name.startsWith(".")) continue;
    names.add(name);
  }
  return names;
}

/**
 * Extract recipe names from a justfile.
 * A recipe is a line starting at column 0 of the form `name:` or
 * `name params...:` (optionally `name: deps`). Settings (`set ...`),
 * imports/modules/aliases, variable assignments (`name := ...`), comments,
 * attribute lines (`[private]`), and indented recipe bodies are ignored.
 */
function extractJustRecipes(source: string): Set<string> {
  const names = new Set<string>();
  for (const raw of source.split(/\r?\n/)) {
    // Recipe body lines are indented (space or tab); skip them.
    if (raw.startsWith(" ") || raw.startsWith("\t")) continue;
    const line = raw.trimEnd();
    if (!line || line.startsWith("#") || line.startsWith("[")) continue;
    // Skip settings, imports, module declarations, aliases, exports.
    if (/^(set|import|mod|alias|export)\b/.test(line)) continue;
    // Skip variable assignments: `name := value`.
    if (/^[A-Za-z0-9_-]+\s*:=/.test(line)) continue;
    // `name` then optional params (no `:`/`=`) then `:` but not `:=`.
    const match = line.match(/^([A-Za-z0-9_][A-Za-z0-9_-]*)[^:=]*:(?!=)/);
    if (!match) continue;
    names.add(match[1]);
  }
  return names;
}

async function fileExists(ctx: RuleContext, path: string): Promise<boolean> {
  const matches = await ctx.glob(path);
  return matches.length > 0;
}

export default {
  rules: {
    "task-runner-files-present": {
      description:
        "The root Makefile and justfile must both exist so each platform has its native task runner",
      async check(ctx) {
        const hasMake = await fileExists(ctx, MAKEFILE);
        const hasJust = await fileExists(ctx, JUSTFILE);

        if (!hasMake && !hasJust) {
          ctx.report.warning({
            message:
              "No task runner found. ARCH-007 requires a root Makefile (macOS/Linux) and a root justfile (Windows) with matching recipe names.",
            fix: "Create Makefile and justfile at the repository root exposing the same recipe set.",
          });
          return;
        }

        if (hasMake && !hasJust) {
          ctx.report.violation({
            message:
              "A Makefile exists but justfile is missing. Windows contributors require a matching justfile.",
            file: MAKEFILE,
            fix: "Add a root justfile mirroring every Makefile target.",
          });
        }

        if (hasJust && !hasMake) {
          ctx.report.violation({
            message:
              "A justfile exists but Makefile is missing. macOS/Linux contributors require a matching Makefile.",
            file: JUSTFILE,
            fix: "Add a root Makefile mirroring every justfile recipe.",
          });
        }
      },
    },

    "justfile-uses-native-windows-shell": {
      description:
        "The justfile must configure a native Windows shell so recipes never fall back to just's missing default `sh`",
      async check(ctx) {
        // Only applies once the Windows runner exists.
        if (!(await fileExists(ctx, JUSTFILE))) return;

        // Match `set windows-shell := [...]` or `set shell := [...]`.
        const matches = await ctx.grep(JUSTFILE, /^\s*set\s+(windows-shell|shell)\s*:=/m);
        if (matches.length === 0) {
          ctx.report.violation({
            message:
              'justfile does not configure a shell. On Windows, just defaults to `sh`, which is not installed, so every recipe fails with "could not find the shell `sh`".',
            file: JUSTFILE,
            fix: 'Add at the top of the justfile: set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]',
          });
        }
      },
    },

    "task-runner-parity": {
      description: "The Makefile and justfile must expose the identical set of recipe/target names",
      async check(ctx) {
        const hasMake = await fileExists(ctx, MAKEFILE);
        const hasJust = await fileExists(ctx, JUSTFILE);

        // Existence is handled by task-runner-files-present; parity only
        // applies when both files are present.
        if (!hasMake || !hasJust) return;

        const makeTargets = extractMakeTargets(await ctx.readFile(MAKEFILE));
        const justRecipes = extractJustRecipes(await ctx.readFile(JUSTFILE));

        for (const name of makeTargets) {
          if (!justRecipes.has(name)) {
            ctx.report.violation({
              message: `Makefile target "${name}" has no matching justfile recipe. Recipe names must stay in parity across platforms.`,
              file: JUSTFILE,
              fix: `Add a "${name}:" recipe to the justfile.`,
            });
          }
        }

        for (const name of justRecipes) {
          if (!makeTargets.has(name)) {
            ctx.report.violation({
              message: `justfile recipe "${name}" has no matching Makefile target. Recipe names must stay in parity across platforms.`,
              file: MAKEFILE,
              fix: `Add a "${name}:" target to the Makefile.`,
            });
          }
        }
      },
    },
  },
} satisfies RuleSet;
