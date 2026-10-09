import { expect, test } from "bun:test";
import { diffLines, splitRows } from "../src/app/components/Shell";
import { anchorPresent } from "../src/app/store";

const DIFF = "@@ -1 +1 @@\n-old\n+new\n";

test("added lines highlight the new line number without an off-by-one", () => {
  const lines = diffLines(DIFF, 1, "new");
  const highlighted = lines.find((line) => line.highlighted);
  expect(highlighted?.text).toBe("+new");
});

test("deleted lines highlight the old line number", () => {
  const lines = diffLines(DIFF, 1, "old");
  const highlighted = lines.find((line) => line.highlighted);
  expect(highlighted?.text).toBe("-old");
});

test("anchor validation ignores file header metadata and trailing newlines", () => {
  const withHeaders =
    "diff --git a/x b/x\nindex 1..2 100644\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n";
  expect(anchorPresent(withHeaders, 1, "new")).toBe(true);
  expect(anchorPresent(withHeaders, 1, "old")).toBe(true);
  expect(anchorPresent(withHeaders, 2, "new")).toBe(false);
  expect(anchorPresent(withHeaders, 5, "new")).toBe(false);
});

test("split rows pair removals with additions and keep old/new line numbers", () => {
  const lines = diffLines("@@ -1,3 +1,2 @@\n-a\n-b\n+A\n c\n", null, "new");
  const rows = splitRows(lines);
  expect(rows[0]).toEqual({ kind: "hunk", text: "@@ -1,3 +1,2 @@" });
  expect(rows[1]).toMatchObject({
    left: { text: "-a", oldNumber: 1 },
    right: { text: "+A", newNumber: 1 },
  });
  expect(rows[2]).toMatchObject({ left: { text: "-b", oldNumber: 2 }, right: null });
  expect(rows[3]).toMatchObject({
    left: { text: " c", oldNumber: 3 },
    right: { text: " c", newNumber: 2 },
  });
});

test("split rows attach newline annotations without breaking pairing", () => {
  const lines = diffLines("@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\n", null, "new");
  const rows = splitRows(lines);
  expect(rows[1]).toMatchObject({ left: { text: "-a" }, right: { text: "+b" } });
  expect(rows[2]).toMatchObject({ kind: "note", text: "\\ No newline at end of file" });
});

test("split view treats file headers as metadata, not source lines", () => {
  const lines = diffLines(
    "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+b\n",
    null,
    "new",
  );
  const rows = splitRows(lines);
  expect(rows[0]).toMatchObject({ kind: "note", text: "diff --git a/f b/f" });
  expect(rows[1]).toMatchObject({ kind: "note", text: "--- a/f" });
  expect(rows[2]).toMatchObject({ kind: "note", text: "+++ b/f" });
  expect(rows[3]).toMatchObject({ kind: "hunk" });
  expect(rows[4]).toMatchObject({
    kind: "pair",
    left: { text: "-a", oldNumber: 1 },
    right: { text: "+b", newNumber: 1 },
  });
});
