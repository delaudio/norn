#!/usr/bin/env node

import { spawnSync } from "node:child_process";

const testFiles = [
  "scripts/distribution-modes.test.mjs",
  "scripts/install-local.test.mjs",
  "scripts/render-homebrew-cask.test.mjs",
  "scripts/render-homebrew-formula.test.mjs",
  "scripts/release-channels.test.mjs",
  "scripts/resolve-previous-homebrew-release.test.mjs",
  "scripts/run-with-timeout.test.mjs",
];

for (const testFile of testFiles) {
  const result = spawnSync(process.execPath, ["--test", testFile], {
    stdio: "inherit",
    timeout: 120_000,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    console.error(`Tooling test failed: ${testFile}.`);
    process.exit(result.status ?? 1);
  }
}
