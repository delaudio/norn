import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");
const manifest = read("src-tauri/Cargo.toml");
const scripts = JSON.parse(read("package.json")).scripts;

test("default Rust graph is headless; desktop and Ratatui remain explicit", () => {
  assert.match(manifest, /^default = \[\]$/m);
  assert.match(manifest, /^desktop-bundle = \["desktop", "tui"\]$/m);
  assert.match(manifest, /^custom-protocol = \["desktop", "tauri\/custom-protocol"\]$/m);
  assert.match(manifest, /path = "src\/bin\/norn-app.rs"\nrequired-features = \["desktop"\]/);
  assert.doesNotMatch(read("scripts/verify-core-no-gui-deps.mjs"), /"--no-default-features"/);
});

test("ordinary scripts cannot transitively enable legacy features", () => {
  const visited = new Set();
  function inspect(name) {
    if (visited.has(name)) return;
    visited.add(name);
    const script = scripts[name];
    assert.equal(typeof script, "string", name);
    assert.doesNotMatch(script, /custom-protocol|--features|--bin norn-app|legacy:/, name);
    for (const match of script.matchAll(/pnpm run ([a-z:-]+)/g)) inspect(match[1]);
  }
  for (const name of [
    "dev",
    "build",
    "install:local",
    "cli:install",
    "tui:install",
    "test:rust:cli",
    "test:rust:core",
  ])
    inspect(name);
  assert.match(scripts.build, /browser-diff:build/);
  assert.match(scripts.build, /ui:build/);
});

test("main CI and command releases use headless targets; legacy has a separate lane", () => {
  const ci = read(".github/workflows/ci.yml");
  assert.doesNotMatch(ci, /libwebkit|--all-features|custom-protocol|test:tauri/);
  assert.match(ci, /verify-core-no-gui-deps/);
  assert.match(read(".github/workflows/legacy-ui.yml"), /--all-features/);
  const release = read(".github/workflows/release-norn-macos.yml").split(
    "  build-macos-desktop:",
  )[0];
  assert.doesNotMatch(release, /custom-protocol|--all-features|libwebkit/);
  assert.match(release, /norn-backend/);
  assert.match(release, /dist\/opentui\/norn-ui/);
});
