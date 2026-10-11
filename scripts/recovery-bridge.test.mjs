import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { recoveryRevision } from "./verify-recovery-bridge.mjs";

test("archived recovery gates compile the original production origin on every platform without launching it", () => {
  const workflow = readFileSync(
    new URL("../.github/workflows/recovery-bridge.yml", import.meta.url),
    "utf8",
  );
  assert.ok(workflow.includes(recoveryRevision));
  assert.match(workflow, /fetch-depth: 0/);
  for (const os of ["ubuntu-latest", "macos-latest", "windows-latest"])
    assert.ok(workflow.includes(os));
  assert.match(workflow, /run: pnpm run build\n/);
  assert.match(
    workflow,
    /cargo build --locked .*--features custom-protocol,desktop-bundle --bin norn/,
  );
  assert.match(workflow, /cargo test --locked .*runtime_identity::tests::directory_migration/);
  assert.ok(!workflow.includes("cargo run") && !workflow.includes("tauri dev"));
  for (const path of [
    "../docs/user-data-recovery.md",
    "../docs/opentui-migration/user-data-recovery.md",
  ])
    assert.ok(readFileSync(new URL(path, import.meta.url), "utf8").includes(recoveryRevision));
});
