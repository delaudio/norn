import { expect, test } from "bun:test";
import { SyntaxHighlighter, validSyntax } from "../src/app/syntaxHighlighter";
import type { SyntaxTask } from "../src/app/syntaxProcess";
import { parseDiffSyntax } from "../src/app/syntaxTokens";

const patch =
  "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1,2 +1,2 @@\n-const old = 'é';\n+const next = '世界';\n /* comment */";
test("syntax preserves exact source on both sides and colors language tokens", () => {
  for (const [path, source] of [
    ["a.ts", patch],
    ["a.tsx", "@@ -1 +1 @@\n-<div old={1}/>\n+<div next={2}/>"],
    ["a.rs", "@@ -1 +1,2 @@\n-let old = 1;\n+let next = 2;\n+// comment"],
    ["a.ts", "@@ -1,3 +1,3 @@\n /* first\n  * second\n  */"],
  ]) {
    const result = parseDiffSyntax(path!, source!);
    expect(result.status).toBe("ready");
    expect(validSyntax(result, source!)).toBe(true);
    for (const [key, parts] of Object.entries(result.rows))
      expect(parts.map((part) => part.text).join("")).toBe(
        source!.split("\n")[Number(key)]!.slice(1),
      );
    expect(
      Object.values(result.rows)
        .flat()
        .some((part) => part.token !== null),
    ).toBe(true);
  }
});
test("unknown grammars and bounded inputs keep complete text available", () => {
  expect(parseDiffSyntax("a.unknown", patch).status).toBe("plain");
  expect(parseDiffSyntax("a.ts", "@@ -1 +1 @@\n+" + "x".repeat(4097)).status).toBe("plain");
  expect(parseDiffSyntax("a.ts", "@@ -1 +1 @@\n" + "+x\n".repeat(3001)).status).toBe("plain");
});
test("worker messages cannot change or invent source rows", () => {
  const valid = parseDiffSyntax("a.ts", patch);
  expect(validSyntax({ ...valid, rows: { 5: [{ text: "changed", token: null }] } }, patch)).toBe(
    false,
  );
  expect(validSyntax({ ...valid, rows: { 999: [] } }, patch)).toBe(false);
  expect(validSyntax({ ...valid, rows: { __proto__: [] } }, patch)).toBe(false);
  expect(validSyntax({ ...valid, rows: { "-1": [] } }, patch)).toBe(false);
  expect(
    validSyntax({ ...valid, rows: { 5: [{ text: "const old = 'é';", token: 1 }] } }, patch),
  ).toBe(false);
});
test("packaged Bun subprocess produces source-preserving syntax and is reaped", async () => {
  const reader = new SyntaxHighlighter(undefined, 3000);
  const result = await reader.highlight("a.ts", patch);
  expect(result.status).toBe("ready");
  expect(validSyntax(result, patch)).toBe(true);
  expect(await reader.highlight("a.ts", patch)).toBe(result);
  reader.cancel();
});
test("new selections wait for owned worker termination and coalesce to latest", async () => {
  const workers: Array<{
    onmessage: ((event: MessageEvent) => void) | null;
    onerror: unknown;
    postMessage: () => void;
    terminate: () => Promise<void>;
  }> = [];
  const releases: Array<() => void> = [];
  const reader = new SyntaxHighlighter(() => {
    const worker = {
      onmessage: null,
      onerror: null,
      postMessage() {},
      terminate: () => new Promise<void>((resolve) => releases.push(resolve)),
    };
    workers.push(worker);
    return worker as unknown as SyntaxTask;
  }, 1000);
  const first = reader.highlight("a.ts", patch);
  const second = reader.highlight("b.ts", patch);
  const third = reader.highlight("c.ts", patch);
  expect(workers.length).toBe(1);
  expect((await second).status).toBe("plain");
  releases[0]!();
  await first;
  expect(workers.length).toBe(2);
  workers[1]!.onmessage!({ data: parseDiffSyntax("c.ts", patch) } as MessageEvent);
  releases[1]!();
  expect((await third).status).toBe("ready");
  reader.cancel();
});
test("rapid tokenizer cancellation never creates overlapping replacements or crashes the host", async () => {
  const reader = new SyntaxHighlighter(undefined, 3000);
  const requests = Array.from({ length: 100 }, (_, index) =>
    reader.highlight(`file-${index}.ts`, patch),
  );
  const results = await Promise.all(requests);
  expect(results.slice(0, -1).every((result) => result.status === "plain")).toBe(true);
  expect(results.at(-1)?.status).toBe("ready");
  reader.cancel();
});
test("timeout, malformed result and start failure resolve to complete plain text", async () => {
  let terminated = 0;
  const task: SyntaxTask = {
    onmessage: null,
    onerror: null,
    postMessage() {},
    terminate() {
      terminated++;
    },
  };
  const timeout = new SyntaxHighlighter(() => task, 5);
  expect((await timeout.highlight("a.ts", patch)).status).toBe("plain");
  expect(terminated).toBe(1);
  const malformed = new SyntaxHighlighter(() => task);
  const pending = malformed.highlight("a.ts", patch);
  task.onmessage!({
    data: {
      status: "ready",
      language: "typescript",
      reason: null,
      rows: { 5: [{ text: "altered", token: "keyword" }] },
    },
  });
  expect((await pending).status).toBe("plain");
  expect(terminated).toBe(2);
  const failed = new SyntaxHighlighter(() => {
    throw new Error("synthetic start failure");
  });
  expect((await failed.highlight("a.ts", patch)).status).toBe("plain");
});
