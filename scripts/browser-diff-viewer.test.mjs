import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync("src-tauri/src/tui/diff_server.rs", "utf8");

function embeddedFunction(name) {
  const start = source.indexOf(`    function ${name}(`);
  assert.notEqual(start, -1, `missing embedded browser function: ${name}`);
  const end = source.indexOf("\n\n    function ", start + 1);
  assert.notEqual(end, -1, `could not find end of embedded browser function: ${name}`);
  return source.slice(start, end);
}

const context = {
  TextDecoder,
  TextEncoder,
  Uint8Array,
  document: {
    createElement(tagName) {
      return { tagName, className: "", textContent: "" };
    },
  },
};
const functions = [
  "normalizeStatus",
  "decodeGitPathToken",
  "readQuotedGitToken",
  "stripGitSidePrefix",
  "parseDiffGitHeader",
  "parseUnifiedDiff",
  "isImageFile",
  "handleImagePreviewError",
]
  .map(embeddedFunction)
  .join("\n\n");
vm.runInNewContext(
  `${functions}\nthis.browserDiff = { normalizeStatus, parseUnifiedDiff, isImageFile, handleImagePreviewError };`,
  context,
);

test("embedded diff parser decodes quoted and unquoted Git paths", () => {
  const raw = String.raw`diff --git "a/docs/caf\303\251.md" "b/docs/caf\303\251.md"
--- "a/docs/caf\303\251.md"
+++ "b/docs/caf\303\251.md"
@@ -1 +1 @@
-old
+new
diff --git a/docs/hello world.md b/docs/hello world.md
--- a/docs/hello world.md
+++ b/docs/hello world.md
@@ -1 +1 @@
-before
+after
diff --git "a/docs/quote\"name.md" "b/docs/quote\"name.md"
--- "a/docs/quote\"name.md"
+++ "b/docs/quote\"name.md"
@@ -1 +1 @@
-left
+right`;

  const parsed = context.browserDiff.parseUnifiedDiff(raw);
  assert.deepEqual(
    Array.from(parsed, (file) => file.path),
    ["docs/café.md", "docs/hello world.md", 'docs/quote"name.md'],
  );
});

test("embedded viewer accepts only canonical provider statuses", () => {
  assert.equal(context.browserDiff.normalizeStatus("removed"), "removed");
  assert.equal(context.browserDiff.normalizeStatus("added"), "added");
  assert.equal(context.browserDiff.normalizeStatus("<img onerror=alert(1)>"), "modified");
});

test("embedded viewer detects only image formats allowed by the preview server", () => {
  for (const path of ["logo.png", "photo.JPG", "clip.gif", "asset.webp"]) {
    assert.equal(context.browserDiff.isImageFile(path), true, path);
  }
  for (const path of ["icon.ico", "bitmap.bmp", "vector.svg", "notes.txt"]) {
    assert.equal(context.browserDiff.isImageFile(path), false, path);
  }
});

test("image preview failures replace the card content through DOM APIs", () => {
  const parent = {
    children: [],
    replaceChildren(...children) {
      this.children = children;
    },
  };

  context.browserDiff.handleImagePreviewError({ parentElement: parent });

  assert.equal(parent.children.length, 1);
  assert.equal(parent.children[0].tagName, "div");
  assert.equal(parent.children[0].className, "image-preview-empty");
  assert.equal(parent.children[0].textContent, "No preview available");
});
