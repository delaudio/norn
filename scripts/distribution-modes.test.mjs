import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const cargoManifest = readFileSync("src-tauri/Cargo.toml", "utf8");
const packageManifest = JSON.parse(readFileSync("package.json", "utf8"));
const releaseWorkflow = readFileSync(".github/workflows/release-norn-macos.yml", "utf8");
const windowsRunner = readFileSync("justfile", "utf8");
const serviceDockerfile = readFileSync("Dockerfile.service", "utf8");

test("desktop routing is the safe default for package builds", () => {
  assert.match(cargoManifest, /^default = \["desktop-bundle"\]$/m);
  assert.match(cargoManifest, /^desktop-bundle = \[\]$/m);
  assert.match(
    windowsRunner,
    /tauri build --bundles nsis --features custom-protocol,desktop-bundle/,
  );
  assert.match(
    releaseWorkflow,
    /tauri build --ci --target "\$TARGET" --bundles dmg --features custom-protocol,desktop-bundle/,
  );
});

test("command distributions explicitly disable desktop routing", () => {
  for (const scriptName of ["install:build", "cli:build", "tui:build"]) {
    assert.match(packageManifest.scripts[scriptName], /--no-default-features/);
  }
  assert.match(serviceDockerfile, /cargo build .*--no-default-features.*--bin norn/);
  assert.match(
    releaseWorkflow,
    /cargo build .*--target "\$TARGET" --no-default-features --features custom-protocol --bin norn --bin norn-tui/,
  );
});
