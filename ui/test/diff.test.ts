import { expect, test } from "bun:test";
import { diffLines } from "../src/app/components/Shell";
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
