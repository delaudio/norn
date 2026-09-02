import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(".github/workflows/ci.yml", "utf8");

test("general CI runs the repository gates for every pull request", () => {
  assert.match(workflow, /^\s*pull_request:\s*$/m);
  for (const command of [
    "pnpm run lint",
    "pnpm run typecheck",
    "pnpm run test",
    "pnpm run build",
    "cargo fmt",
    "cargo clippy",
    "cargo test",
    "pnpm run test:rust:cli",
    "pnpm run test:tauri",
    "archgate check",
  ]) {
    assert.ok(workflow.includes(command), `missing CI gate: ${command}`);
  }
});
