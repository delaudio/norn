#!/usr/bin/env node

import { readFileSync } from "node:fs";

const formulaPath = "homebrew-tap/Formula/norn.rb";
const caskPath = "homebrew-tap/Casks/norn.rb";

const formula = readFileSync(formulaPath, "utf8");
const cask = readFileSync(caskPath, "utf8");

const checks = [];
const fail = (message) => {
  console.error(message);
  process.exit(1);
};

const assert = (condition, message) => {
  checks.push({ message, passed: Boolean(condition) });
};

assert(
  /test do[\s\S]*norn --version[\s\S]*norn --help/.test(formula),
  "Formula should include a test that checks `norn --version` and `norn --help`.",
);
assert(
  /arm64\.tar\.gz/.test(formula) && /x86_64\.tar\.gz/.test(formula),
  "Formula should include both macOS architecture artifact URLs.",
);
assert(
  /version [\"\']\d+\.\d+\.\d+/.test(formula),
  "Formula should pin a version.",
);

assert(/app \"Norn\.app\"/.test(cask), "Cask should install the Norn app bundle.");
assert(/sha256 :no_check/.test(cask), "Cask should opt-in to explicit checksum handling via release automation.");
assert(/uninstall quit: \"app\.norn\.desktop\"/.test(cask), "Cask should define uninstall app identifier.");

const failed = checks.filter((check) => !check.passed);
if (failed.length > 0) {
  for (const check of failed) {
    console.error(`FAIL: ${check.message}`);
  }
  process.exit(1);
}

console.log("Homebrew manifests pass structural checks:");
for (const check of checks) {
  console.log(`- ${check.message}`);
}
