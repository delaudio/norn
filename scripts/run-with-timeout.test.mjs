import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import test from "node:test";

const runner = resolve("scripts/run-with-timeout.mjs");

function run(timeout, program) {
  return spawnSync(
    process.execPath,
    [runner, "--timeout-ms", String(timeout), "--", process.execPath, "-e", program],
    { encoding: "utf8", timeout: 30_000 },
  );
}

test("returns the bounded command exit status", () => {
  assert.equal(run(10_000, "process.exit(0)").status, 0);
  assert.equal(run(10_000, "process.exit(7)").status, 7);
});

test("terminates a hung process group with the timeout exit status", () => {
  const startedAt = Date.now();
  const result = run(100, "setInterval(() => {}, 10_000)");

  assert.equal(result.status, 124);
  assert.match(result.stderr, /timed out after 100ms/);
  assert.ok(Date.now() - startedAt < 3_000);
});
