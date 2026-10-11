import { refractor } from "refractor";
import jsx from "refractor/jsx";
import tsx from "refractor/tsx";
import {
  type DiffSyntax,
  MAX_SYNTAX_BYTES,
  MAX_SYNTAX_ROWS,
  plainSyntax,
  type SyntaxFragment,
  syntaxLanguage,
} from "./syntaxTypes";

refractor.register(jsx);
refractor.register(tsx);
interface Node {
  type: string;
  value?: string;
  properties?: { className?: string[] };
  children?: Node[];
}
const tokens = new Set([
  "comment",
  "string",
  "keyword",
  "function",
  "number",
  "boolean",
  "operator",
  "punctuation",
  "class-name",
  "property",
  "tag",
  "attr-name",
  "attr-value",
  "regex",
]);
export function parseDiffSyntax(path: string, diff: string): DiffSyntax {
  const language = syntaxLanguage(path);
  if (!language || !refractor.registered(language))
    return plainSyntax("No syntax grammar for this file.", language);
  const lines = diff.split("\n");
  if (
    Buffer.byteLength(diff) > MAX_SYNTAX_BYTES ||
    lines.length > MAX_SYNTAX_ROWS ||
    lines.some((line) => line.length > 4096)
  )
    return plainSyntax("Syntax limit reached; complete diff text remains available.", language);
  const rows: Record<number, SyntaxFragment[]> = {};
  let fragments = 0;
  let visited = 0;
  let renderedFragments = 0;
  function parse(source: Array<{ index: number; text: string }>, side: "old" | "new") {
    if (!source.length) return;
    const text = source.map((line) => line.text).join("\n");
    const root = refractor.highlight(text, language!) as Node;
    const flat: SyntaxFragment[] = [];
    function visit(node: Node, token: string | null, depth: number) {
      if (++visited > 100000 || depth > 64) throw new Error("Syntax tree exceeds limits");
      const selected = node.properties?.className?.find((name) => tokens.has(name)) ?? token;
      if (node.type === "text") {
        if (++fragments > 16000) throw new Error("Syntax fragments exceed limits");
        flat.push({ text: node.value ?? "", token: selected });
      } else for (const child of node.children ?? []) visit(child, selected, depth + 1);
    }
    visit(root, null, 0);
    if (flat.map((part) => part.text).join("") !== text)
      throw new Error("Syntax changed source text");
    let row = 0;
    for (const part of flat) {
      const pieces = part.text.split("\n");
      for (let index = 0; index < pieces.length; index++) {
        const entry = source[row];
        if (!entry) throw new Error("Syntax source rows changed");
        if ((side === "new" || lines[entry.index]?.startsWith("-")) && pieces[index]) {
          if (++renderedFragments > 16000) throw new Error("Syntax row fragments exceed limits");
          rows[entry.index] ??= [];
          rows[entry.index]!.push({ text: pieces[index]!, token: part.token });
        }
        if (index < pieces.length - 1) row++;
      }
    }
    if (row !== source.length - 1) throw new Error("Syntax line count changed");
  }
  let old: Array<{ index: number; text: string }> = [];
  let next: Array<{ index: number; text: string }> = [];
  let inHunk = false;
  function flush() {
    parse(old, "old");
    parse(next, "new");
    old = [];
    next = [];
  }
  try {
    for (const [index, line] of lines.entries()) {
      if (line.startsWith("@@ ")) {
        flush();
        inHunk = true;
        continue;
      }
      if (line.startsWith("diff --git ")) {
        flush();
        inHunk = false;
        continue;
      }
      if (!inHunk || ![" ", "+", "-"].includes(line[0] ?? "")) continue;
      const entry = { index, text: line.slice(1) };
      if (line[0] !== "+") old.push(entry);
      if (line[0] !== "-") next.push(entry);
    }
    flush();
    // Preserve blank code rows too; metadata and diff prefixes stay in Shell.
    for (const [index, line] of lines.entries())
      if (rows[index] && rows[index]!.map((part) => part.text).join("") !== line.slice(1))
        throw new Error("Syntax row text changed");
    return { status: "ready", language, reason: null, rows };
  } catch {
    return {
      status: "failed",
      language,
      reason: "Syntax could not be parsed; complete diff text remains available.",
      rows: {},
    };
  }
}
