#!/usr/bin/env node

import { readFileSync } from "node:fs";

const formulaTemplatePath = "packaging/homebrew/norn.rb.template";
const caskTemplatePath = "packaging/homebrew/norn-cask.rb.template";

function optionalPath(flag) {
  const index = process.argv.indexOf(flag);
  if (index < 0) {
    return undefined;
  }
  const path = process.argv[index + 1];
  if (!path || path.startsWith("--")) {
    console.error(`${flag} requires a path.`);
    process.exit(1);
  }
  return path;
}

const renderedFormulaPath = optionalPath("--formula");
const renderedCaskPath = optionalPath("--cask");
const formulaTemplate = readFileSync(formulaTemplatePath, "utf8");
const caskTemplate = readFileSync(caskTemplatePath, "utf8");
const formula = renderedFormulaPath ? readFileSync(renderedFormulaPath, "utf8") : formulaTemplate;
const cask = renderedCaskPath ? readFileSync(renderedCaskPath, "utf8") : caskTemplate;

const checks = [];
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
  renderedFormulaPath
    ? /version ["']\d+\.\d+\.\d+["']/.test(formula)
    : /version ["']\{\{VERSION\}\}["']/.test(formula),
  renderedFormulaPath
    ? "Rendered formula should pin a stable version."
    : "Formula template should declare the version placeholder.",
);
assert(
  !/sha256\s+:no_check/.test(formula),
  "CLI/TUI formula should never disable checksum verification.",
);
assert(
  renderedFormulaPath
    ? (formula.match(/sha256\s+["'][0-9a-f]{64}["']/g)?.length ?? 0) === 2 &&
        !/\{\{[^}]+\}\}/.test(formula)
    : formula.includes("{{ARM64_SHA256}}") && formula.includes("{{X86_64_SHA256}}"),
  renderedFormulaPath
    ? "Rendered formula should contain two exact checksums and no placeholders."
    : "Formula template should declare both architecture checksum placeholders.",
);

assert(/app "Norn\.app"/.test(cask), "Cask should install the Norn app bundle.");
assert(
  /arch arm: "arm64", intel: "x86_64"/.test(cask) &&
    /Norn-#\{version\}-macos-#\{arch\}\.dmg/.test(cask),
  "Cask should select immutable architecture-specific DMG assets.",
);
assert(!/sha256\s+:no_check/.test(cask), "Desktop cask should never disable checksums.");
assert(
  renderedCaskPath
    ? /version ["']\d+\.\d+\.\d+["']/.test(cask) &&
        (cask.match(/["'][0-9a-f]{64}["']/g)?.length ?? 0) === 2 &&
        !/\{\{[^}]+\}\}/.test(cask)
    : cask.includes("{{VERSION}}") &&
        cask.includes("{{ARM64_SHA256}}") &&
        cask.includes("{{X86_64_SHA256}}"),
  renderedCaskPath
    ? "Rendered cask should pin its version, two checksums, and no placeholders."
    : "Cask template should declare version and architecture checksum placeholders.",
);
assert(
  !/^\s*binary\b/m.test(cask),
  "Cask should not install CLI binaries that conflict with the formula.",
);
assert(
  /uninstall quit: "app\.norn\.desktop"/.test(cask),
  "Cask should define uninstall app identifier.",
);

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
