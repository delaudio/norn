import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { renderFormula } from "./render-homebrew-formula.mjs";

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "norn-formula-"));
  const version = "1.2.3";
  for (const architecture of ["arm64", "x86_64"]) {
    const name = `norn-${version}-macos-${architecture}.tar.gz`;
    const contents = Buffer.from(`archive-${architecture}`);
    const digest = createHash("sha256").update(contents).digest("hex");
    writeFileSync(join(directory, name), contents);
    writeFileSync(join(directory, `${name}.sha256`), `${digest}  ${name}\n`);
  }
  return { directory, version, outputPath: join(directory, "norn.rb") };
}

test("renders exact version, URLs, and verified architecture checksums", () => {
  const context = fixture();
  try {
    const { checksums } = renderFormula({
      version: context.version,
      artifactsDirectory: context.directory,
      outputPath: context.outputPath,
    });
    const formula = readFileSync(context.outputPath, "utf8");
    assert.match(formula, /version "1\.2\.3"/);
    assert.match(formula, new RegExp(checksums.arm64));
    assert.match(formula, new RegExp(checksums.x86_64));
    assert.match(formula, /pkgshare\.install "share\/norn\/agent-skills"/);
    assert.match(formula, /pkgshare\.install "share\/norn\/browser-diff"/);
    assert.match(formula, /browser-diff\/browser-diff\.html/);
    assert.match(formula, /norn skills status --json/);
    assert.doesNotMatch(formula, /\{\{|:no_check/);
  } finally {
    rmSync(context.directory, { recursive: true, force: true });
  }
});

test("fails closed when a sidecar does not match its archive", () => {
  const context = fixture();
  try {
    writeFileSync(
      join(context.directory, `norn-${context.version}-macos-arm64.tar.gz.sha256`),
      `${"0".repeat(64)}  norn-${context.version}-macos-arm64.tar.gz\n`,
    );
    assert.throws(
      () =>
        renderFormula({
          version: context.version,
          artifactsDirectory: context.directory,
          outputPath: context.outputPath,
        }),
      /Checksum mismatch/,
    );
  } finally {
    rmSync(context.directory, { recursive: true, force: true });
  }
});

test("fails closed when either architecture artifact is missing", () => {
  const context = fixture();
  try {
    const missing = `norn-${context.version}-macos-x86_64.tar.gz`;
    rmSync(join(context.directory, missing));
    rmSync(join(context.directory, `${missing}.sha256`));
    assert.throws(
      () =>
        renderFormula({
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
