import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { renderCask } from "./render-homebrew-cask.mjs";

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "norn-cask-"));
  const version = "1.2.3";
  for (const architecture of ["arm64", "x86_64"]) {
    const name = `Norn-${version}-macos-${architecture}.dmg`;
    const contents = Buffer.from(`desktop-${architecture}`);
    const digest = createHash("sha256").update(contents).digest("hex");
    writeFileSync(join(directory, name), contents);
    writeFileSync(join(directory, `${name}.sha256`), `${digest}  ${name}\n`);
  }
  return { directory, version, outputPath: join(directory, "norn-cask.rb") };
}

test("renders architecture-specific DMG URLs and verified checksums", () => {
  const context = fixture();
  try {
    const { checksums } = renderCask({
      version: context.version,
      artifactsDirectory: context.directory,
      outputPath: context.outputPath,
    });
    const cask = readFileSync(context.outputPath, "utf8");
    assert.match(cask, /version "1\.2\.3"/);
    assert.match(cask, /Norn-#\{version\}-macos-#\{arch\}\.dmg/);
    assert.match(cask, new RegExp(checksums.arm64));
    assert.match(cask, new RegExp(checksums.x86_64));
    assert.doesNotMatch(cask, /\{\{|:no_check|\bbinary\b/);
  } finally {
    rmSync(context.directory, { recursive: true, force: true });
  }
});

test("fails closed when a notarized architecture artifact is incomplete", () => {
  const context = fixture();
  try {
    rmSync(join(context.directory, `Norn-${context.version}-macos-x86_64.dmg.sha256`));
    assert.throws(
      () =>
        renderCask({
          version: context.version,
          artifactsDirectory: context.directory,
          outputPath: context.outputPath,
        }),
      /Expected exactly one/,
    );
  } finally {
    rmSync(context.directory, { recursive: true, force: true });
  }
});
