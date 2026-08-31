import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const releaseWorkflow = readFileSync(".github/workflows/release-norn-macos.yml", "utf8");
const lifecycleWorkflow = readFileSync(".github/workflows/homebrew-lifecycle-smoke.yml", "utf8");
const desktopGate = "vars.NORN_DESKTOP_RELEASE_ENABLED == 'true'";

function yamlBlock(contents, marker, nextPattern) {
  const start = contents.indexOf(marker);
  assert.notEqual(start, -1, `Missing YAML block marker: ${marker.trim()}`);
  const remainder = contents.slice(start + marker.length);
  const next = remainder.search(nextPattern);
  return remainder.slice(0, next === -1 ? undefined : next);
}

function job(contents, name) {
  return yamlBlock(contents, `  ${name}:\n`, /^ {2}[a-z0-9-]+:\n/m);
}

function step(jobContents, name) {
  return yamlBlock(jobContents, `      - name: ${name}\n`, /^ {6}- name: /m);
}

test("command-only releases skip desktop work without weakening command gates", () => {
  const desktopBuild = job(releaseWorkflow, "build-macos-desktop");
  const release = job(releaseWorkflow, "release");
  const caskSmoke = job(releaseWorkflow, "homebrew-cask-smoke");
  const finalize = job(releaseWorkflow, "finalize-stable-release");
  const commandAssets = step(release, "Attach immutable command release assets");
  const desktopAssets = step(release, "Attach immutable desktop release assets");

  assert.match(desktopBuild, new RegExp(`^    if: ${desktopGate.replaceAll("'", "\\'")}$`, "m"));
  assert.match(release, /needs\.build-macos-artifacts\.result == 'success'/);
  assert.match(release, /needs\.build-macos-desktop\.result == 'skipped'/);
  assert.doesNotMatch(step(release, "Render verified Homebrew formula"), /if: /);
  assert.doesNotMatch(commandAssets, /if: /);
  assert.match(commandAssets, /\.tar\.gz/);
  assert.match(commandAssets, /dist\/homebrew\/norn\.rb/);
  assert.doesNotMatch(commandAssets, /\.dmg|norn-cask\.rb/);
  assert.match(step(release, "Render verified Homebrew cask"), new RegExp(desktopGate));
  assert.match(step(release, "Upload verified Homebrew cask"), new RegExp(desktopGate));
  assert.match(desktopAssets, new RegExp(desktopGate));
  assert.match(desktopAssets, /\.dmg/);
  assert.match(desktopAssets, /dist\/homebrew\/norn-cask\.rb/);
  assert.doesNotMatch(desktopAssets, /\.tar\.gz|dist\/homebrew\/norn\.rb(?:\n|$)/);
  assert.match(caskSmoke, new RegExp(`^    if: ${desktopGate.replaceAll("'", "\\'")}$`, "m"));
  assert.match(finalize, /needs\.homebrew-formula-smoke\.result == 'success'/);
  assert.match(finalize, /needs\.homebrew-cask-smoke\.result == 'skipped'/);
});

test("tap publication always advances the formula and gates the cask", () => {
  const tap = job(releaseWorkflow, "publish-homebrew-tap");

  assert.doesNotMatch(step(tap, "Download verified Homebrew formula"), /if: /);
  assert.match(step(tap, "Download verified Homebrew cask"), new RegExp(desktopGate));
  assert.match(tap, /git add Formula\/norn\.rb/);
  assert.match(tap, /NORN_DESKTOP_RELEASE_ENABLED:-/);
  assert.match(tap, /git add Casks\/norn\.rb/);
  assert.match(tap, /if ! git diff --cached --quiet -- Casks\/norn\.rb/);
});

test("scheduled lifecycle validation requires desktop assets only when enabled", () => {
  const lifecycle = job(lifecycleWorkflow, "lifecycle");
  const gatedDesktopSteps = [
    "Require complete stable desktop release assets",
    "Download and validate candidate cask",
    "Resolve previous stable desktop release",
    "Validate desktop install, upgrade or authorized bootstrap, uninstall, and reinstall",
  ];

  assert.doesNotMatch(step(lifecycle, "Require complete stable command release assets"), /if: /);
  assert.doesNotMatch(step(lifecycle, "Download and validate candidate formula"), /if: /);
  for (const name of gatedDesktopSteps) {
    assert.match(step(lifecycle, name), new RegExp(desktopGate));
  }
  assert.match(
    step(lifecycle, "Require complete stable desktop release assets"),
    /phase=desktop-asset-check/,
  );
  assert.match(
    step(lifecycle, "Download and validate previous stable cask"),
    new RegExp(desktopGate),
  );
});
