import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  chmodSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { installLocal } from "./install-local.mjs";

function command(path, version, succeeds = true) {
  writeFileSync(path, `#!/bin/sh\nprintf '%s\\n' '${version}'\nexit ${succeeds ? 0 : 1}\n`);
  chmodSync(path, 0o755);
}

function fixture(version = "1.0.0") {
  const root = mkdtempSync(join(tmpdir(), "norn-install-"));
  const sourceDirectory = join(root, "build");
  const prefix = join(root, "prefix");
  const browserAssetsDirectory = join(root, "browser-diff");
  mkdirSync(sourceDirectory);
  mkdirSync(join(browserAssetsDirectory, "assets"), { recursive: true });
  writeFileSync(join(browserAssetsDirectory, "browser-diff.html"), '<div id="root"></div>');
  writeFileSync(join(browserAssetsDirectory, "assets", "viewer.js"), "export {};");
  for (const name of ["norn", "norn-tui", "norn-app", "lachesi", "lac"]) {
    command(join(sourceDirectory, name), `${name} ${version}`);
  }
  return { root, sourceDirectory, browserAssetsDirectory, prefix };
}

test("installs durable executable files and upgrades every selected command", () => {
  const context = fixture();
  try {
    const first = installLocal(context);
    assert.deepEqual(first.commands, ["norn", "norn-tui", "norn-app", "lachesi", "lac"]);
    assert.equal(
      readFileSync(
        join(context.prefix, "share", "norn", "browser-diff", "browser-diff.html"),
        "utf8",
      ),
      '<div id="root"></div>',
    );
    rmSync(context.sourceDirectory, { recursive: true });
    for (const name of first.commands) {
      const installed = join(context.prefix, "bin", name);
      assert.equal(lstatSync(installed).isSymbolicLink(), false);
      assert.match(execFileSync(installed, ["--version"], { encoding: "utf8" }), /1\.0\.0/);
    }

    mkdirSync(context.sourceDirectory);
    for (const name of first.commands) {
      command(join(context.sourceDirectory, name), `${name} 2.0.0`);
    }
    writeFileSync(
      join(context.browserAssetsDirectory, "browser-diff.html"),
      '<div id="root">version 2</div>',
    );
    installLocal(context);
    for (const name of first.commands) {
      assert.match(
        execFileSync(join(context.prefix, "bin", name), ["--version"], {
          encoding: "utf8",
        }),
        /2\.0\.0/,
      );
    }
    assert.equal(
      readFileSync(
        join(context.prefix, "share", "norn", "browser-diff", "browser-diff.html"),
        "utf8",
      ),
      '<div id="root">version 2</div>',
    );
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});

test("leaves the previous installation intact when staging verification fails", () => {
  const context = fixture();
  try {
    installLocal({ ...context, components: ["cli", "tui"] });
    command(join(context.sourceDirectory, "norn"), "norn 2.0.0");
    command(join(context.sourceDirectory, "norn-tui"), "broken", false);

    assert.throws(
      () => installLocal({ ...context, components: ["cli", "tui"] }),
      /returned a failure/,
    );
    assert.match(
      execFileSync(join(context.prefix, "bin", "norn"), ["--version"], {
        encoding: "utf8",
      }),
      /1\.0\.0/,
    );
    assert.match(
      execFileSync(join(context.prefix, "bin", "norn-tui"), ["--version"], {
        encoding: "utf8",
      }),
      /1\.0\.0/,
    );
    assert.equal(
      readFileSync(
        join(context.prefix, "share", "norn", "browser-diff", "browser-diff.html"),
        "utf8",
      ),
      '<div id="root"></div>',
    );
  } finally {
    rmSync(context.root, { recursive: true, force: true });
  }
});
