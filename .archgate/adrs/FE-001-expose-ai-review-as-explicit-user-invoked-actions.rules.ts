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
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "'" || character === '"' || character === "`") quote = character;
    else if (character === "{") depth += 1;
    else if (character === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openingBrace, index + 1);
    }
  }
  return null;
}

function topLevelObjectSegments(objectSource: string): string[] {
  const segments: string[] = [];
  let start = 1;
  let depth = 0;
  let quote: "'" | '"' | "`" | null = null;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;

  for (let index = 1; index < objectSource.length - 1; index += 1) {
    const character = objectSource[index];
    const next = objectSource[index + 1];
    if (lineComment) {
      if (character === "\n") lineComment = false;
      continue;
    }
    if (blockComment) {
      if (character === "*" && next === "/") {
        blockComment = false;
        index += 1;
      }
      continue;
    }
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "/" && next === "/") {
      lineComment = true;
      index += 1;
    } else if (character === "/" && next === "*") {
      blockComment = true;
      index += 1;
    } else if (character === "'" || character === '"' || character === "`") {
      quote = character;
    } else if (character === "{" || character === "[" || character === "(") {
      depth += 1;
    } else if (character === "}" || character === "]" || character === ")") {
      depth -= 1;
    } else if (character === "," && depth === 0) {
      segments.push(objectSource.slice(start, index));
      start = index + 1;
    }
  }
  segments.push(objectSource.slice(start, -1));
  return segments;
}

function hasEffectiveTrueProperty(objectSource: string, expectedName: string): boolean {
  let effectiveValue: boolean | null = null;
  const directName = `(?:${expectedName}|["']${expectedName}["'])`;
  const computedName = `\\[\\s*["']${expectedName}["']\\s*\\]`;

  for (const segment of topLevelObjectSegments(objectSource)) {
    const property = segment
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/\/\/[^\n]*/g, "")
      .trim();
    if (!property) continue;
    if (property.startsWith("...") || property.startsWith("[")) {
      const computed = property.match(new RegExp(`^${computedName}\\s*:\\s*(true|false)\\s*$`));
      effectiveValue = computed ? computed[1] === "true" : null;
      continue;
    }
    const literal = property.match(new RegExp(`^${directName}\\s*:\\s*(true|false)\\s*$`));
    if (literal) {
      effectiveValue = literal[1] === "true";
      continue;
    }
    if (
      new RegExp(`^${directName}\\s*:`).test(property) ||
      new RegExp(`^(?:get|set)\\s+${directName}\\b`).test(property) ||
      new RegExp(`^${expectedName}$`).test(property)
    ) {
      effectiveValue = null;
    }
  }
  return effectiveValue === true;
}

function findStartInlineReviewCalls(source: string) {
  const calls: Array<{ line: number; valid: boolean }> = [];
  const callPattern = /tauriCall(?:<[^>]*>)?\s*\(\s*["']start_inline_review["']/g;
  for (const match of source.matchAll(callPattern)) {
    const callIndex = match.index ?? 0;
    const argument = extractInlineObjectArgument(source, callIndex + match[0].length);
    calls.push({
      line: source.slice(0, callIndex).split("\n").length,
      valid: !!argument && hasEffectiveTrueProperty(argument, "skipAnalyzers"),
    });
  }
  return calls;
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
            const calls = findStartInlineReviewCalls(source);

            for (const call of calls) {
              if (!call.valid) {
                ctx.report.violation({
                  message: "Every GUI start_inline_review call must send skipAnalyzers: true.",
                  file,
                  line: call.line,
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
