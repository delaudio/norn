export interface SyntaxFragment {
  text: string;
  token: string | null;
}
export interface DiffSyntax {
  status: "ready" | "plain" | "failed";
  language: string | null;
  reason: string | null;
  rows: Record<number, SyntaxFragment[]>;
}
export interface SyntaxReader {
  highlight(path: string, diff: string): Promise<DiffSyntax>;
  cancel(): void;
}
export const plainSyntax = (reason: string, language: string | null = null): DiffSyntax => ({
  status: "plain",
  language,
  reason,
  rows: {},
});
export const MAX_SYNTAX_BYTES = 128 * 1024;
export const MAX_SYNTAX_ROWS = 3000;
export const languages: Record<string, string> = {
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "tsx",
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  json: "json",
  jsonc: "json",
  css: "css",
  scss: "scss",
  sass: "scss",
  less: "less",
  html: "markup",
  htm: "markup",
  xml: "markup",
  svg: "markup",
  vue: "markup",
  svelte: "markup",
  md: "markdown",
  mdx: "markdown",
  yml: "yaml",
  yaml: "yaml",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  rs: "rust",
  py: "python",
  go: "go",
  java: "java",
  rb: "ruby",
  php: "php",
  sql: "sql",
  toml: "toml",
  c: "c",
  h: "c",
  cpp: "cpp",
  graphql: "graphql",
  gql: "graphql",
};
export function syntaxLanguage(path: string): string | null {
  return languages[path.split(".").at(-1)?.toLowerCase() ?? ""] ?? null;
}
